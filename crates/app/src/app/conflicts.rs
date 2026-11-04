//! A job found its target taken: the conflict dialog asks Replace, Skip or
//! Keep Both, one conflict at a time, and answers the waiting job.

use std::{collections::VecDeque, path::PathBuf};

use crownui::prelude::{Cx, Runtime, Signal, Store as Cell};
use tokio::sync::oneshot;

use crate::{
    jobs::{ConflictPolicy, ConflictReply, JobId},
    state::Store,
};

struct PendingConflict {
    id: JobId,
    target: PathBuf,
    reply: oneshot::Sender<ConflictReply>,
}

/// What the dialog says about the conflict at the front of the queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictPrompt {
    pub name: String,
    pub folder: String,
}

#[derive(Clone, Copy)]
pub struct Conflicts {
    queue: Cell<VecDeque<PendingConflict>>,
    pub prompt: Signal<Option<ConflictPrompt>>,
    pub open: Signal<bool>,
    pub apply_to_all: Signal<bool>,
}

impl Conflicts {
    pub fn new(cx: &mut Cx) -> Self {
        Self {
            queue: cx.store(VecDeque::new()),
            prompt: cx.signal(None),
            open: cx.signal(false),
            apply_to_all: cx.signal(false),
        }
    }
}

/// Queues a conflict of job `id`; the job waits for `reply`.
pub fn ask(
    runtime: &mut Runtime,
    store: Store,
    id: JobId,
    target: PathBuf,
    reply: oneshot::Sender<ConflictReply>,
) {
    store.conflicts.queue.update(runtime, |queue| {
        queue.push_back(PendingConflict { id, target, reply });
    });
    if store.conflicts.prompt.with(runtime, Option::is_none) {
        show_next(runtime, store);
    }
}

/// A button of the dialog: answers the front conflict and shows the next.
pub fn answer(runtime: &mut Runtime, store: Store, policy: ConflictPolicy) {
    let apply_to_all = store.conflicts.apply_to_all.get(runtime);
    if let Some(pending) = store.conflicts.queue.update(runtime, VecDeque::pop_front) {
        let _ = pending.reply.send(ConflictReply {
            policy,
            apply_to_all,
        });
    }
    show_next(runtime, store);
}

/// Drops the conflicts of a job that ended, so the dialog does not ask for it.
pub fn forget_job(runtime: &mut Runtime, store: Store, id: JobId) {
    let front_gone = store.conflicts.queue.update(runtime, |queue| {
        let front_gone = queue.front().is_some_and(|pending| pending.id == id);
        queue.retain(|pending| pending.id != id);
        front_gone
    });
    if front_gone {
        show_next(runtime, store);
    }
}

/// Closing the dialog with Escape or the scrim cancels the job it asked for.
pub fn follow_dismissal(cx: &mut Cx, store: Store) {
    let conflicts = store.conflicts;
    cx.effect(move |runtime| {
        if conflicts.open.get(runtime) || conflicts.prompt.with_untracked(runtime, Option::is_none)
        {
            return;
        }
        drop(conflicts.queue.update(runtime, VecDeque::pop_front));
        show_next(runtime, store);
    });
}

fn show_next(runtime: &mut Runtime, store: Store) {
    let conflicts = store.conflicts;
    let next = conflicts.queue.with(runtime, |queue| {
        queue.front().map(|pending| ConflictPrompt {
            name: file_name(&pending.target),
            folder: pending.target.parent().map(file_name).unwrap_or_default(),
        })
    });
    let open = next.is_some();
    if next.is_some() {
        conflicts.apply_to_all.set(runtime, false);
    }
    conflicts.prompt.set(runtime, next);
    if conflicts
        .open
        .with_untracked(runtime, |shown| *shown != open)
    {
        conflicts.open.set(runtime, open);
    }
}

fn file_name(path: &std::path::Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}
