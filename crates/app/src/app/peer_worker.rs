//! The crownlink session for as long as the window is open: paired
//! devices up, files to send down.

use std::path::{Path, PathBuf};

use crownos_files_devices::connect::{Connect, ConnectEvent, DeviceId, DeviceKind, PairedDevice};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::app::peers::{Peer, PeerCommand, PeerEvent, PeerId, PeerKind};

pub async fn run_peers(
    events: UnboundedSender<PeerEvent>,
    mut commands: UnboundedReceiver<PeerCommand>,
) {
    let (connect, mut updates) = Connect::spawn();
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                Some(PeerCommand::Send { id, name, paths }) => {
                    tokio::spawn(send(connect.clone(), events.clone(), id, name, paths));
                }
                None => break,
            },
            Some(update) = updates.recv() => {
                let paired = match update {
                    ConnectEvent::Devices(devices) => devices.iter().map(peer).collect(),
                    ConnectEvent::Unavailable => Vec::new(),
                };
                if events.send(PeerEvent::Paired(paired)).is_err() {
                    break;
                }
            }
        }
    }
}

async fn send(
    connect: Connect,
    events: UnboundedSender<PeerEvent>,
    id: PeerId,
    name: String,
    paths: Vec<PathBuf>,
) {
    let borrowed: Vec<&Path> = paths.iter().map(PathBuf::as_path).collect();
    let event = match connect.send_files(DeviceId(id.0), &borrowed).await {
        Ok(()) => PeerEvent::Sent {
            name,
            count: paths.len(),
        },
        Err(error) => PeerEvent::SendFailed {
            name,
            error: error.to_string(),
        },
    };
    let _ = events.send(event);
}

fn peer(device: &PairedDevice) -> Peer {
    Peer {
        id: PeerId(device.id.0),
        name: device.name.clone(),
        kind: match device.kind {
            DeviceKind::Phone => PeerKind::Phone,
            DeviceKind::Tablet => PeerKind::Tablet,
            DeviceKind::Watch => PeerKind::Watch,
            DeviceKind::Computer => PeerKind::Computer,
        },
        connected: device.connected,
    }
}
