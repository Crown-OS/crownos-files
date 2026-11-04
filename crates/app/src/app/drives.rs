//! The sidebar's drives: the volumes UDisks reports and what is being done
//! to each, mounting on a click or as they are plugged in, ejecting, and
//! leaving a folder whose volume went away.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crownconfig::Files;
use crownos_files_devices::udisks::{
    AutomountPolicy, DeviceError, DeviceEvent, OwnedObjectPath, Volume, should_automount,
};
use crownui::prelude::{Cx, Runtime, Signal, Store as Cell};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    app::{
        drive_worker::{DriveAction, DriveCommand, DriveEvent},
        navigation::open_dir,
        unlock::{Unlock, ask_passphrase, close_unlock, unlock_failed},
    },
    state::Store,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DriveActivity {
    Mounting,
    Ejecting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveRow {
    pub volume: Volume,
    pub activity: Option<DriveActivity>,
}

impl DriveRow {
    pub fn mount_point(&self) -> Option<&Path> {
        self.volume.mount_points.first().map(PathBuf::as_path)
    }

    /// Whether the row offers an eject button.
    pub fn can_release(&self) -> bool {
        self.volume.is_removable() || self.volume.is_mounted()
    }
}

/// A drive that could not be ejected because something still uses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusyDrive {
    pub volume: OwnedObjectPath,
    pub name: String,
}

#[derive(Clone, Copy)]
pub struct Drives {
    pub sender: Cell<Option<UnboundedSender<DriveCommand>>>,
    pub rows: Signal<Vec<DriveRow>>,
    pub busy: Signal<Option<BusyDrive>>,
    pub unlock: Unlock,
    automount: Cell<AutomountPolicy>,
    open_automounted: Cell<bool>,
}

impl Drives {
    pub fn new(cx: &mut Cx, files: &Files) -> Self {
        Self {
            sender: cx.store(None),
            rows: cx.signal(Vec::new()),
            busy: cx.signal(None),
            unlock: Unlock::new(cx),
            automount: cx.store(automount_policy(files)),
            open_automounted: cx.store(files.automount_open),
        }
    }

    /// Follows the `files.automount` and `files.automount_open` settings.
    pub fn configure(self, runtime: &mut Runtime, files: &Files) {
        self.automount
            .update(runtime, |policy| *policy = automount_policy(files));
        self.open_automounted
            .update(runtime, |open| *open = files.automount_open);
    }

    pub fn send(self, runtime: &Runtime, command: DriveCommand) {
        self.sender.with(runtime, |sender| {
            if let Some(sender) = sender {
                let _ = sender.send(command);
            }
        });
    }

    fn row(self, runtime: &mut Runtime, volume: &OwnedObjectPath) -> Option<DriveRow> {
        self.rows.with(runtime, |rows| {
            rows.iter()
                .find(|row| row.volume.object_path == *volume)
                .cloned()
        })
    }

    fn set_activity(
        self,
        runtime: &mut Runtime,
        volume: &OwnedObjectPath,
        activity: Option<DriveActivity>,
    ) {
        self.rows.update(runtime, |rows| {
            if let Some(row) = rows
                .iter_mut()
                .find(|row| row.volume.object_path == *volume)
            {
                row.activity = activity;
            }
        });
    }
}

fn automount_policy(files: &Files) -> AutomountPolicy {
    if files.automount {
        AutomountPolicy::Enabled
    } else {
        AutomountPolicy::Disabled
    }
}

/// A click on a drive: opens it, mounting or unlocking it first.
pub fn open_drive(cx: &mut Cx, store: Store, volume: &OwnedObjectPath) {
    let Some(row) = store.drives.row(cx, volume) else {
        return;
    };
    if row.activity.is_some() {
        return;
    }
    if let Some(mount_point) = row.mount_point() {
        open_dir(cx, store, mount_point.into());
    } else if row.volume.filesystem_object().is_none() {
        ask_passphrase(cx, store, row.volume);
    } else {
        store
            .drives
            .set_activity(cx, volume, Some(DriveActivity::Mounting));
        store
            .drives
            .send(cx, DriveCommand::Run(row.volume, DriveAction::Open));
    }
}

/// The eject button: ejects a removable drive, unmounts any other volume.
pub fn release_drive(cx: &mut Cx, store: Store, volume: &OwnedObjectPath) {
    let Some(row) = store.drives.row(cx, volume) else {
        return;
    };
    if row.activity.is_some() || !row.can_release() {
        return;
    }
    if store.drives.busy.with(cx, Option::is_some) {
        store.drives.busy.set(cx, None);
    }
    let action = if row.volume.is_removable() {
        DriveAction::Eject
    } else {
        DriveAction::Unmount
    };
    store
        .drives
        .set_activity(cx, volume, Some(DriveActivity::Ejecting));
    store.drives.send(cx, DriveCommand::Run(row.volume, action));
}

/// "Try Again" on the busy callout.
pub fn retry_release(cx: &mut Cx, store: Store) {
    if let Some(busy) = store.drives.busy.get(cx) {
        release_drive(cx, store, &busy.volume);
    }
}

pub fn dismiss_busy(runtime: &mut Runtime, store: Store) {
    if store.drives.busy.with(runtime, Option::is_some) {
        store.drives.busy.set(runtime, None);
    }
}

