use std::path::Path;

use tokio::{
    runtime::Handle,
    sync::{mpsc::Sender, oneshot},
};
use tokio_util::sync::CancellationToken;

use crate::jobs::{
    conflict::{ConflictPolicy, ConflictReply},
    error::JobError,
    event::JobEvent,
    job::{JobId, JobKind},
    progress::Progress,
};

/// What a job needs while it runs on a blocking thread.
pub(crate) struct JobContext {
    pub id: JobId,
    pub kind: JobKind,
    pub cancel: CancellationToken,
    pub progress: Progress,
    events: Sender<JobEvent>,
    runtime: Handle,
    remembered: Option<ConflictPolicy>,
}

impl JobContext {
    pub fn new(
        id: JobId,
        kind: JobKind,
        cancel: CancellationToken,
        events: Sender<JobEvent>,
        runtime: Handle,
    ) -> Self {
        Self {
            id,
            kind,
            cancel,
            progress: Progress::new(id, events.clone()),
            events,
            runtime,
            remembered: None,
        }
    }

    pub fn check(&self) -> Result<(), JobError> {
        check(&self.cancel)
    }

    pub fn send(&self, event: JobEvent) {
        let _ = self.events.blocking_send(event);
    }

    pub fn start(&self, total_bytes: u64, total_items: u64) {
        self.send(JobEvent::Started {
            id: self.id,
            kind: self.kind,
            total_bytes,
            total_items,
        });
    }

    /// Asks the UI what to do about an existing `target`, unless an earlier
    /// answer was meant for all conflicts.
    pub fn resolve(&mut self, source: &Path, target: &Path) -> Result<ConflictPolicy, JobError> {
        if let Some(policy) = self.remembered {
            return Ok(policy);
        }
        let (reply, answer) = oneshot::channel::<ConflictReply>();
        self.send(JobEvent::Conflict {
            id: self.id,
            source: source.to_path_buf(),
            target: target.to_path_buf(),
            reply,
        });
        let cancel = &self.cancel;
        let answer = self.runtime.block_on(async {
            tokio::select! {
                answer = answer => answer.ok(),
                () = cancel.cancelled() => None,
            }
        });
        let answer = answer.ok_or(JobError::Cancelled)?;
        if answer.apply_to_all {
            self.remembered = Some(answer.policy);
        }
        Ok(answer.policy)
    }
}

pub(crate) fn check(cancel: &CancellationToken) -> Result<(), JobError> {
    if cancel.is_cancelled() {
        Err(JobError::Cancelled)
    } else {
        Ok(())
    }
}
