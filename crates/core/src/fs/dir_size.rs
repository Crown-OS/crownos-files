use std::{
    ffi::OsStr,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use compact_str::CompactString;
use rustix::fs::{AtFlags, CWD, Dir, FileType, Mode, OFlags, RawMode, StatxFlags, openat, statx};
use tokio_util::sync::CancellationToken;

use crate::{
    model::files_op::FilesOp,
    util::{hash::FastIndexMap, throttle::Throttle},
};

const SLICE_ENTRIES: usize = 5000;
const SLICE_TIME: Duration = Duration::from_millis(50);
const CLOCK_CHECK_STRIDE: usize = 256;
const DELIVERY_INTERVAL: Duration = Duration::from_millis(300);

/// Measures the total size of each of `names` inside `parent`, on the calling
/// (blocking) thread.
///
/// The walk runs in slices of [`SLICE_ENTRIES`] entries or [`SLICE_TIME`],
/// checking `cancel` between them. Sizes, including the running total of the
/// directory still being walked, are emitted as `Size` ops at most every
/// [`DELIVERY_INTERVAL`], and once more at the end.
pub fn measure_dirs(
    parent: &Arc<Path>,
    names: &[CompactString],
    cancel: &CancellationToken,
    mut emit: impl FnMut(FilesOp),
) {
    let mut pending: FastIndexMap<CompactString, u64> = FastIndexMap::default();
    let mut delivery = Throttle::new(DELIVERY_INTERVAL);
    let mut deliver = |pending: &mut FastIndexMap<CompactString, u64>| {
        if !pending.is_empty() {
            let sizes = pending.drain(..).collect();
            emit(FilesOp::Size {
                dir: parent.clone(),
                sizes,
            });
        }
    };
    for name in names {
        let mut walk = SizeWalk::new(parent.join(name.as_str()));
        while !walk.step() {
            if cancel.is_cancelled() {
                return;
            }
            if delivery.ready() {
                pending.insert(name.clone(), walk.total);
                deliver(&mut pending);
            }
        }
        pending.insert(name.clone(), walk.total);
        if delivery.ready() {
            deliver(&mut pending);
        }
    }
    deliver(&mut pending);
}

/// An iterative, resumable walk summing apparent file sizes. Symlinks are
/// counted as links, never followed.
struct SizeWalk {
    pending: Vec<PathBuf>,
    current: Option<(PathBuf, Dir)>,
    total: u64,
}

impl SizeWalk {
    fn new(root: PathBuf) -> Self {
        Self {
            pending: vec![root],
            current: None,
            total: 0,
        }
    }

    /// Walks one slice; `true` once the whole tree is summed.
    fn step(&mut self) -> bool {
        let started = Instant::now();
        for visited in 1..=SLICE_ENTRIES {
            if visited % CLOCK_CHECK_STRIDE == 0 && started.elapsed() >= SLICE_TIME {
                return false;
            }
            if self.current.is_none() && !self.open_next() {
                return true;
            }
            let Some((path, dir)) = &mut self.current else {
                return true;
            };
            let Some(Ok(dirent)) = dir.read() else {
                self.current = None;
                continue;
            };
            let name = dirent.file_name().to_bytes();
            if name == b"." || name == b".." {
                continue;
            }
            let Ok(fd) = dir.fd() else {
                continue;
            };
            let flags = AtFlags::SYMLINK_NOFOLLOW | AtFlags::STATX_DONT_SYNC;
            let Ok(stat) = statx(fd, name, flags, StatxFlags::TYPE | StatxFlags::SIZE) else {
                continue;
            };
            if FileType::from_raw_mode(RawMode::from(stat.stx_mode)) == FileType::Directory {
                self.pending.push(path.join(OsStr::from_bytes(name)));
            } else {
                self.total += stat.stx_size;
            }
        }
        false
    }

    fn open_next(&mut self) -> bool {
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        while let Some(path) = self.pending.pop() {
            let opened = openat(CWD, &path, flags, Mode::empty()).and_then(Dir::new);
            if let Ok(dir) = opened {
                self.current = Some((path, dir));
                return true;
            }
        }
        false
    }
}
