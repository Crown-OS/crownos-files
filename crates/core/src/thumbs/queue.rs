//! The thumbnail service: newest requests first, a bounded set of decode
//! workers, and superseded or scrolled-away requests cancelled.

use std::{path::Path, sync::Arc};

use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc};
use tokio_util::sync::CancellationToken;

use crate::{
    thumbs::{
        cache::ThumbCache,
        key::{ThumbKey, Thumbnail, ThumbnailEvent},
        source::{CachedDecoder, ThumbnailSource},
    },
    util::hash::FastIndexMap,
};

pub const DECODE_WORKERS: usize = 2;
/// Requests remembered at once, a few pages of a dense grid; older ones are
/// cancelled, as they scrolled out of view long ago.
const TRACKED_REQUESTS: usize = 256;
const EVENT_CAPACITY: usize = 32;

#[derive(Debug)]
enum Command {
    Request(ThumbKey),
    Cancel(Arc<Path>),
    CancelAll,
}

/// Handle to the thumbnail service; dropping it stops the service and
/// cancels what is pending.
#[derive(Debug, Clone)]
pub struct ThumbnailService {
    commands: mpsc::UnboundedSender<Command>,
}

impl ThumbnailService {
    /// Starts the service on the current tokio runtime, backed by the
    /// shared cache and [`DECODE_WORKERS`] decoders.
    pub fn spawn(cache: ThumbCache) -> (Self, mpsc::Receiver<ThumbnailEvent>) {
        Self::with_source(CachedDecoder::new(cache), DECODE_WORKERS)
    }

    pub fn with_source(
        source: impl ThumbnailSource,
        workers: usize,
    ) -> (Self, mpsc::Receiver<ThumbnailEvent>) {
        let (commands, inbox) = mpsc::unbounded_channel();
        let (events, receiver) = mpsc::channel(EVENT_CAPACITY);
        let (finished, finished_inbox) = mpsc::unbounded_channel();
        let dispatcher = Dispatcher {
            source: Arc::new(source),
            tracked: FastIndexMap::default(),
            workers: Arc::new(Semaphore::new(workers.max(1))),
            events,
            finished,
            next_generation: 0,
        };
        tokio::spawn(dispatcher.run(inbox, finished_inbox));
        (Self { commands }, receiver)
    }

    /// Asks for a thumbnail. The latest request is served first, so request
    /// the prefetch margin before the visible rows.
    pub fn request(&self, key: ThumbKey) {
        self.commands.send(Command::Request(key)).ok();
    }

    pub fn cancel(&self, path: Arc<Path>) {
        self.commands.send(Command::Cancel(path)).ok();
    }

    /// Drops every pending request, as when the folder changes.
    pub fn cancel_all(&self) {
        self.commands.send(Command::CancelAll).ok();
    }
}

struct Tracked {
    key: ThumbKey,
    token: CancellationToken,
    generation: u64,
    running: bool,
}

struct Finished {
    path: Arc<Path>,
    generation: u64,
}

struct Dispatcher<S> {
    source: Arc<S>,
    /// In request order, the newest last.
    tracked: FastIndexMap<Arc<Path>, Tracked>,
    workers: Arc<Semaphore>,
    events: mpsc::Sender<ThumbnailEvent>,
    finished: mpsc::UnboundedSender<Finished>,
    next_generation: u64,
}

impl<S: ThumbnailSource> Dispatcher<S> {
    async fn run(
        mut self,
        mut inbox: mpsc::UnboundedReceiver<Command>,
        mut finished: mpsc::UnboundedReceiver<Finished>,
    ) {
        loop {
            let waiting = self.tracked.values().any(|tracked| !tracked.running);
            let workers = Arc::clone(&self.workers);
            tokio::select! {
                biased;
                command = inbox.recv() => match command {
                    Some(command) => self.apply(command),
                    None => break,
                },
                Some(done) = finished.recv() => self.finish(done),
                Ok(permit) = workers.acquire_owned(), if waiting => {
                    while let Ok(command) = inbox.try_recv() {
                        self.apply(command);
                    }
                    self.start_newest(permit);
                }
            }
        }
        self.tracked
            .values()
            .for_each(|tracked| tracked.token.cancel());
    }

    fn apply(&mut self, command: Command) {
        match command {
            Command::Request(key) => self.request(key),
            Command::Cancel(path) => {
                if let Some(tracked) = self.tracked.shift_remove(&path) {
                    tracked.token.cancel();
                }
            }
            Command::CancelAll => self
                .tracked
                .drain(..)
                .for_each(|(_, tracked)| tracked.token.cancel()),
        }
    }

    fn request(&mut self, key: ThumbKey) {
        if let Some(index) = self.tracked.get_index_of(&key.path) {
            if self.tracked[index].key == key {
                let newest = self.tracked.len() - 1;
                self.tracked.move_index(index, newest);
                return;
            }
            if let Some((_, superseded)) = self.tracked.shift_remove_index(index) {
                superseded.token.cancel();
            }
        }
        self.next_generation += 1;
        let tracked = Tracked {
            key,
            token: CancellationToken::new(),
            generation: self.next_generation,
            running: false,
        };
        self.tracked.insert(Arc::clone(&tracked.key.path), tracked);
        while self.tracked.len() > TRACKED_REQUESTS {
            if let Some((_, oldest)) = self.tracked.shift_remove_index(0) {
                oldest.token.cancel();
            }
        }
    }

    fn finish(&mut self, done: Finished) {
        if self
            .tracked
            .get(&done.path)
            .is_some_and(|tracked| tracked.generation == done.generation)
        {
            self.tracked.shift_remove(&done.path);
        }
    }

    fn start_newest(&mut self, permit: OwnedSemaphorePermit) {
        let Some(tracked) = self
            .tracked
            .values_mut()
            .rev()
            .find(|tracked| !tracked.running)
        else {
            return;
        };
        tracked.running = true;
        let key = tracked.key.clone();
        let token = tracked.token.clone();
        let generation = tracked.generation;
        let source = Arc::clone(&self.source);
        let events = self.events.clone();
        let finished = self.finished.clone();
        tokio::spawn(async move {
            let produced = tokio::task::spawn_blocking({
                let (key, token) = (key.clone(), token.clone());
                move || source.produce(&key, &token)
            })
            .await;
            drop(permit);
            if !token.is_cancelled() {
                let event = match produced.map_err(Into::into).and_then(|result| result) {
                    Ok(decoded) => ThumbnailEvent::Ready(Thumbnail::new(key.clone(), decoded)),
                    Err(error) => ThumbnailEvent::Failed {
                        key: key.clone(),
                        error,
                    },
                };
                events.send(event).await.ok();
            }
            finished
                .send(Finished {
                    path: key.path,
                    generation,
                })
                .ok();
        });
    }
}
