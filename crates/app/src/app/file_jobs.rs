//! Copy, move, delete and trash jobs on the scheduler: submitting them,
//! following their progress for the toolbar, and what their end leaves
//! behind (undo, toasts, a cleared cut, a retry with privileges).

use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};

use crownui::prelude::{Cx, Runtime, Signal, Store as Cell};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::{
    app::{
        conflicts::{ask, forget_job},
        privilege::offer_privileges,
        virtual_folder::reload_virtual,
    },
    jobs::{Job, JobEvent, JobId, JobKind, Outcome, Priority, Scheduler, UndoStack},
    state::Store,
    util::hash::FastHashMap,
};
use crownos_files_devices::privileged::{PrivilegedClient, PrivilegedJob, PrivilegedRequest};

const EVENT_CAPACITY: usize = 64;

#[derive(Debug)]
pub enum JobCommand {
    Submit { job: Job, priority: Priority },
    Elevate(PrivilegedRequest),
    Cancel(JobId),
}

#[derive(Debug)]
pub enum JobUpdate {
    /// The id of the job submitted next; it precedes every event of that job.
    Submitted(JobId),
    Event(JobEvent),
}

/// Why a job runs, which decides what its end does.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Purpose {
    #[default]
    Plain,
    /// Reverses an earlier job; not undoable itself.
    Undo,
    /// Pastes a cut, which empties the clipboard once it succeeds.
    PasteCut,
    /// Runs through the privileged helper.
    Elevated,
}

/// A job as the UI knows it: what was asked and how far it got.
#[derive(Debug)]
struct Record {
    job: Option<Job>,
    purpose: Purpose,
    kind: JobKind,
    items: usize,
    total_bytes: u64,
    total_items: u64,
    done_bytes: u64,
    done_items: u64,
    failed: bool,
}

impl Record {
    fn fraction(&self) -> f32 {
        match (self.total_bytes, self.total_items) {
            (0, 0) => 0.0,
            (0, items) => self.done_items as f32 / items as f32,
            (bytes, _) => self.done_bytes as f32 / bytes as f32,
        }
        .clamp(0.0, 1.0)
    }
}

/// One running job in the toolbar's popover.
#[derive(Debug, Clone, PartialEq)]
pub struct JobRow {
    pub id: JobId,
    pub label: String,
    pub fraction: f32,
}

#[derive(Clone, Copy)]
pub struct Jobs {
    pub sender: Cell<Option<UnboundedSender<JobCommand>>>,
    awaiting: Cell<VecDeque<Record>>,
    records: Cell<FastHashMap<JobId, Record>>,
    pub running: Signal<Vec<JobRow>>,
    pub undo: Cell<UndoStack>,
}

impl Jobs {
    pub fn new(cx: &mut Cx) -> Self {
        Self {
            sender: cx.store(None),
            awaiting: cx.store(VecDeque::new()),
            records: cx.store(FastHashMap::default()),
            running: cx.signal(Vec::new()),
            undo: cx.store(UndoStack::default()),
        }
    }

    fn send(self, runtime: &Runtime, command: JobCommand) {
        self.sender.with(runtime, |sender| {
            if let Some(sender) = sender {
                let _ = sender.send(command);
            }
        });
    }
}

