//! The passphrase dialog of an encrypted drive: unlocks it, mounts what is
//! inside and opens it, or says why not and lets the user try again.

use std::mem;

use crownos_files_devices::udisks::{DeviceError, OwnedObjectPath, Volume};
use crownui::prelude::{Cx, Runtime, Signal, Store as Cell};

use crate::{
    app::{
        drive_worker::{DriveCommand, Passphrase},
        drives::DriveActivity,
    },
    state::Store,
};

#[derive(Clone, Copy)]
pub struct Unlock {
    pub open: Signal<bool>,
    pub name: Signal<String>,
    pub passphrase: Signal<String>,
    pub error: Signal<Option<String>>,
    pub waiting: Signal<bool>,
    volume: Cell<Option<Volume>>,
}

impl Unlock {
    pub fn new(cx: &mut Cx) -> Self {
        Self {
            open: cx.signal(false),
            name: cx.signal(String::new()),
            passphrase: cx.signal(String::new()),
            error: cx.signal(None),
            waiting: cx.signal(false),
            volume: cx.store(None),
        }
    }

    fn is_for(self, runtime: &Runtime, volume: &OwnedObjectPath) -> bool {
        self.volume.with(runtime, |asked| {
            asked
                .as_ref()
                .is_some_and(|asked| asked.object_path == *volume)
        })
    }
}

pub fn ask_passphrase(cx: &mut Cx, store: Store, volume: Volume) {
    let unlock = store.drives.unlock;
    unlock.name.set(cx, volume.name().to_owned());
    unlock.passphrase.set(cx, String::new());
    unlock.error.set(cx, None);
    unlock.waiting.set(cx, false);
    unlock.volume.update(cx, |asked| *asked = Some(volume));
    unlock.open.set(cx, true);
}

/// "Unlock" or Enter in the passphrase field.
pub fn submit_passphrase(cx: &mut Cx, store: Store) {
    let unlock = store.drives.unlock;
    let Some(volume) = unlock.volume.with(cx, Clone::clone) else {
        return;
    };
    if unlock.waiting.get(cx) || unlock.passphrase.with(cx, String::is_empty) {
        return;
    }
    let mut passphrase = String::new();
    unlock
        .passphrase
        .update(cx, |typed| passphrase = mem::take(typed));
    unlock.error.set(cx, None);
    unlock.waiting.set(cx, true);
    store.drives.rows.update(cx, |rows| {
        if let Some(row) = rows
            .iter_mut()
            .find(|row| row.volume.object_path == volume.object_path)
        {
            row.activity = Some(DriveActivity::Mounting);
        }
    });
    store
        .drives
        .send(cx, DriveCommand::Unlock(volume, Passphrase(passphrase)));
}

pub fn cancel_unlock(runtime: &mut Runtime, store: Store) {
    store.drives.unlock.open.set(runtime, false);
}

/// Closes the dialog once its volume is open or gone.
pub fn close_unlock(runtime: &mut Runtime, store: Store, volume: &OwnedObjectPath) {
    let unlock = store.drives.unlock;
    if unlock.is_for(runtime, volume) {
        unlock.volume.update(runtime, |asked| *asked = None);
        unlock.waiting.set(runtime, false);
        unlock.open.set(runtime, false);
    }
}

/// Shows why unlocking `volume` failed in its open dialog; `false` when no
/// dialog is waiting for it.
pub fn unlock_failed(
    runtime: &mut Runtime,
    store: Store,
    volume: &OwnedObjectPath,
    error: &DeviceError,
) -> bool {
    let unlock = store.drives.unlock;
    if !unlock.is_for(runtime, volume) || !unlock.open.get(runtime) {
        return false;
    }
    unlock.waiting.set(runtime, false);
    unlock.error.set(
        runtime,
        Some(match error {
            DeviceError::Cancelled => "Authentication was cancelled".to_owned(),
            DeviceError::NotAuthorized => "You aren't allowed to unlock this drive".to_owned(),
            error => format!("Couldn't unlock: {error}"),
        }),
    );
    true
}

/// Forgets the typed passphrase and the volume whenever the dialog closes.
pub fn follow_unlock_dismissal(cx: &mut Cx, store: Store) {
    let unlock = store.drives.unlock;
    cx.effect(move |runtime| {
        if unlock.open.get(runtime) {
            return;
        }
        if unlock
            .passphrase
            .with_untracked(runtime, |typed| !typed.is_empty())
        {
            unlock.passphrase.set(runtime, String::new());
        }
        unlock.volume.update(runtime, |asked| *asked = None);
    });
}
