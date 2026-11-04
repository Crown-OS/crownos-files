use std::path::PathBuf;

use tokio::sync::oneshot;

use crate::jobs::{
    conflict::ConflictReply,
    error::JobError,
    job::{JobId, JobKind},
    undo::Reversible,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Completed,
    Cancelled,
    Failed,
}

/// What a running job reports. `Progress` may be dropped when the receiver
/// lags; every other event is delivered.
#[derive(Debug)]
pub enum JobEvent {
    /// The job measured its work; not sent for jobs cancelled while queued.
    Started {
        id: JobId,
        kind: JobKind,
        total_bytes: u64,
        total_items: u64,
    },
    Progress {
        id: JobId,
        done_bytes: u64,
        done_items: u64,
        current: PathBuf,
    },
    /// `target` exists. The job waits for `reply`; dropping it cancels the job.
    Conflict {
        id: JobId,
        source: PathBuf,
        target: PathBuf,
        reply: oneshot::Sender<ConflictReply>,
    },
    Failed {
        id: JobId,
        error: JobError,
        needs_privilege: bool,
    },
    /// Always the last event of a job. `undo` records what was done, even
    /// when the job stopped part-way.
    Finished {
        id: JobId,
        outcome: Outcome,
        undo: Option<Reversible>,
    },
}