/// Runs jobs on the scheduler, and privileged ones through pkexec, until the
/// window closes.
pub async fn run_jobs(
    updates: UnboundedSender<JobUpdate>,
    mut commands: UnboundedReceiver<JobCommand>,
) {
    let (events, mut job_events) = mpsc::channel(EVENT_CAPACITY);
    let scheduler = Scheduler::spawn(events.clone());
    let helper = PrivilegedClient::default();
    let mut elevated: Vec<PrivilegedJob> = Vec::new();
    loop {
        tokio::select! {
            command = commands.recv() => {
                let submitted = match command {
                    Some(JobCommand::Submit { job, priority }) => Some(scheduler.submit(job, priority)),
                    Some(JobCommand::Elevate(request)) => {
                        let job = helper.submit(request, events.clone());
                        let id = job.id;
                        elevated.push(job);
                        Some(id)
                    }
                    Some(JobCommand::Cancel(id)) => {
                        scheduler.cancel(id);
                        elevated.iter().filter(|job| job.id == id).for_each(|job| job.cancel.cancel());
                        None
                    }
                    None => break,
                };
                if let Some(id) = submitted && updates.send(JobUpdate::Submitted(id)).is_err() {
                    break;
                }
            }
            Some(event) = job_events.recv() => {
                if let JobEvent::Finished { id, .. } = &event {
                    elevated.retain(|job| job.id != *id);
                }
                if updates.send(JobUpdate::Event(event)).is_err() {
                    break;
                }
            }
        }
    }
}

/// How many top-level items `job` works on, as its toasts count them.
fn item_count(job: &Job) -> usize {
    match job {
        Job::Copy(transfers) | Job::Move(transfers) => transfers.len(),
        Job::Delete(paths) | Job::Trash(paths) => paths.len(),
        Job::Restore(items) | Job::DeleteFromTrash(items) => items.len(),
        Job::EmptyTrash => 0,
    }
}

pub fn submit(runtime: &mut Runtime, store: Store, job: Job, purpose: Purpose) {
    if item_count(&job) == 0 && job != Job::EmptyTrash {
        return;
    }
    let priority = if purpose == Purpose::Undo {
        Priority::High
    } else {
        Priority::Normal
    };
    let record = Record {
        kind: job.kind(),
        items: item_count(&job),
        job: Some(job.clone()),
        ..Record::new(purpose)
    };
    store
        .jobs
        .awaiting
        .update(runtime, |awaiting| awaiting.push_back(record));
    store
        .jobs
        .send(runtime, JobCommand::Submit { job, priority });
}

/// Runs `request` through the privileged helper, as a job of `kind`.
pub fn submit_elevated(
    runtime: &mut Runtime,
    store: Store,
    request: PrivilegedRequest,
    kind: JobKind,
    items: usize,
) {
    let record = Record {
        kind,
        items,
        ..Record::new(Purpose::Elevated)
    };
    store
        .jobs
        .awaiting
        .update(runtime, |awaiting| awaiting.push_back(record));
    store.jobs.send(runtime, JobCommand::Elevate(request));
}

impl Record {
    fn new(purpose: Purpose) -> Self {
        Self {
            job: None,
            purpose,
            kind: JobKind::Copy,
            items: 0,
            total_bytes: 0,
            total_items: 0,
            done_bytes: 0,
            done_items: 0,
            failed: false,
        }
    }
}

pub fn cancel_job(runtime: &mut Runtime, store: Store, id: JobId) {
    store.jobs.send(runtime, JobCommand::Cancel(id));
}

/// Ctrl+Z: reverses the latest undoable operation.
pub fn undo(cx: &mut Cx, store: Store) {
    match store.jobs.undo.update(cx, UndoStack::undo) {
        Some(job) => submit(cx, store, job, Purpose::Undo),
        None => store.toasts.show(cx, "Nothing to undo"),
    }
}

pub fn finish_jobs(cx: &mut Cx, store: Store, updates: Vec<JobUpdate>) {
    for update in updates {
        match update {
            JobUpdate::Submitted(id) => {
                if let Some(record) = store.jobs.awaiting.update(cx, VecDeque::pop_front) {
                    store
                        .jobs
                        .records
                        .update(cx, |records| records.insert(id, record));
                }
            }
            JobUpdate::Event(event) => apply_event(cx, store, event),
        }
    }
    publish_rows(cx, store);
}

