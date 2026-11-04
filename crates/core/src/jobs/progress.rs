use std::{path::Path, time::Duration};

use tokio::sync::mpsc::Sender;

use crate::{
    jobs::{event::JobEvent, job::JobId},
    util::throttle::Throttle,
};

pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

/// Running totals of a job, reported at most every [`PROGRESS_INTERVAL`]
/// with `try_send`, so a slow receiver never stalls the work.
pub(crate) struct Progress {
    id: JobId,
    events: Sender<JobEvent>,
    throttle: Throttle,
    done_bytes: u64,
    done_items: u64,
}

impl Progress {
    pub fn new(id: JobId, events: Sender<JobEvent>) -> Self {
        Self {
            id,
            events,
            throttle: Throttle::new(PROGRESS_INTERVAL),
            done_bytes: 0,
            done_items: 0,
        }
    }

    pub fn add_bytes(&mut self, bytes: u64, current: &Path) {
        self.done_bytes += bytes;
        self.report(current);
    }

    pub fn add_item(&mut self, current: &Path) {
        self.done_items += 1;
        self.report(current);
    }

    pub fn skip(&mut self, bytes: u64, items: u64) {
        self.done_bytes += bytes;
        self.done_items += items;
    }

    fn report(&mut self, current: &Path) {
        if !self.throttle.ready() {
            return;
        }
        let _ = self.events.try_send(JobEvent::Progress {
            id: self.id,
            done_bytes: self.done_bytes,
            done_items: self.done_items,
            current: current.to_path_buf(),
        });
    }
}
