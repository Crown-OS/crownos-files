use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    sync::Arc,
};

use tokio::{
    io::{Interest, unix::AsyncFd},
    sync::watch,
};
use tokio_util::task::AbortOnDropHandle;

use crate::{
    error::EngineError,
    fs::mounts::{MOUNTINFO, MountTable},
};

/// The live mount table. The kernel flags `/proc/self/mountinfo` with
/// `POLLPRI` whenever a mount appears or goes; each change is re-read and
/// published to every subscriber.
pub struct MountWatch {
    table: watch::Receiver<Arc<MountTable>>,
    _task: AbortOnDropHandle<()>,
}

impl MountWatch {
    /// Starts watching on the current tokio runtime.
    pub fn spawn() -> Result<Self, EngineError> {
        let read_error = |source| EngineError::Read {
            path: MOUNTINFO.into(),
            source,
        };
        let mut file = File::open(MOUNTINFO).map_err(read_error)?;
        let table = read_table(&mut file).map_err(read_error)?;
        // SAFETY: the `File` is moved into the `AsyncFd`, which owns its fd until dropped.
        let mountinfo = unsafe { AsyncFd::register_with_interest(file, Interest::PRIORITY) }
            .map_err(|error| read_error(error.into()))?;
        let (publisher, table) = watch::channel(Arc::new(table));
        let task = tokio::spawn(publish_changes(mountinfo, publisher));
        Ok(Self {
            table,
            _task: AbortOnDropHandle::new(task),
        })
    }

    pub fn current(&self) -> Arc<MountTable> {
        self.table.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<Arc<MountTable>> {
        self.table.clone()
    }
}

async fn publish_changes(mut mountinfo: AsyncFd<File>, publisher: watch::Sender<Arc<MountTable>>) {
    while let Ok(mut ready) = mountinfo.ready_mut(Interest::PRIORITY).await {
        ready.clear_ready();
        let Ok(table) = read_table(ready.get_inner_mut()) else {
            continue;
        };
        publisher.send_if_modified(|current| {
            let changed = **current != table;
            if changed {
                *current = Arc::new(table);
            }
            changed
        });
    }
}

fn read_table(file: &mut File) -> std::io::Result<MountTable> {
    file.seek(SeekFrom::Start(0))?;
    let mut text = String::new();
    file.read_to_string(&mut text)?;
    Ok(MountTable::parse(&text))
}
