//! Paired phones and tablets from crownconnect, for the sidebar and
//! "Send to device".

use std::{
    ops::ControlFlow,
    path::{Path, PathBuf},
    time::Duration,
};

use crownconnect_linux::ipc::proto::{self as protocol, DeviceClass, DeviceInfo};
pub use crownconnect_linux::ipc::proto::{Battery, DeviceId};
use crownos_ipc::{EventMsg, adapter::tokio::AsyncClient};
use serde::de::DeserializeOwned;
use tokio::{
    sync::{mpsc, oneshot, watch},
    time::{Instant, sleep_until, timeout},
};

const FIRST_RETRY_DELAY: Duration = Duration::from_millis(250);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(15);
const REPLY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Phone,
    Tablet,
    Watch,
    Computer,
}

impl From<DeviceClass> for DeviceKind {
    fn from(class: DeviceClass) -> Self {
        match class {
            DeviceClass::Phone => Self::Phone,
            DeviceClass::Tablet => Self::Tablet,
            DeviceClass::Watch => Self::Watch,
            DeviceClass::Computer => Self::Computer,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairedDevice {
    pub id: DeviceId,
    pub name: String,
    pub kind: DeviceKind,
    pub connected: bool,
    pub battery: Option<Battery>,
}

impl PairedDevice {
    fn from_info(info: &DeviceInfo) -> Self {
        Self {
            id: info.id,
            name: info.name.clone(),
            kind: info.class.into(),
            connected: info.connected,
            battery: info.battery,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectEvent {
    /// The paired devices, after any change.
    Devices(Vec<PairedDevice>),
    /// crownconnect is not running; the client keeps retrying.
    Unavailable,
}

#[derive(Debug, thiserror::Error)]
pub enum ConnectError {
    #[error("crownconnect is not running")]
    Unavailable,
    #[error("{} cannot be sent: its name is not UTF-8", .0.display())]
    NonUtf8Path(PathBuf),
    #[error("crownconnect did not answer in time")]
    Timeout,
    #[error(transparent)]
    Ipc(#[from] crownos_ipc::Error),
}

#[derive(Debug)]
enum Request {
    Send {
        id: DeviceId,
        paths: Vec<String>,
        reply: oneshot::Sender<Result<(), ConnectError>>,
    },
}

#[derive(Debug, Clone)]
enum Endpoint {
    Session,
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

/// Handle to the crownconnect session task, which reconnects with backoff
/// whenever the daemon goes away. Dropping every clone ends it.
#[derive(Debug, Clone)]
pub struct Connect {
    requests: mpsc::UnboundedSender<Request>,
    devices: watch::Receiver<Vec<PairedDevice>>,
}

impl Connect {
    /// Starts the session task on the current tokio runtime.
    pub fn spawn() -> (Self, mpsc::UnboundedReceiver<ConnectEvent>) {
        Self::spawn_with(Endpoint::Session)
    }

    /// Like [`Connect::spawn`] against a service bound in `dir`.
    pub fn spawn_in(dir: impl Into<PathBuf>) -> (Self, mpsc::UnboundedReceiver<ConnectEvent>) {
        Self::spawn_with(Endpoint::Directory(dir.into()))
    }

    fn spawn_with(endpoint: Endpoint) -> (Self, mpsc::UnboundedReceiver<ConnectEvent>) {
        let (requests, inbox) = mpsc::unbounded_channel();
        let (events, receiver) = mpsc::unbounded_channel();
        let (published, devices) = watch::channel(Vec::new());
        let session = Session {
            endpoint,
            devices: Vec::new(),
            published,
            events,
            available: None,
        };
        tokio::spawn(session.run(inbox));
        (Self { requests, devices }, receiver)
    }

    /// The paired devices as last reported.
    pub fn devices(&self) -> Vec<PairedDevice> {
        self.devices.borrow().clone()
    }

    /// Offers each of `paths` to the device `id`.
    pub async fn send_files(&self, id: DeviceId, paths: &[&Path]) -> Result<(), ConnectError> {
        let paths = paths
            .iter()
            .map(|path| {
                path.to_str()
                    .map(str::to_owned)
                    .ok_or_else(|| ConnectError::NonUtf8Path(path.to_path_buf()))
            })
            .collect::<Result<_, _>>()?;
        let (reply, answer) = oneshot::channel();
        self.requests
            .send(Request::Send { id, paths, reply })
            .map_err(|_| ConnectError::Unavailable)?;
        answer.await.map_err(|_| ConnectError::Unavailable)?
    }
}

struct Session {
    endpoint: Endpoint,
    devices: Vec<DeviceInfo>,
    published: watch::Sender<Vec<PairedDevice>>,
    events: mpsc::UnboundedSender<ConnectEvent>,
    available: Option<bool>,
}

impl Session {
    async fn run(mut self, mut inbox: mpsc::UnboundedReceiver<Request>) {
        let mut delay = FIRST_RETRY_DELAY;
        loop {
            if let Ok(client) = self.connect().await {
                delay = FIRST_RETRY_DELAY;
                if self.serve(client, &mut inbox).await.is_break() {
                    return;
                }
            }
            self.mark_unavailable();
            if refuse_until(Instant::now() + delay, &mut inbox)
                .await
                .is_break()
            {
                return;
            }
            delay = (delay * 2).min(MAX_RETRY_DELAY);
        }
    }

    async fn connect(&mut self) -> Result<AsyncClient, ConnectError> {
        let mut client = self.endpoint.connect()?;
        client.subscribe::<protocol::DeviceConnected>().await?;
        client.subscribe::<protocol::DevicesChanged>().await?;
        client.subscribe::<protocol::BatteryChanged>().await?;
        self.refresh(&mut client).await?;
        Ok(client)
    }

    async fn serve(
        &mut self,
        mut client: AsyncClient,
        inbox: &mut mpsc::UnboundedReceiver<Request>,
    ) -> ControlFlow<()> {
        loop {
            let alive = tokio::select! {
                request = inbox.recv() => match request {
                    Some(request) => self.handle(&mut client, request).await,
                    None => return ControlFlow::Break(()),
                },
                inbound = client.pump() => inbound.is_ok(),
            };
            let in_sync = !self.drain(&mut client) || self.refresh(&mut client).await.is_ok();
            if !alive || !in_sync || client.client().is_closed() {
                return ControlFlow::Continue(());
            }
        }
    }

    /// Answers `request`; `false` when the connection is lost.
    async fn handle(&mut self, client: &mut AsyncClient, request: Request) -> bool {
        let Request::Send { id, paths, reply } = request;
        let result = send_all(client, id, paths).await;
        let alive = !matches!(
            result,
            Err(ConnectError::Timeout
                | ConnectError::Ipc(crownos_ipc::Error::Io(_) | crownos_ipc::Error::Disconnected))
        );
        reply.send(result).ok();
        alive
    }

    async fn refresh(&mut self, client: &mut AsyncClient) -> Result<(), ConnectError> {
        let call = client.call::<protocol::devices>(&protocol::devices {}, Vec::new());
        self.devices = timeout(REPLY_TIMEOUT, call)
            .await
            .map_err(|_| ConnectError::Timeout)??;
        self.publish();
        Ok(())
    }

    /// Applies the queued events; `true` when the list must be fetched anew.
    fn drain(&mut self, client: &mut AsyncClient) -> bool {
        let mut changed = false;
        let mut stale = false;
        let inner = client.client_mut();
        while let Some(message) = inner.next_event_raw() {
            let payload = &message.payload;
            if message.selector == protocol::DevicesChanged::SELECTOR {
                if let Some(event) = decode::<protocol::DevicesChanged>(payload) {
                    self.devices = event.devices;
                    changed = true;
                }
            } else if message.selector == protocol::DeviceConnected::SELECTOR {
                if let Some(event) = decode::<protocol::DeviceConnected>(payload) {
                    match self.devices.iter_mut().find(|device| device.id == event.id) {
                        Some(device) => {
                            device.connected = event.connected;
                            device.name = event.name;
                            changed = true;
                        }
                        None => stale = true,
                    }
                }
            } else if message.selector == protocol::BatteryChanged::SELECTOR
                && let Some(event) = decode::<protocol::BatteryChanged>(payload)
                && let Some(device) = self.devices.iter_mut().find(|device| device.id == event.id)
            {
                device.battery = Some(Battery {
                    percent: event.percent,
                    charging: event.charging,
                });
                changed = true;
            }
        }
        if changed && !stale {
            self.publish();
        }
        stale
    }

    fn publish(&mut self) {
        let paired: Vec<PairedDevice> = self
            .devices
            .iter()
            .filter(|device| device.trusted)
            .map(PairedDevice::from_info)
            .collect();
        self.available = Some(true);
        self.published.send_replace(paired.clone());
        self.events.send(ConnectEvent::Devices(paired)).ok();
    }

    fn mark_unavailable(&mut self) {
        if self.available == Some(false) {
            return;
        }
        self.available = Some(false);
        self.devices.clear();
        self.published.send_replace(Vec::new());
        self.events.send(ConnectEvent::Unavailable).ok();
    }
}

async fn send_all(
    client: &mut AsyncClient,
    id: DeviceId,
    paths: Vec<String>,
) -> Result<(), ConnectError> {
    for path in paths {
        let request = protocol::send_file { id, path };
        let call = client.call::<protocol::send_file>(&request, Vec::new());
        timeout(REPLY_TIMEOUT, call)
            .await
            .map_err(|_| ConnectError::Timeout)??;
    }
    Ok(())
}

/// Turns requests away until `deadline`; `Break` when every handle is gone.
async fn refuse_until(
    deadline: Instant,
    inbox: &mut mpsc::UnboundedReceiver<Request>,
) -> ControlFlow<()> {
    loop {
        tokio::select! {
            () = sleep_until(deadline) => return ControlFlow::Continue(()),
            request = inbox.recv() => match request {
                Some(Request::Send { reply, .. }) => {
                    reply.send(Err(ConnectError::Unavailable)).ok();
                }
                None => return ControlFlow::Break(()),
            },
        }
    }
}

fn decode<E: EventMsg + DeserializeOwned>(payload: &[u8]) -> Option<E> {
    crownos_ipc::postcard::from_bytes(payload).ok()
}
