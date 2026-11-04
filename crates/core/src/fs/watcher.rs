use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use compact_str::CompactString;
use notify::{
    Config, Event, EventKind, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher as _,
    event::{AccessKind, AccessMode},
};
use rustix::io::Errno;
use tokio::{
    sync::{
        mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
        watch,
    },
    task::spawn_blocking,
    time::sleep,
};
use tokio_util::task::AbortOnDropHandle;

use crate::{
    error::EngineError,
    fs::{
        mounts::{Mount, MountTable},
        stat_at::stat_path,
    },
    model::files_op::FilesOp,
    util::hash::{FastHashMap, FastHashSet},
};

const BATCH_WINDOW: Duration = Duration::from_millis(250);
const BATCH_LIMIT: usize = 1000;
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Watches the directories on screen (non-recursively) and reports what
/// changed in them as `Upsert`/`Delete` ops.
///
/// Raw events are collected for [`BATCH_WINDOW`], deduplicated, and every
/// touched path is re-statted, so a burst of writes to one file costs one
/// stat. Directories on network and FUSE filesystems are polled instead,
/// since inotify never hears about changes made elsewhere.
pub struct Watcher {
    desired: watch::Sender<Vec<Arc<Path>>>,
    _reconcile: AbortOnDropHandle<()>,
    _deliver: AbortOnDropHandle<()>,
}

impl Watcher {
    /// Starts watching on the current tokio runtime.
    pub fn spawn(
        ops: UnboundedSender<FilesOp>,
        mounts: watch::Receiver<Arc<MountTable>>,
    ) -> Result<Self, EngineError> {
        let (changed_paths, changes) = unbounded_channel();
        let backends = Backends::new(changed_paths)?;
        let (desired, wanted) = watch::channel(Vec::new());
        Ok(Self {
            desired,
            _reconcile: AbortOnDropHandle::new(tokio::spawn(reconcile(backends, wanted, mounts))),
            _deliver: AbortOnDropHandle::new(tokio::spawn(deliver(changes, ops))),
        })
    }

    /// Replaces the watched set; only the latest request matters.
    pub fn watch(&self, dirs: Vec<Arc<Path>>) {
        self.desired.send_replace(dirs);
    }
}

#[derive(Debug, Clone, Copy)]
enum Backend {
    Native,
    Poll,
}

struct Backends {
    native: RecommendedWatcher,
    poll: Option<PollWatcher>,
    sink: UnboundedSender<PathBuf>,
    watched: FastHashMap<Arc<Path>, Backend>,
}

impl Backends {
    fn new(sink: UnboundedSender<PathBuf>) -> Result<Self, EngineError> {
        Ok(Self {
            native: RecommendedWatcher::new(forward_to(sink.clone()), Config::default())?,
            poll: None,
            sink,
            watched: FastHashMap::default(),
        })
    }

    fn sync(&mut self, desired: &[Arc<Path>], mounts: &MountTable) {
        self.watched.retain(|dir, backend| {
            let keep = desired.contains(dir);
            if !keep {
                let _ = match backend {
                    Backend::Native => self.native.unwatch(dir),
                    Backend::Poll => self.poll.as_mut().map_or(Ok(()), |poll| poll.unwatch(dir)),
                };
            }
            keep
        });
        for dir in desired {
            if self.watched.contains_key(dir) {
                continue;
            }
            let backend = if mounts.mount_for(dir).is_some_and(Mount::is_remote) {
                self.watch_polling(dir)
            } else {
                self.watch_native(dir).or_else(|_| self.watch_polling(dir))
            };
            if let Ok(backend) = backend {
                self.watched.insert(dir.clone(), backend);
            }
        }
    }

    fn watch_native(&mut self, dir: &Path) -> notify::Result<Backend> {
        self.native.watch(dir, RecursiveMode::NonRecursive)?;
        Ok(Backend::Native)
    }

    fn watch_polling(&mut self, dir: &Path) -> notify::Result<Backend> {
        let poll = match &mut self.poll {
            Some(poll) => poll,
            None => self.poll.insert(PollWatcher::new(
                forward_to(self.sink.clone()),
                Config::default().with_poll_interval(POLL_INTERVAL),
            )?),
        };
        poll.watch(dir, RecursiveMode::NonRecursive)?;
        Ok(Backend::Poll)
    }
}

fn forward_to(
    sink: UnboundedSender<PathBuf>,
) -> impl FnMut(notify::Result<Event>) + Send + 'static {
    move |event: notify::Result<Event>| {
        let Ok(event) = event else {
            return;
        };
        if is_read_only(event.kind) {
            return;
        }
        for path in event.paths {
            let _ = sink.send(path);
        }
    }
}

fn is_read_only(kind: EventKind) -> bool {
    matches!(kind, EventKind::Access(access) if access != AccessKind::Close(AccessMode::Write))
}

async fn reconcile(
    mut backends: Backends,
    mut wanted: watch::Receiver<Vec<Arc<Path>>>,
    mounts: watch::Receiver<Arc<MountTable>>,
) {
    while wanted.changed().await.is_ok() {
        let desired = wanted.borrow_and_update().clone();
        let table = mounts.borrow().clone();
        let synced = spawn_blocking(move || {
            backends.sync(&desired, &table);
            backends
        });
        let Ok(synced) = synced.await else {
            return;
        };
        backends = synced;
    }
}

async fn deliver(mut changes: UnboundedReceiver<PathBuf>, ops: UnboundedSender<FilesOp>) {
    let mut batch = Vec::with_capacity(BATCH_LIMIT);
    while changes.recv_many(&mut batch, BATCH_LIMIT).await > 0 {
        if batch.len() < BATCH_LIMIT {
            sleep(BATCH_WINDOW).await;
            while batch.len() < BATCH_LIMIT {
                let Ok(path) = changes.try_recv() else {
                    break;
                };
                batch.push(path);
            }
        }
        let paths: FastHashSet<PathBuf> = batch.drain(..).collect();
        let Ok(updates) = spawn_blocking(move || restat(&paths)).await else {
            return;
        };
        if updates.into_iter().any(|op| ops.send(op).is_err()) {
            return;
        }
    }
}

fn restat(paths: &FastHashSet<PathBuf>) -> Vec<FilesOp> {
    FilesOp::merge(paths.iter().filter_map(|path| {
        let dir: Arc<Path> = path.parent()?.into();
        Some(match stat_path(path) {
            Ok(entry) => FilesOp::Upsert {
                dir,
                entries: vec![entry],
            },
            Err(Errno::NOENT | Errno::NOTDIR) => FilesOp::Delete {
                dir,
                names: vec![CompactString::from(path.file_name()?.to_string_lossy())],
            },
            Err(_) => return None,
        })
    }))
}
