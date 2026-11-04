use std::{
    mem::MaybeUninit,
    ops::Range,
    os::fd::{AsFd, BorrowedFd},
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};
use rustix::fs::{CWD, Mode, OFlags, RawDir, openat};
use tokio_util::sync::CancellationToken;

use crate::{
    fs::{revalidate::stamp_of, stat_at::stat_entry},
    model::{entry::Entry, files_op::FilesOp, ticket::Ticket},
};

/// Small enough that the first screen paints before a big directory is read.
const FIRST_CHUNK: usize = 256;
const CHUNK: usize = 4096;
const FLUSH_INTERVAL: Duration = Duration::from_millis(16);
const CLOCK_CHECK_STRIDE: usize = 64;
const DIRENT_BUFFER_BYTES: usize = 64 * 1024;
const PARALLEL_STAT_THRESHOLD: usize = 512;
const PARALLEL_STAT_BATCH: usize = 64;

/// Lists `dir` on the calling thread, which must be allowed to block.
///
/// Names come from `getdents64` into one reused buffer and are statted with
/// `statx` relative to the directory's fd, in parallel for large chunks.
/// Emits `Part` chunks (the first after [`FIRST_CHUNK`] names, then every
/// [`CHUNK`] names or [`FLUSH_INTERVAL`]) and then `Done`, or `Fail`. Stops
/// silently once `cancel` fires.
pub fn list_dir(
    dir: &Arc<Path>,
    ticket: Ticket,
    cancel: &CancellationToken,
    mut emit: impl FnMut(FilesOp),
) {
    let fail = |error| FilesOp::Fail {
        dir: dir.clone(),
        ticket,
        error,
    };
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
    let opened = openat(CWD, &**dir, flags, Mode::empty()).and_then(|fd| Ok((stamp_of(&fd)?, fd)));
    let (stamp, fd) = match opened {
        Ok(opened) => opened,
        Err(error) => return emit(fail(error)),
    };
    let mut batch = NameBatch::default();
    let mut flush_at = FIRST_CHUNK;
    let mut last_flush = Instant::now();
    let mut buffer = vec![MaybeUninit::uninit(); DIRENT_BUFFER_BYTES];
    let mut dirents = RawDir::new(&fd, &mut buffer);
    while let Some(dirent) = dirents.next() {
        if cancel.is_cancelled() {
            return;
        }
        match dirent {
            Ok(dirent) => batch.push(dirent.file_name().to_bytes()),
            Err(error) => return emit(fail(error)),
        }
        let due = batch.len() >= flush_at
            || (batch.len() % CLOCK_CHECK_STRIDE == 0 && last_flush.elapsed() >= FLUSH_INTERVAL);
        if due && !batch.is_empty() {
            emit(FilesOp::Part {
                dir: dir.clone(),
                ticket,
                entries: batch.stat_all(fd.as_fd()),
            });
            batch.clear();
            flush_at = CHUNK;
            last_flush = Instant::now();
        }
    }
    if cancel.is_cancelled() {
        return;
    }
    if !batch.is_empty() {
        emit(FilesOp::Part {
            dir: dir.clone(),
            ticket,
            entries: batch.stat_all(fd.as_fd()),
        });
    }
    emit(FilesOp::Done {
        dir: dir.clone(),
        ticket,
        stamp,
    });
}

/// Names copied out of the dirent buffer into one arena.
#[derive(Default)]
struct NameBatch {
    bytes: Vec<u8>,
    spans: Vec<Range<usize>>,
}

impl NameBatch {
    fn push(&mut self, name: &[u8]) {
        if name == b"." || name == b".." {
            return;
        }
        let start = self.bytes.len();
        self.bytes.extend_from_slice(name);
        self.spans.push(start..self.bytes.len());
    }

    fn len(&self) -> usize {
        self.spans.len()
    }

    fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    fn clear(&mut self) {
        self.bytes.clear();
        self.spans.clear();
    }

    fn stat_all(&self, dir: BorrowedFd<'_>) -> Vec<Entry> {
        let stat = |span: &Range<usize>| stat_entry(dir, &self.bytes[span.clone()]);
        if self.len() < PARALLEL_STAT_THRESHOLD {
            return self.spans.iter().map(stat).collect();
        }
        self.spans
            .as_slice()
            .into_par_iter()
            .with_min_len(PARALLEL_STAT_BATCH)
            .map(stat)
            .collect()
    }
}
