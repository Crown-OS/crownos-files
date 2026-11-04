//! One-off filesystem work the UI asks for, run off the UI thread by a
//! worker that lives as long as the window.

use std::{
    ffi::OsString,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::Arc,
};

use compact_str::CompactString;
use crownos_files_devices::{storage::space_usage, tags::Tags};
use crownui::prelude::Cx;
use tokio::{
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    task::spawn_blocking,
};
use tokio_util::sync::CancellationToken;

use crate::{
    app::{
        listing::{AfterChange, apply_ops, resolve_after_change},
        properties::{FileFacts, adopt_facts, adopt_folder_size, inspect, measure_folder},
        storage::DiskSpace,
        tagging::{TagChange, adopt_tags, change_tags, read_entry_tags},
        virtual_folder::{VirtualFolder, VirtualListing, apply_listing, reload_virtual},
    },
    jobs::{JobError, Reversible, create_folder, rename},
    mime::{refine_kind, sniff},
    model::{files_op::FilesOp, kind::FileKind, mime_id::MimeId},
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
    /// The disk `dir` is on, and whether `home` is on it too.
    MeasureDisk {
        dir: Arc<Path>,
        home: Option<PathBuf>,
    },
    ListVirtual(VirtualFolder),
    /// Types the entries named in `dir` by content, as their names say nothing.
    Sniff {
        dir: Arc<Path>,
        entries: Vec<(CompactString, PathBuf)>,
    },
    ReadTags {
        dir: Arc<Path>,
        generation: u64,
        entries: Vec<(CompactString, OsString)>,
    },
    Tag {
        dir: Arc<Path>,
        entries: Vec<(CompactString, PathBuf)>,
        tag: CompactString,
        change: TagChange,
    },
    Inspect(PathBuf),
    MeasureFolder {
        path: PathBuf,
        cancel: CancellationToken,
    },
}

#[derive(Debug)]
pub enum TaskDone {
    Renamed(Result<(PathBuf, PathBuf), JobError>),
    Created(Result<PathBuf, JobError>),
    Measured(Arc<Path>, DiskSpace),
    Listed(VirtualListing),
    Sniffed {
        dir: Arc<Path>,
        kinds: Vec<(CompactString, MimeId, FileKind)>,
    },
    TagsRead {
        dir: Arc<Path>,
        generation: u64,
        tags: Vec<(CompactString, Tags)>,
    },
    Tagged {
        dir: Arc<Path>,
        tags: Vec<(CompactString, Tags)>,
        failure: Option<String>,
    },
    Inspected(PathBuf, FileFacts),
    FolderMeasured(PathBuf, u64),
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
        Task::MeasureDisk { dir, home } => {
            let measured = dir.clone();
            let space = spawn_blocking(move || disk_space(&measured, home.as_deref()))
                .await
                .ok()??;
            TaskDone::Measured(dir, space)
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
        Task::ReadTags {
            dir,
            generation,
            entries,
        } => {
            let read = dir.clone();
            let tags = spawn_blocking(move || read_entry_tags(&read, entries))
                .await
                .ok()?;
            TaskDone::TagsRead {
                dir,
                generation,
                tags,
            }
        }
        Task::Tag {
            dir,
            entries,
            tag,
            change,
        } => {
            let (tags, failure) = spawn_blocking(move || change_tags(entries, &tag, change))
                .await
                .ok()?;
            TaskDone::Tagged { dir, tags, failure }
        }
        Task::Inspect(path) => {
            let inspected = path.clone();
            let facts = spawn_blocking(move || inspect(&inspected))
                .await
                .ok()?
                .ok()?;
            TaskDone::Inspected(path, facts)
        }
        Task::MeasureFolder { path, cancel } => {
            let measured = path.clone();
            let bytes = spawn_blocking(move || measure_folder(&measured, &cancel))
                .await
                .ok()??;
            TaskDone::FolderMeasured(path, bytes)
        }
    })
}

fn disk_space(dir: &Path, home: Option<&Path>) -> Option<DiskSpace> {
    let usage = space_usage(dir).ok()?;
    let device = |path: &Path| std::fs::metadata(path).ok().map(|metadata| metadata.dev());
    let holds_home = home
        .and_then(device)
        .is_some_and(|home| device(dir) == Some(home));
    Some(DiskSpace { usage, holds_home })
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
            TaskDone::Measured(dir, space) => {
                if store.cwd.with(cx, |cwd| *cwd == dir) {
                    store.storage.show(cx, space);
                }
            }
            TaskDone::Listed(listing) => apply_listing(cx, store, listing),
            TaskDone::Sniffed { dir, kinds } => apply_kinds(cx, store, dir, &kinds),
            TaskDone::TagsRead {
                dir,
                generation,
                tags,
            } => adopt_tags(cx, store, &dir, Some(generation), tags),
            TaskDone::Tagged { dir, tags, failure } => {
                adopt_tags(cx, store, &dir, None, tags);
                if let Some(failure) = failure {
                    store.toasts.show(cx, failure);
                }
            }
            TaskDone::Inspected(path, facts) => adopt_facts(cx, store, &path, facts),
            TaskDone::FolderMeasured(path, bytes) => adopt_folder_size(cx, store, &path, bytes),
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
