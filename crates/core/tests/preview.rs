#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use crownos_files_core::preview::{PreviewEvent, QuickLook};
use crownos_ipc::{RemoteError, ServiceBuilder, adapter::tokio::serve_with_outbox};
use crownos_preview::protocol;
use tokio::{
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    time::timeout,
};

const PATIENCE: Duration = Duration::from_secs(10);

#[derive(Debug, PartialEq, Eq)]
enum Seen {
    OpenList(Vec<String>, u32),
    Select(u32),
    Close,
}

enum Push {
    Navigated(u32, String),
    Closed,
}

/// Stands in for crownos-preview: records calls and steps the viewer on `select`.
struct FakePreview {
    seen: UnboundedSender<Seen>,
    list: Vec<String>,
    pending: Vec<Push>,
}

impl protocol::Handler for FakePreview {
    fn open(&mut self, _: String) -> Result<bool, RemoteError> {
        Ok(true)
    }

    fn edit(&mut self, _: String) -> Result<bool, RemoteError> {
        Ok(true)
    }

    fn close(&mut self) {
        self.seen.send(Seen::Close).unwrap();
    }

    fn open_list(&mut self, paths: Vec<String>, index: u32) -> Result<bool, RemoteError> {
        let shown = (index as usize) < paths.len();
        self.list.clone_from(&paths);
        self.seen.send(Seen::OpenList(paths, index)).unwrap();
        Ok(shown)
    }

    fn select(&mut self, index: u32) {
        self.seen.send(Seen::Select(index)).unwrap();
        let path = self.list.get(index as usize).cloned().unwrap_or_default();
        self.pending.push(Push::Navigated(index, path));
    }
}

fn emit(server: &mut crownos_ipc::Server, push: Push) {
    match push {
        Push::Navigated(index, path) => server.emit(&protocol::Navigated { index, path }),
        Push::Closed => server.emit(&protocol::Closed {
            path: String::new(),
        }),
    }
    .unwrap();
}

async fn next<T>(receiver: &mut UnboundedReceiver<T>) -> T {
    timeout(PATIENCE, receiver.recv()).await.unwrap().unwrap()
}

async fn wait_until(mut condition: impl FnMut() -> bool) {
    timeout(PATIENCE, async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn quick_look_shows_lists_follows_navigation_and_closes() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = ServiceBuilder::new(protocol::SERVICE)
        .directory(dir.path())
        .build()
        .unwrap();
    let (seen, mut calls) = mpsc::unbounded_channel();
    let (pushes, mut outbox) = mpsc::unbounded_channel::<Push>();
    tokio::spawn(async move {
        let mut preview = FakePreview {
            seen,
            list: Vec::new(),
            pending: Vec::new(),
        };
        serve_with_outbox(
            &mut server,
            |server, peer, message| {
                protocol::dispatch(&mut preview, server, peer, message)?;
                preview
                    .pending
                    .drain(..)
                    .for_each(|push| emit(server, push));
                Ok(true)
            },
            |cx| outbox.poll_recv(cx),
            emit,
        )
        .await
        .unwrap();
    });

    let (quick, mut events) = QuickLook::spawn_in(dir.path());
    assert!(!quick.is_open());
    let paths = vec!["/a.png".to_owned(), "/b.md".to_owned(), "/c.rs".to_owned()];
    quick.show(paths.clone(), 1);
    assert_eq!(next(&mut calls).await, Seen::OpenList(paths.clone(), 1));
    wait_until(|| quick.is_open()).await;

    quick.select(2);
    assert_eq!(next(&mut calls).await, Seen::Select(2));
    assert_eq!(
        next(&mut events).await,
        PreviewEvent::Navigated { index: 2 }
    );

    pushes.send(Push::Closed).unwrap();
    assert_eq!(next(&mut events).await, PreviewEvent::Closed);
    assert!(!quick.is_open());

    quick.select(0);
    quick.show(paths.clone(), 7);
    assert_eq!(next(&mut calls).await, Seen::OpenList(paths.clone(), 7));
    assert_eq!(next(&mut events).await, PreviewEvent::Closed);
    assert!(!quick.is_open());

    quick.show(paths.clone(), 0);
    assert_eq!(next(&mut calls).await, Seen::OpenList(paths, 0));
    wait_until(|| quick.is_open()).await;
    quick.close();
    assert_eq!(next(&mut calls).await, Seen::Close);
    wait_until(|| !quick.is_open()).await;
}

#[tokio::test]
async fn an_unreachable_service_reports_closed() {
    let dir = tempfile::tempdir().unwrap();
    let (quick, mut events) = QuickLook::spawn_in(dir.path());
    quick.show(vec!["/a.png".to_owned()], 0);
    assert_eq!(next(&mut events).await, PreviewEvent::Closed);
    assert!(!quick.is_open());
}