fn apply_event(cx: &mut Cx, store: Store, event: JobEvent) {
    let records = store.jobs.records;
    match event {
        JobEvent::Started {
            id,
            kind,
            total_bytes,
            total_items,
        } => records.update(cx, |records| {
            if let Some(record) = records.get_mut(&id) {
                record.kind = kind;
                record.total_bytes = total_bytes;
                record.total_items = total_items;
            }
        }),
        JobEvent::Progress {
            id,
            done_bytes,
            done_items,
            ..
        } => records.update(cx, |records| {
            if let Some(record) = records.get_mut(&id) {
                record.done_bytes = done_bytes;
                record.done_items = done_items;
            }
        }),
        JobEvent::Conflict {
            id, target, reply, ..
        } => ask(cx, store, id, target, reply),
        JobEvent::Failed {
            id,
            error,
            needs_privilege,
        } => {
            let retry = records.update(cx, |records| {
                let record = records.get_mut(&id)?;
                record.failed = true;
                needs_privilege.then(|| record.job.clone()).flatten()
            });
            match retry {
                Some(job) => offer_privileges(cx, store, job, error.to_string()),
                None => store.toasts.show(cx, error.to_string()),
            }
        }
        JobEvent::Finished { id, outcome, undo } => {
            forget_job(cx, store, id);
            let Some(record) = records.update(cx, |records| records.remove(&id)) else {
                return;
            };
            if let Some(undo) = undo.filter(|_| record.purpose != Purpose::Undo) {
                store.jobs.undo.update(cx, |stack| stack.push(undo));
            }
            if outcome == Outcome::Completed && !record.failed {
                store
                    .toasts
                    .show(cx, done_message(record.kind, record.items));
                if record.purpose == Purpose::PasteCut {
                    cx.set_clipboard(Vec::new());
                }
            }
            reload_virtual(cx, store);
        }
    }
}

fn publish_rows(runtime: &mut Runtime, store: Store) {
    let rows: Vec<JobRow> = store.jobs.records.with(runtime, |records| {
        let mut rows: Vec<JobRow> = records
            .iter()
            .map(|(&id, record)| JobRow {
                id,
                label: running_label(record.kind, record.items),
                fraction: record.fraction(),
            })
            .collect();
        rows.sort_by_key(|row| row.id);
        rows
    });
    if store
        .jobs
        .running
        .with_untracked(runtime, |running| *running != rows)
    {
        store.jobs.running.set(runtime, rows);
    }
}

fn items(count: usize) -> String {
    match count {
        1 => "1 item".to_owned(),
        count => format!("{count} items"),
    }
}

pub fn running_label(kind: JobKind, count: usize) -> String {
    match kind {
        JobKind::Copy => format!("Copying {}", items(count)),
        JobKind::Move => format!("Moving {}", items(count)),
        JobKind::Delete | JobKind::DeleteFromTrash => format!("Deleting {}", items(count)),
        JobKind::Trash => format!("Moving {} to the Trash", items(count)),
        JobKind::Restore => format!("Restoring {}", items(count)),
        JobKind::EmptyTrash => "Emptying the Trash".to_owned(),
    }
}

pub fn done_message(kind: JobKind, count: usize) -> String {
    match kind {
        JobKind::Copy => format!("Copied {}", items(count)),
        JobKind::Move => format!("Moved {}", items(count)),
        JobKind::Delete | JobKind::DeleteFromTrash => format!("Deleted {}", items(count)),
        JobKind::Trash => format!("Moved {} to the Trash", items(count)),
        JobKind::Restore => format!("Restored {}", items(count)),
        JobKind::EmptyTrash => "Emptied the Trash".to_owned(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferMode {
    Copy,
    Move,
}

/// Copies or moves `sources` into `dir`; a move leaves out what is already there.
pub fn transfer(
    runtime: &mut Runtime,
    store: Store,
    sources: Vec<PathBuf>,
    dir: &Path,
    mode: TransferMode,
    purpose: Purpose,
) {
    let job = match mode {
        TransferMode::Move => Job::move_into(
            sources
                .into_iter()
                .filter(|source| source.parent() != Some(dir)),
            dir,
        ),
        TransferMode::Copy => Job::copy_into(sources, dir),
    };
    submit(runtime, store, job, purpose);
}
