#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{
    path::Path,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

use crownos_files_core::{
    model::timestamp::Timestamp,
    thumbs::{
        Decoded, Frame, ThumbError, ThumbKey, ThumbSize, ThumbnailEvent, ThumbnailService,
        ThumbnailSource,
    },
};
use tokio::{sync::mpsc::Receiver, time::timeout};
use tokio_util::sync::CancellationToken;

const PATIENCE: Duration = Duration::from_secs(10);
const QUIET: Duration = Duration::from_millis(300);
const SLOW: &str = "/slow";

/// A source that blocks on [`SLOW`] until released and counts what it made.
#[derive(Default)]
struct Gate {
    released: Mutex<bool>,
    changed: Condvar,
    started: Mutex<Vec<ThumbKey>>,
}

#[derive(Clone, Default)]
struct GatedSource(Arc<Gate>);

impl GatedSource {
    fn release(&self) {
        *self.0.released.lock().unwrap() = true;
        self.0.changed.notify_all();
    }

    async fn wait_started(&self, count: usize) {
        timeout(PATIENCE, async {
            while self.0.started.lock().unwrap().len() < count {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("the source was never called");
    }

    fn started(&self) -> Vec<ThumbKey> {
        self.0.started.lock().unwrap().clone()
    }
}

impl ThumbnailSource for GatedSource {
    fn produce(&self, key: &ThumbKey, _: &CancellationToken) -> Result<Decoded, ThumbError> {
        self.0.started.lock().unwrap().push(key.clone());
        if &*key.path == Path::new(SLOW) {
            let mut released = self.0.released.lock().unwrap();
            while !*released {
                released = self.0.changed.wait(released).unwrap();
            }
        }
        let edge = key.size.edge();
        Ok(Frame::new(edge, 1, vec![0; edge as usize * 4])
            .unwrap()
            .into())
    }
}

fn key(path: &str, size: ThumbSize) -> ThumbKey {
    ThumbKey {
        path: Path::new(path).into(),
        modified: Timestamp::from_parts(1, 0),
        size,
    }
}

async fn next_ready(events: &mut Receiver<ThumbnailEvent>) -> ThumbKey {
    match timeout(PATIENCE, events.recv()).await.unwrap().unwrap() {
        ThumbnailEvent::Ready(thumbnail) => {
            assert_eq!(thumbnail.width, thumbnail.key.size.edge());
            thumbnail.key
        }
        ThumbnailEvent::Failed { key, error } => panic!("{key:?} failed: {error}"),
    }
}

async fn assert_quiet(events: &mut Receiver<ThumbnailEvent>) {
    assert!(
        timeout(QUIET, events.recv()).await.is_err(),
        "unexpected event"
    );
}

#[tokio::test]
async fn a_newer_request_for_the_same_file_supersedes_the_running_one() {
    let source = GatedSource::default();
    let (service, mut events) = ThumbnailService::with_source(source.clone(), 1);
    service.request(key(SLOW, ThumbSize::Normal));
    source.wait_started(1).await;
    service.request(key(SLOW, ThumbSize::Large));
    source.release();
    assert_eq!(next_ready(&mut events).await, key(SLOW, ThumbSize::Large));
    assert_quiet(&mut events).await;
}

#[tokio::test]
async fn the_newest_requests_are_decoded_first_and_duplicates_once() {
    let source = GatedSource::default();
    let (service, mut events) = ThumbnailService::with_source(source.clone(), 1);
    service.request(key(SLOW, ThumbSize::Normal));
    source.wait_started(1).await;
    for path in ["/a", "/b", "/c", "/a"] {
        service.request(key(path, ThumbSize::Normal));
    }
    source.release();
    let mut order = Vec::new();
    for _ in 0..4 {
        order.push(next_ready(&mut events).await.path);
    }
    let expected: Vec<Arc<Path>> = [SLOW, "/a", "/c", "/b"]
        .map(|path| Path::new(path).into())
        .into();
    assert_eq!(order, expected);
    assert_quiet(&mut events).await;
    assert_eq!(source.started().len(), 4);
}

#[tokio::test]
async fn cancelling_everything_drops_queued_and_running_requests() {
    let source = GatedSource::default();
    let (service, mut events) = ThumbnailService::with_source(source.clone(), 1);
    service.request(key(SLOW, ThumbSize::Normal));
    source.wait_started(1).await;
    service.request(key("/a", ThumbSize::Normal));
    service.request(key("/b", ThumbSize::Normal));
    service.cancel_all();
    source.release();
    assert_quiet(&mut events).await;
    assert_eq!(source.started().len(), 1);

    service.request(key("/c", ThumbSize::Normal));
    assert_eq!(next_ready(&mut events).await, key("/c", ThumbSize::Normal));
}

#[tokio::test]
async fn two_workers_decode_side_by_side() {
    let source = GatedSource::default();
    let (service, mut events) = ThumbnailService::with_source(source.clone(), 2);
    service.request(key(SLOW, ThumbSize::Normal));
    source.wait_started(1).await;
    service.request(key("/fast", ThumbSize::Normal));
    assert_eq!(
        next_ready(&mut events).await,
        key("/fast", ThumbSize::Normal)
    );
    source.release();
    assert_eq!(next_ready(&mut events).await, key(SLOW, ThumbSize::Normal));
}
