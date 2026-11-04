//! One-off filesystem work the UI asks for, run off the UI thread by a
//! worker that lives as long as the window.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::Arc,
};

use compact_str::CompactString;
use crownui::prelude::Cx;
use tokio::{
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    task::spawn_blocking,
};

use crate::{
    app::{
        listing::{AfterChange, apply_ops, resolve_after_change},
        virtual_folder::{VirtualFolder, VirtualListing, apply_listing, reload_virtual},
    },
    jobs::{JobError, Reversible, create_folder, rename},
    mime::{refine_kind, sniff},
    model::{disk_usage::DiskUsage, files_op::FilesOp, kind::FileKind, mime_id::MimeId},
    state::Store,
};

pub const NEW_FOLDER_NAME: &str = "New Folder";

#[derive(Debug)]
pub enum Task {
    Rename {
        path: PathBuf,
        name: OsString,
    },
    CreateFolder {
        dir: PathBuf,
    },
    MeasureDisk(Arc<Path>),
    ListVirtual(VirtualFolder),
    /// Types the entries named in `dir` by content, as their names say nothing.
    Sniff {
        dir: Arc<Path>,
        entries: Vec<(CompactString, PathBuf)>,
    },
}

#[derive(Debug)]
pub enum TaskDone {
    Renamed(Result<(PathBuf, PathBuf), JobError>),
    Created(Result<PathBuf, JobError>),
    Measured(Arc<Path>, DiskUsage),
    Listed(VirtualListing),
    Sniffed {
        dir: Arc<Path>,
        kinds: Vec<(CompactString, MimeId, FileKind)>,
    },
}

/// Serves `tasks` until the window closes, each one concurrently.
pub async fn run_tasks(done: UnboundedSender<TaskDone>, mut tasks: UnboundedReceiver<Task>) {
    while let Some(task) = tasks.recv().await {
        let done = done.clone();
        tokio::spawn(async move {
            if let Some(outcome) = perform(task).await {
                let _ = done.send(outcome);
            }
        });
    }
}

async fn perform(task: Task) -> Option<TaskDone> {
    Some(match task {
        Task::Rename { path, name } => {
            TaskDone::Renamed(rename(path.clone(), name).await.map(|to| (path, to)))
        }
        Task::CreateFolder { dir } => {
            TaskDone::Created(create_folder(dir, NEW_FOLDER_NAME.into()).await)
        }
        Task::MeasureDisk(dir) => {
            let measured = dir.clone();
            let usage = spawn_blocking(move || DiskUsage::of(&measured))
                .await
                .ok()?
                .ok()?;
            TaskDone::Measured(dir, usage)
        }
        Task::ListVirtual(folder) => TaskDone::Listed(folder.list().await),
        Task::Sniff { dir, entries } => {
            let kinds = spawn_blocking(move || {
                entries
                    .into_iter()
                    .filter_map(|(name, path)| {
                        let mime = sniff(&path).ok()?;
                        Some((name, mime, refine_kind(FileKind::Other, mime)))
                    })
                    .collect()
            })
            .await
            .ok()?;
            TaskDone::Sniffed { dir, kinds }
        }
    })
}

pub fn finish_tasks(cx: &mut Cx, store: Store, burst: Vec<TaskDone>) {
    for done in burst {
        match done {
            TaskDone::Renamed(Ok((from, to))) => {
                store
                    .jobs
                    .undo
                    .update(cx, |undo| undo.push(Reversible::Rename { from, to }));
                reload_virtual(cx, store);
                resolve_after_change(cx, store);
            }
            TaskDone::Created(Ok(path)) => {
                let Some(name) = path.file_name() else {
                    continue;
                };
                let name = name.to_string_lossy().into();
                store
                    .after_change
                    .update(cx, |slot| *slot = Some(AfterChange::Rename(name)));
                store
                    .jobs
                    .undo
                    .update(cx, |undo| undo.push(Reversible::Create(path)));
                resolve_after_change(cx, store);
            }
            TaskDone::Renamed(Err(error)) | TaskDone::Created(Err(error)) => {
                store.after_change.update(cx, |slot| *slot = None);
                store.toasts.show(cx, error.to_string());
            }
            TaskDone::Measured(dir, usage) => {
                if store.cwd.with(cx, |cwd| *cwd == dir) {
                    store.storage.set(cx, Some(usage));
                }
            }
            TaskDone::Listed(listing) => apply_listing(cx, store, listing),
            TaskDone::Sniffed { dir, kinds } => apply_kinds(cx, store, dir, &kinds),
        }
    }
}

/// Rewrites the sniffed entries with their content types.
fn apply_kinds(
    cx: &mut Cx,
    store: Store,
    dir: Arc<Path>,
    kinds: &[(CompactString, MimeId, FileKind)],
) {
    if store.dir(cx) != dir {
        return;
    }
    let entries = store.folder.with(cx, |folder| {
        kinds
            .iter()
            .filter_map(|(name, mime, kind)| {
                let position = folder.position_of(name)?;
                let mut entry = folder.get(position)?.clone();
                entry.mime = *mime;
                entry.kind = *kind;
                Some(entry)
            })
            .collect::<Vec<_>>()
    });
    if !entries.is_empty() {
        apply_ops(cx, store, vec![FilesOp::Upsert { dir, entries }]);
    }
}
