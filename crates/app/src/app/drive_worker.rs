//! UDisks for as long as the window is open: the volumes as they come and
//! go, and mounting, unlocking and ejecting them. When UDisks cannot be
//! reached the worker ends and the sidebar lists no drives.

use std::{fmt, path::PathBuf};

use crownos_files_devices::udisks::{
    DeviceError, DeviceEvent, DeviceMonitor, OwnedObjectPath, Udisks, Volume,
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

/// What the user, or automounting, asked of a volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveAction {
    /// Mount if needed, then open it.
    Open,
    Automount,
    Eject,
    Unmount,
}

#[derive(Debug)]
pub enum DriveCommand {
    Run(Volume, DriveAction),
    /// Unlock an encrypted volume, mount it and open it.
    Unlock(Volume, Passphrase),
}

/// What the user typed to unlock a drive; never printed.
pub struct Passphrase(pub String);

impl fmt::Debug for Passphrase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Passphrase(…)")
    }
}

#[derive(Debug)]
pub enum DriveEvent {
    Device(DeviceEvent),
    /// An action ended; mounts report where.
    Finished {
        volume: OwnedObjectPath,
        name: String,
        action: DriveAction,
        result: Result<Option<PathBuf>, DeviceError>,
    },
}

pub async fn run_drives(
    events: UnboundedSender<DriveEvent>,
    mut commands: UnboundedReceiver<DriveCommand>,
) {
    let Ok(udisks) = Udisks::connect().await else {
        return;
    };
    let Ok((_monitor, mut devices)) = DeviceMonitor::spawn(&udisks).await else {
        return;
    };
    loop {
        tokio::select! {
            command = commands.recv() => {
                let Some(command) = command else {
                    break;
                };
                let (udisks, events) = (udisks.clone(), events.clone());
                tokio::spawn(async move {
                    let _ = events.send(perform(&udisks, command).await);
                });
            }
            Some(event) = devices.recv() => {
                if events.send(DriveEvent::Device(event)).is_err() {
                    break;
                }
            }
        }
    }
}

async fn perform(udisks: &Udisks, command: DriveCommand) -> DriveEvent {
    let (volume, action, result) = match command {
        DriveCommand::Run(volume, action) => {
            let result = match action {
                DriveAction::Open | DriveAction::Automount => udisks.mount(&volume).await.map(Some),
                DriveAction::Eject => udisks.eject(&volume).await.map(|()| None),
                DriveAction::Unmount => udisks.unmount(&volume).await.map(|()| None),
            };
            (volume, action, result)
        }
        DriveCommand::Unlock(volume, passphrase) => {
            let result = udisks.unlock(&volume, &passphrase.0).await.map(Some);
            (volume, DriveAction::Open, result)
        }
    };
    DriveEvent::Finished {
        name: volume.name().to_owned(),
        volume: volume.object_path,
        action,
        result,
    }
}