pub fn finish_drives(cx: &mut Cx, store: Store, events: Vec<DriveEvent>) {
    for event in events {
        match event {
            DriveEvent::Device(event) => apply_device_event(cx, store, event),
            DriveEvent::Finished {
                volume,
                name,
                action,
                result,
            } => {
                store.drives.set_activity(cx, &volume, None);
                finish_action(cx, store, &volume, &name, action, result);
            }
        }
    }
}

fn apply_device_event(runtime: &mut Runtime, store: Store, event: DeviceEvent) {
    let policy = store.drives.automount.with(runtime, |policy| *policy);
    let automount = should_automount(&event, policy).is_some();
    let before = match &event {
        DeviceEvent::Added { volume, .. } | DeviceEvent::Changed(volume) => {
            mount_points(runtime, store, &volume.object_path)
        }
        DeviceEvent::Removed(path) => mount_points(runtime, store, path),
    };
    let after = match event {
        DeviceEvent::Added { volume, .. } | DeviceEvent::Changed(volume) => {
            let after = volume.mount_points.clone();
            if automount {
                store.drives.send(
                    runtime,
                    DriveCommand::Run(volume.clone(), DriveAction::Automount),
                );
            }
            let activity = automount.then_some(DriveActivity::Mounting);
            store
                .drives
                .rows
                .update(runtime, |rows| upsert(rows, volume, activity));
            after
        }
        DeviceEvent::Removed(path) => {
            store.drives.rows.update(runtime, |rows| {
                rows.retain(|row| row.volume.object_path != path)
            });
            if store.drives.busy.with(runtime, |busy| {
                busy.as_ref().is_some_and(|busy| busy.volume == path)
            }) {
                store.drives.busy.set(runtime, None);
            }
            close_unlock(runtime, store, &path);
            Vec::new()
        }
    };
    leave_if_unmounted(runtime, store, &before, &after);
}

fn mount_points(runtime: &mut Runtime, store: Store, volume: &OwnedObjectPath) -> Vec<PathBuf> {
    store.drives.rows.with(runtime, |rows| {
        rows.iter()
            .find(|row| row.volume.object_path == *volume)
            .map(|row| row.volume.mount_points.clone())
            .unwrap_or_default()
    })
}

/// Adds or refreshes the row of `volume`, internal disks first, then by name.
fn upsert(rows: &mut Vec<DriveRow>, volume: Volume, activity: Option<DriveActivity>) {
    match rows
        .iter_mut()
        .find(|row| row.volume.object_path == volume.object_path)
    {
        Some(row) => {
            row.volume = volume;
            row.activity = activity.or(row.activity);
        }
        None => rows.push(DriveRow { volume, activity }),
    }
    rows.sort_by(|left, right| {
        (left.volume.is_removable(), left.volume.name())
            .cmp(&(right.volume.is_removable(), right.volume.name()))
    });
}

/// Goes home when the current folder was on a mount that is gone.
fn leave_if_unmounted(runtime: &mut Runtime, store: Store, before: &[PathBuf], after: &[PathBuf]) {
    let dir = store.dir(runtime);
    let gone = before
        .iter()
        .any(|mount| !after.contains(mount) && dir.starts_with(mount));
    if gone {
        let home = store.places.with(runtime, |places| places.home.clone());
        let home: Arc<Path> = home.unwrap_or_else(|| PathBuf::from("/")).into();
        open_dir(runtime, store, home);
    }
}

fn finish_action(
    cx: &mut Cx,
    store: Store,
    volume: &OwnedObjectPath,
    name: &str,
    action: DriveAction,
    result: Result<Option<PathBuf>, DeviceError>,
) {
    match (action, result) {
        (DriveAction::Open, Ok(mount_point)) => {
            close_unlock(cx, store, volume);
            if let Some(mount_point) = mount_point {
                open_dir(cx, store, mount_point.into());
            }
        }
        (DriveAction::Automount, Ok(mount_point)) => {
            store.toasts.show(cx, format!("Mounted {name}"));
            if let Some(mount_point) = mount_point
                && store.drives.open_automounted.with(cx, |open| *open)
            {
                open_dir(cx, store, mount_point.into());
            }
        }
        (DriveAction::Eject, Ok(_)) => {
            store
                .toasts
                .show(cx, format!("{name} can be removed safely"));
        }
        (DriveAction::Unmount, Ok(_)) => store.toasts.show(cx, format!("Unmounted {name}")),
        (DriveAction::Eject | DriveAction::Unmount, Err(DeviceError::Busy)) => {
            store.drives.busy.set(
                cx,
                Some(BusyDrive {
                    volume: volume.clone(),
                    name: name.to_owned(),
                }),
            );
        }
        (action, Err(error)) => {
            if action != DriveAction::Open || !unlock_failed(cx, store, volume, &error) {
                store.toasts.show(cx, failure_message(action, name, &error));
            }
        }
    }
}

fn failure_message(action: DriveAction, name: &str, error: &DeviceError) -> String {
    let verb = match action {
        DriveAction::Open | DriveAction::Automount => "mount",
        DriveAction::Eject => "eject",
        DriveAction::Unmount => "unmount",
    };
    match error {
        DeviceError::NotAuthorized => format!("You aren't allowed to {verb} {name}"),
        DeviceError::Cancelled => format!("{name} wasn't {verb}ed: authentication was cancelled"),
        error => format!("Couldn't {verb} {name}: {error}"),
    }
}
