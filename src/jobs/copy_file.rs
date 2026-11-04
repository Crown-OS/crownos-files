use std::{
    fs::{self, File, OpenOptions},
    io::{self, ErrorKind, Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use rustix::{
    fs::{CWD, Mode, OFlags, copy_file_range, ioctl_ficlone, openat, sendfile},
    io::Errno,
};
use tokio_util::sync::CancellationToken;

use crate::jobs::{
    attributes::copy_attributes,
    context::check,
    error::{AtPath, JobError},
};

const COPY_CHUNK: usize = 32 * 1024 * 1024;
const BUFFER_SIZE: usize = 128 * 1024;
const NEW_FILE_MODE: u32 = 0o600;
const SOURCE_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::NONBLOCK)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

static COPY_RANGE_AVAILABLE: AtomicBool = AtomicBool::new(true);
static SENDFILE_AVAILABLE: AtomicBool = AtomicBool::new(true);

/// Copies the regular file `source` to the new file `target` with its mode
/// and times, trying a reflink, then `copy_file_range`, then `sendfile`, then
/// a buffered copy. `on_copied` hears every chunk; `cancel` is checked
/// between chunks. On failure or cancellation `target` is removed.
pub fn copy_file(
    source: &Path,
    target: &Path,
    cancel: &CancellationToken,
    on_copied: impl FnMut(u64),
) -> Result<(), JobError> {
    let output = create_exclusive(target).at(target)?;
    fill_file(source, target, &output, cancel, on_copied)
}

pub(crate) fn create_exclusive(target: &Path) -> io::Result<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(NEW_FILE_MODE)
        .open(target)
}

/// Copies `source` into `output`, the just-created file at `target`.
pub(crate) fn fill_file(
    source: &Path,
    target: &Path,
    output: &File,
    cancel: &CancellationToken,
    mut on_copied: impl FnMut(u64),
) -> Result<(), JobError> {
    let copied = copy_into(source, target, output, cancel, &mut on_copied);
    if copied.is_err() {
        let _ = fs::remove_file(target);
    }
    copied
}

fn copy_into(
    source: &Path,
    target: &Path,
    output: &File,
    cancel: &CancellationToken,
    on_copied: &mut impl FnMut(u64),
) -> Result<(), JobError> {
    let input = File::from(openat(CWD, source, SOURCE_FLAGS, Mode::empty()).at(source)?);
    let meta = input.metadata().at(source)?;
    copy_data(&input, output, meta.len(), target, cancel, on_copied)?;
    copy_attributes(&meta, output).at(target)
}

fn copy_data(
    input: &File,
    output: &File,
    len: u64,
    target: &Path,
    cancel: &CancellationToken,
    on_copied: &mut impl FnMut(u64),
) -> Result<(), JobError> {
    check(cancel)?;
    if len > 0 {
        if ioctl_ficlone(output, input).is_ok() {
            on_copied(len);
            return Ok(());
        }
        let range = || copy_file_range(input, None, output, None, COPY_CHUNK);
        if copy_in_kernel(&COPY_RANGE_AVAILABLE, range, target, cancel, on_copied)? {
            return Ok(());
        }
        let send = || sendfile(output, input, None, COPY_CHUNK);
        if copy_in_kernel(&SENDFILE_AVAILABLE, send, target, cancel, on_copied)? {
            return Ok(());
        }
    }
    copy_buffered(input, output, target, cancel, on_copied)
}

/// Runs `step` until end of file; `false` when it copied nothing and the
/// next method should take over. Only a missing syscall disables it for good.
fn copy_in_kernel(
    available: &AtomicBool,
    step: impl Fn() -> rustix::io::Result<usize>,
    target: &Path,
    cancel: &CancellationToken,
    on_copied: &mut impl FnMut(u64),
) -> Result<bool, JobError> {
    if !available.load(Ordering::Relaxed) {
        return Ok(false);
    }
    let mut copied_any = false;
    loop {
        check(cancel)?;
        match step() {
            Ok(0) => return Ok(copied_any),
            Ok(copied) => {
                copied_any = true;
                on_copied(copied as u64);
            }
            Err(Errno::INTR) => {}
            Err(Errno::NOSYS) if !copied_any => {
                available.store(false, Ordering::Relaxed);
                return Ok(false);
            }
            Err(
                Errno::XDEV
                | Errno::INVAL
                | Errno::OPNOTSUPP
                | Errno::BADF
                | Errno::PERM
                | Errno::OVERFLOW,
            ) if !copied_any => return Ok(false),
            Err(error) => return Err(error).at(target),
        }
    }
}

fn copy_buffered(
    mut input: &File,
    mut output: &File,
    target: &Path,
    cancel: &CancellationToken,
    on_copied: &mut impl FnMut(u64),
) -> Result<(), JobError> {
    let mut buffer = vec![0; BUFFER_SIZE];
    loop {
        check(cancel)?;
        let read = match input.read(&mut buffer) {
            Ok(0) => return Ok(()),
            Ok(read) => read,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error).at(target),
        };
        output.write_all(&buffer[..read]).at(target)?;
        on_copied(read as u64);
    }
}
