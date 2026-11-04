//! The crownos-preview client behind quick look: started on demand, fed
//! the folder's previewable files, and listened to for navigation.

use std::{
    ffi::OsString,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use crownos_ipc::{EventMsg, adapter::tokio::AsyncClient};
use crownos_preview::protocol;
use tokio::{sync::mpsc, time::timeout};

use crate::util::process::spawn_detached;

const PREVIEW_BINARY: &str = "crownos-preview";
const CONNECT_ATTEMPTS: u32 = 10;
const FIRST_RETRY_DELAY: Duration = Duration::from_millis(20);
const MAX_RETRY_DELAY: Duration = Duration::from_millis(400);
const REPLY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewEvent {
    /// The viewer stepped to entry `index` of the list last shown.
    Navigated { index: u32 },
    /// The preview went away, or could not be shown.
    Closed,
}

#[derive(Debug)]
enum Request {
    Show { paths: Vec<String>, index: u32 },
    Select(u32),
    Close,
}

#[derive(Debug, Clone)]
enum Endpoint {
    /// The session's service, started when it is not running.
    Session,
    /// A service bound in `dir`, never started by the client.
    Directory(PathBuf),
}

impl Endpoint {
    fn connect(&self) -> Result<AsyncClient, crownos_ipc::Error> {
        match self {
            Self::Session => AsyncClient::connect(protocol::SERVICE),
            Self::Directory(dir) => AsyncClient::connect_in(dir, protocol::SERVICE),
        }
    }
}

/// Handle to the quick-look session task. Dropping every clone ends it.
#[derive(Debug, Clone)]
pub struct QuickLook {
    requests: mpsc::UnboundedSender<Request>,
    open: Arc<AtomicBool>,
}

impl QuickLook {
    /// Starts the session task on the current tokio runtime.
    pub fn spawn() -> (Self, mpsc::UnboundedReceiver<PreviewEvent>) {
        Self::spawn_with(Endpoint::Session)
    }

    /// Like [`QuickLook::spawn`] against a service bound in `dir`.
    pub fn spawn_in(dir: impl Into<PathBuf>) -> (Self, mpsc::UnboundedReceiver<PreviewEvent>) {
        Self::spawn_with(Endpoint::Directory(dir.into()))
    }

    fn spawn_with(endpoint: Endpoint) -> (Self, mpsc::UnboundedReceiver<PreviewEvent>) {
        let (requests, inbox) = mpsc::unbounded_channel();
        let (events, receiver) = mpsc::unbounded_channel();
        let open = Arc::new(AtomicBool::new(false));
        let session = Session {
            endpoint,
            client: None,
            open: Arc::clone(&open),
            events,
        };
        tokio::spawn(session.run(inbox));
        (Self { requests, open }, receiver)
    }

    /// Shows `paths[index]`; the viewer's arrow keys step through `paths`.
    pub fn show(&self, paths: Vec<String>, index: u32) {
        self.requests.send(Request::Show { paths, index }).ok();
    }

    /// Moves the open preview to `paths[index]` of the list last shown.
    pub fn select(&self, index: u32) {
        self.requests.send(Request::Select(index)).ok();
    }

    pub fn close(&self) {
        self.requests.send(Request::Close).ok();
    }

    pub fn is_open(&self) -> bool {
        self.open.load(Ordering::Acquire)
    }
}

enum Wake {
    Request(Option<Request>),
    Inbound(Result<(), crownos_ipc::Error>),
}

struct Session {
    endpoint: Endpoint,
    client: Option<AsyncClient>,
    open: Arc<AtomicBool>,
    events: mpsc::UnboundedSender<PreviewEvent>,
}

impl Session {
    async fn run(mut self, mut inbox: mpsc::UnboundedReceiver<Request>) {
        loop {
            let wake = match self.client.as_mut() {
                Some(client) => tokio::select! {
                    request = inbox.recv() => Wake::Request(request),
                    inbound = client.pump() => Wake::Inbound(inbound),
                },
                None => Wake::Request(inbox.recv().await),
            };
            match wake {
                Wake::Request(Some(request)) => self.handle(request).await,
                Wake::Request(None) => break,
                Wake::Inbound(Ok(())) => self.drain_events(),
                Wake::Inbound(Err(_)) => self.disconnect(),
            }
        }
    }

    async fn handle(&mut self, request: Request) {
        match request {
            Request::Show { paths, index } => {
                let shown = self.show(paths, index).await.unwrap_or_else(|_| {
                    self.client = None;
                    false
                });
                self.set_open(shown);
                if !shown {
                    self.events.send(PreviewEvent::Closed).ok();
                }
            }
            Request::Select(index) => {
                if let Some(client) = self.client.as_mut()
                    && self.open.load(Ordering::Acquire)
                    && client
                        .notify::<protocol::select>(&protocol::select { index }, Vec::new())
                        .await
                        .is_err()
                {
                    self.disconnect();
                }
            }
            Request::Close => {
                if let Some(client) = self.client.as_mut()
                    && client
                        .notify::<protocol::close>(&protocol::close {}, Vec::new())
                        .await
                        .is_err()
                {
                    self.disconnect();
                }
                self.set_open(false);
            }
        }
    }

    /// Sends `open_list`, reconnecting once when the connection held went
    /// stale, as after the viewer restarted.
    async fn show(&mut self, paths: Vec<String>, index: u32) -> Result<bool, crownos_ipc::Error> {
        let request = protocol::open_list { paths, index };
        if let Some(client) = self.client.as_mut() {
            match open_list(client, &request).await {
                Ok(shown) => return Ok(shown),
                Err(_) => self.client = None,
            }
        }
        let first = request.paths.get(index as usize).map(String::as_str);
        let client = self.client.insert(self.connect(first).await?);
        open_list(client, &request).await
    }

    /// Connects and subscribes, starting crownos-preview on `path` first
    /// when the session's service is not running.
    async fn connect(&self, path: Option<&str>) -> Result<AsyncClient, crownos_ipc::Error> {
        let mut client = match self.endpoint.connect() {
            Ok(client) => client,
            Err(crownos_ipc::Error::ServiceUnavailable { .. })
                if matches!(self.endpoint, Endpoint::Session) =>
            {
                self.start_service(path).await?
            }
            Err(error) => return Err(error),
        };
        client.subscribe::<protocol::Navigated>().await?;
        client.subscribe::<protocol::Closed>().await?;
        Ok(client)
    }

    async fn start_service(&self, path: Option<&str>) -> Result<AsyncClient, crownos_ipc::Error> {
        let argv: Vec<OsString> = std::iter::once(PREVIEW_BINARY)
            .chain(path)
            .map(OsString::from)
            .collect();
        tokio::task::spawn_blocking(move || spawn_detached(&argv, None))
            .await
            .map_err(|error| crownos_ipc::Error::Io(error.into()))??;
        let mut delay = FIRST_RETRY_DELAY;
        let mut last = crownos_ipc::Error::Timeout;
        for _ in 0..CONNECT_ATTEMPTS {
            tokio::time::sleep(delay).await;
            match self.endpoint.connect() {
                Ok(client) => return Ok(client),
                Err(error) => last = error,
            }
            delay = (delay * 2).min(MAX_RETRY_DELAY);
        }
        Err(last)
    }

    fn drain_events(&mut self) {
        let Some(client) = self.client.as_mut() else {
            return;
        };
        let inner = client.client_mut();
        while let Some(message) = inner.next_event_raw() {
            if message.selector == protocol::Navigated::SELECTOR {
                if let Ok(navigated) =
                    crownos_ipc::postcard::from_bytes::<protocol::Navigated>(&message.payload)
                {
                    self.events
                        .send(PreviewEvent::Navigated {
                            index: navigated.index,
                        })
                        .ok();
                }
            } else if message.selector == protocol::Closed::SELECTOR {
                self.open.store(false, Ordering::Release);
                self.events.send(PreviewEvent::Closed).ok();
            }
        }
        if inner.is_closed() {
            self.disconnect();
        }
    }

    fn disconnect(&mut self) {
        self.client = None;
        if self.open.swap(false, Ordering::AcqRel) {
            self.events.send(PreviewEvent::Closed).ok();
        }
    }

    fn set_open(&self, open: bool) {
        self.open.store(open, Ordering::Release);
    }
}

async fn open_list(
    client: &mut AsyncClient,
    request: &protocol::open_list,
) -> Result<bool, crownos_ipc::Error> {
    timeout(
        REPLY_TIMEOUT,
        client.call::<protocol::open_list>(request, Vec::new()),
    )
    .await
    .map_err(|_| crownos_ipc::Error::Timeout)?
}
