use std::time::Duration;

use crownos_files_core::jobs::{
    ConflictReply, Job, JobError, JobEvent, Outcome, Priority, Reversible, Scheduler,
};
use tokio::{sync::mpsc, time::timeout};

const EVENT_CAPACITY: usize = 64;
const PATIENCE: Duration = Duration::from_secs(60);

#[derive(Debug, Default)]
pub struct Report {
    pub outcome: Option<Outcome>,
    pub undo: Option<Reversible>,
    pub totals: Option<(u64, u64)>,
    pub error: Option<JobError>,
    pub needs_privilege: bool,
    pub conflicts: usize,
}

pub struct Jobs {
    pub scheduler: Scheduler,
    events: mpsc::Receiver<JobEvent>,
}

impl Jobs {
    pub fn new() -> Self {
        let (sender, events) = mpsc::channel(EVENT_CAPACITY);
        Self {
            scheduler: Scheduler::spawn(sender),
            events,
        }
    }

    /// Runs `job`, answering every conflict with `answer` (or dropping the
    /// reply when `None`).
    pub async fn run(&mut self, job: Job, answer: Option<ConflictReply>) -> Report {
        let id = self.scheduler.submit(job, Priority::Normal);
        self.wait(id, answer).await
    }

    pub async fn wait(
        &mut self,
        id: crownos_files_core::jobs::JobId,
        answer: Option<ConflictReply>,
    ) -> Report {
        let mut report = Report::default();
        timeout(PATIENCE, async {
            while let Some(event) = self.events.recv().await {
                match event {
                    JobEvent::Started {
                        id: started,
                        total_bytes,
                        total_items,
                        ..
                    } if started == id => report.totals = Some((total_bytes, total_items)),
                    JobEvent::Conflict {
                        id: asking, reply, ..
                    } if asking == id => {
                        report.conflicts += 1;
                        if let Some(answer) = answer {
                            reply.send(answer).unwrap();
                        }
                    }
                    JobEvent::Failed {
                        id: failed,
                        error,
                        needs_privilege,
                    } if failed == id => {
                        report.error = Some(error);
                        report.needs_privilege = needs_privilege;
                    }
                    JobEvent::Finished {
                        id: finished,
                        outcome,
                        undo,
                    } if finished == id => {
                        report.outcome = Some(outcome);
                        report.undo = undo;
                        return;
                    }
                    _ => {}
                }
            }
        })
        .await
        .expect("the job finished in time");
        report
    }
}
