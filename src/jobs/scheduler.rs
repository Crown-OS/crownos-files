use std::{
    cmp::Ordering,
    collections::BinaryHeap,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use tokio::{
    runtime::Handle,
    sync::{Notify, mpsc::Sender},
    task::spawn_blocking,
};
use tokio_util::{
    sync::{CancellationToken, DropGuard},
    task::AbortOnDropHandle,
};

use crate::{
    jobs::{
        context::JobContext,
        event::JobEvent,
        execute::execute,
        job::{Job, JobId, Priority},
    },
    util::hash::FastHashMap,
};

/// File jobs that run at once; other work (sizes, thumbnails) has its own pools.
pub const FILE_WORKERS: usize = 3;

struct Queued {
    priority: Priority,
    id: JobId,
    job: Job,
    cancel: CancellationToken,
}

impl Ord for Queued {
    /// Higher priority first, then first come first served.
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.id.cmp(&self.id))
    }
}

impl PartialOrd for Queued {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Queued {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Queued {}

struct Shared {
    queue: Mutex<BinaryHeap<Queued>>,
    wake: Notify,
    tokens: Mutex<FastHashMap<JobId, CancellationToken>>,
    events: Sender<JobEvent>,
    root: CancellationToken,
}

impl Shared {
    fn pop(&self) -> Option<Queued> {
        let mut queue = lock(&self.queue);
        let next = queue.pop();
        if !queue.is_empty() {
            self.wake.notify_one();
        }
        next
    }
}

/// Runs file jobs on [`FILE_WORKERS`] workers, by priority. Every job's
/// events go to the `events` channel. Dropping the scheduler cancels all jobs.
pub struct Scheduler {
    shared: Arc<Shared>,
    _workers: Vec<AbortOnDropHandle<()>>,
    _cancel_on_drop: DropGuard,
}

impl Scheduler {
    /// Starts the workers on the current tokio runtime.
    pub fn spawn(events: Sender<JobEvent>) -> Self {
        let root = CancellationToken::new();
        let shared = Arc::new(Shared {
            queue: Mutex::default(),
            wake: Notify::new(),
            tokens: Mutex::default(),
            events,
            root: root.clone(),
        });
        let workers = (0..FILE_WORKERS)
            .map(|_| AbortOnDropHandle::new(tokio::spawn(work(shared.clone()))))
            .collect();
        Self {
            shared,
            _workers: workers,
            _cancel_on_drop: root.drop_guard(),
        }
    }

    pub fn submit(&self, job: Job, priority: Priority) -> JobId {
        let id = JobId::next();
        let cancel = self.shared.root.child_token();
        lock(&self.shared.tokens).insert(id, cancel.clone());
        lock(&self.shared.queue).push(Queued {
            priority,
            id,
            job,
            cancel,
        });
        self.shared.wake.notify_one();
        id
    }

    /// Cancels a queued or running job; it still reports `Finished`.
    pub fn cancel(&self, id: JobId) {
        if let Some(token) = lock(&self.shared.tokens).get(&id) {
            token.cancel();
        }
    }

    pub fn cancel_all(&self) {
        lock(&self.shared.tokens)
            .values()
            .for_each(CancellationToken::cancel);
    }
}

async fn work(shared: Arc<Shared>) {
    loop {
        let Some(Queued {
            id, job, cancel, ..
        }) = shared.pop()
        else {
            shared.wake.notified().await;
            continue;
        };
        let ctx = JobContext::new(
            id,
            job.kind(),
            cancel,
            shared.events.clone(),
            Handle::current(),
        );
        let _ = spawn_blocking(move || execute(job, ctx)).await;
        lock(&shared.tokens).remove(&id);
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queued(priority: Priority) -> Queued {
        Queued {
            priority,
            id: JobId::next(),
            job: Job::EmptyTrash,
            cancel: CancellationToken::new(),
        }
    }

    #[test]
    fn higher_priorities_run_first_then_oldest_first() {
        let mut heap = BinaryHeap::new();
        let low = queued(Priority::Low);
        let (first_normal, second_normal) = (queued(Priority::Normal), queued(Priority::Normal));
        let high = queued(Priority::High);
        let expected = [high.id, first_normal.id, second_normal.id, low.id];
        heap.extend([low, second_normal, high, first_normal]);
        let order: Vec<_> = std::iter::from_fn(|| heap.pop().map(|queued| queued.id)).collect();
        assert_eq!(order, expected);
    }
}
