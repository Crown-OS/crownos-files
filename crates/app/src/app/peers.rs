//! Paired phones and tablets: the sidebar lists them, and "Send to" hands
//! them the selection through the crownconnect worker.

use std::path::PathBuf;

use crownui::{
    kit::icons,
    prelude::{Cx, Runtime, Signal, Store as Cell},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::state::Store;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PeerId(pub [u8; 32]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PeerKind {
    Phone,
    Tablet,
    Watch,
    Computer,
}

impl PeerKind {
    pub fn icon(self) -> &'static str {
        match self {
            Self::Phone => icons::SMARTPHONE,
            Self::Tablet => icons::TABLET,
            Self::Watch => icons::WATCH,
            Self::Computer => icons::LAPTOP,
        }
    }
}

/// A device paired with crownconnect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub id: PeerId,
    pub name: String,
    pub kind: PeerKind,
    pub connected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerCommand {
    Send {
        id: PeerId,
        name: String,
        paths: Vec<PathBuf>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerEvent {
    /// The paired devices after any change; none while crownconnect is away.
    Paired(Vec<Peer>),
    Sent {
        name: String,
        count: usize,
    },
    SendFailed {
        name: String,
        error: String,
    },
}

#[derive(Clone, Copy)]
pub struct Peers {
    pub sender: Cell<Option<UnboundedSender<PeerCommand>>>,
    pub paired: Signal<Vec<Peer>>,
}

impl Peers {
    pub fn new(cx: &mut Cx) -> Self {
        Self {
            sender: cx.store(None),
            paired: cx.signal(Vec::new()),
        }
    }

    /// The details panel's "Send to" target: the first connected phone.
    pub fn first_phone(self, runtime: &mut Runtime) -> Option<Peer> {
        self.paired.with(runtime, |paired| {
            paired
                .iter()
                .find(|peer| peer.connected && peer.kind == PeerKind::Phone)
                .cloned()
        })
    }

    /// The devices files can go to right now.
    pub fn reachable(self, runtime: &mut Runtime) -> Vec<Peer> {
        self.paired.with(runtime, |paired| {
            paired
                .iter()
                .filter(|peer| peer.connected)
                .cloned()
                .collect()
        })
    }
}

/// Offers the selected files to `peer`; folders are left out.
pub fn send_selection(cx: &mut Cx, store: Store, peer: &Peer) {
    let paths: Vec<PathBuf> = store.folder.with(cx, |folder| {
        folder
            .selected_entries()
            .filter(|entry| !entry.is_dir())
            .map(|entry| entry.path_in(folder.dir()))
            .collect()
    });
    if paths.is_empty() {
        store
            .toasts
            .show(cx, format!("Only files can be sent to {}", peer.name));
        return;
    }
    let command = PeerCommand::Send {
        id: peer.id,
        name: peer.name.clone(),
        paths,
    };
    store.peers.sender.with(cx, |sender| {
        if let Some(sender) = sender {
            let _ = sender.send(command);
        }
    });
}

pub fn finish_peers(cx: &mut Cx, store: Store, events: Vec<PeerEvent>) {
    for event in events {
        match event {
            PeerEvent::Paired(peers) => {
                if store.peers.paired.with(cx, |known| *known != peers) {
                    store.peers.paired.set(cx, peers);
                }
            }
            PeerEvent::Sent { name, count } => {
                let files = if count == 1 { "file" } else { "files" };
                store
                    .toasts
                    .show(cx, format!("Sent {count} {files} to {name}"));
            }
            PeerEvent::SendFailed { name, error } => {
                store
                    .toasts
                    .show(cx, format!("Couldn't send to {name}: {error}"));
            }
        }
    }
}
