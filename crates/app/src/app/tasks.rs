//! One-off filesystem work the UI asks for, run off the UI thread by a
//! worker that lives as long as the window.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::Arc,
};

use crownui::prelude::Cx;
use tokio::{
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    task::spawn_blocking,
};

use crate::{
    app::listing::{AfterChange, resolve_after_change},
    jobs::{JobError, create_folder, rename},
    model::disk_usage::DiskUsage,
    state::Store,
};

pub const NEW_FOLDER_NAME: &str = "New Folder";

#[derive(Debug)]
pub enum Task {
    Rename { path: PathBuf, name: OsString },
    CreateFolder { dir: PathBuf },
    MeasureDisk(Arc<Path>),
}

#[derive(Debug)]
pub enum TaskDone {
    Renamed(Result<PathBuf, JobError>),
    Created(Result<PathBuf, JobError>),
    Measured(Arc<Path>, DiskUsage),
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
        Task::Rename { path, name } => TaskDone::Renamed(rename(path, name).await),
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
    })
}

pub fn finish_tasks(cx: &mut Cx, store: Store, burst: Vec<TaskDone>) {
    for done in burst {
        match done {
            TaskDone::Renamed(Ok(_)) => resolve_after_change(cx, store),
            TaskDone::Created(Ok(path)) => {
                let Some(name) = path.file_name() else {
                    continue;
                };
                let name = name.to_string_lossy().into();
                store
                    .after_change
                    .update(cx, |slot| *slot = Some(AfterChange::Rename(name)));
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
        }
    }
}
