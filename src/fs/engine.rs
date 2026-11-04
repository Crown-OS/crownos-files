use std::{path::Path, sync::Arc};

use compact_str::CompactString;
use tokio::{
    sync::{
        mpsc::{UnboundedReceiver, UnboundedSender},
        watch,
    },
    task::spawn_blocking,
};
use tokio_util::sync::{CancellationToken, DropGuard};

use crate::{
    error::EngineError,
    fs::{
        dir_size::measure_dirs,
        mounts::MountTable,
        refresher::Refresher,
        revalidate::{Freshness, revalidate},
        watcher::Watcher,
    },
    model::{dir_stamp::DirStamp, files_op::FilesOp},
};

/// What the UI asks of the filesystem engine.
#[derive(Debug, Clone)]
pub enum EngineCmd {
    /// List `dir` now (coalesced with a listing already running).
    Load(Arc<Path>),
    /// List `dir` again only if it changed since the listing `stamp` came from.
    Revalidate {
        dir: Arc<Path>,
        stamp: Option<DirStamp>,
    },
    /// Stop listing `dir`.
    Cancel(Arc<Path>),
    /// Watch exactly these directories from now on.
    Watch(Vec<Arc<Path>>),
    /// Measure these subdirectories of `dir`, abandoning any earlier request.
    MeasureDirs {
        dir: Arc<Path>,
        names: Vec<CompactString>,
    },
}

/// Serves `commands` until the sender is dropped, sending every change as a
/// [`FilesOp`] to `ops`. All filesystem work runs on blocking threads, so the
/// caller's executor is never stalled.
pub async fn run_engine(
    ops: UnboundedSender<FilesOp>,
    mut commands: UnboundedReceiver<EngineCmd>,
    mounts: watch::Receiver<Arc<MountTable>>,
) -> Result<(), EngineError> {
    let refresher = Arc::new(Refresher::spawn(ops.clone()));
    let watcher = Watcher::spawn(ops.clone(), mounts.clone())?;
    let mut measuring: Option<DropGuard> = None;
    while let Some(command) = commands.recv().await {
        match command {
            EngineCmd::Load(dir) => refresher.load(dir),
            EngineCmd::Revalidate { dir, stamp: None } => refresher.load(dir),
            EngineCmd::Revalidate { dir, stamp } => {
                let (refresher, table) = (refresher.clone(), mounts.borrow().clone());
                tokio::spawn(async move {
                    let checked = dir.clone();
                    let freshness = spawn_blocking(move || revalidate(&checked, stamp, &table));
                    if freshness
                        .await
                        .is_ok_and(|freshness| freshness == Freshness::Stale)
                    {
                        refresher.load(dir);
                    }
                });
            }
            EngineCmd::Cancel(dir) => refresher.cancel(dir),
            EngineCmd::Watch(dirs) => watcher.watch(dirs),
            EngineCmd::MeasureDirs { dir, names } => {
                let cancel = CancellationToken::new();
                measuring = Some(cancel.clone().drop_guard());
                let ops = ops.clone();
                spawn_blocking(move || {
                    measure_dirs(&dir, &names, &cancel, |op| {
                        if ops.send(op).is_err() {
                            cancel.cancel();
                        }
                    });
                });
            }
        }
    }
    drop(measuring);
    Ok(())
}
