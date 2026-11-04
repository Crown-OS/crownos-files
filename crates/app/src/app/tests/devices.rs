//! Drives in the sidebar: listing, mounting on a click or on plug-in,
//! ejecting, unlocking, and leaving a drive that went away. The UDisks
//! worker is played by the tests.

use std::{fs, path::PathBuf};

use crownconfig::Files;
use crownos_files_devices::udisks::{Arrival, DeviceError, DeviceEvent, Volume};
use crowntest::{Query, Role, role, text};

use super::window::{Window, drain, window};
use crate::app::{
    drive_worker::{DriveAction, DriveCommand, DriveEvent},
    drives::{DriveActivity, finish_drives, release_drive},
    fixtures::volume,
};

const STICK: &str = "KINGSTON";

fn stick() -> Volume {
    volume("sdb1", STICK, true)
}

fn device(window: &mut Window, event: DeviceEvent) {
    window.answer(|cx, store| finish_drives(cx, store, vec![DriveEvent::Device(event)]));
}

fn plug(window: &mut Window, volume: Volume, arrival: Arrival) {
    device(window, DeviceEvent::Added { volume, arrival });
}

fn finish(
    window: &mut Window,
    volume: &Volume,
    action: DriveAction,
    result: Result<Option<PathBuf>, DeviceError>,
) {
    let event = DriveEvent::Finished {
        volume: volume.object_path.clone(),
        name: volume.name().to_owned(),
        action,
        result,
    };
    window.answer(|cx, store| finish_drives(cx, store, vec![event]));
}

fn commands(window: &mut Window) -> Vec<DriveCommand> {
    drain(&mut window.workers.drives)
}

fn mount_point(window: &Window) -> PathBuf {
    let path = window.home.path().join("Media").join(STICK);
    fs::create_dir_all(&path).unwrap();
    path
}

fn cwd(window: &mut Window) -> PathBuf {
    window.answer(|cx, store| store.cwd.get(cx).to_path_buf())
}

fn row(name: &str) -> Query {
    role(Role::MenuItem).text(name)
}

/// The stick's row with its eject button, rather than a spinner.
fn ejectable() -> Query {
    role(Role::MenuItem).name(format!("{STICK} Eject {STICK}"))
}

fn activity(window: &mut Window, volume: &Volume) -> Option<DriveActivity> {
    window.answer(|cx, store| {
        store.drives.rows.with(cx, |rows| {
            rows.iter()
                .find(|row| row.volume.object_path == volume.object_path)
                .and_then(|row| row.activity)
        })
    })
}

fn eject(window: &mut Window, volume: &Volume) {
    let path = volume.object_path.clone();
    window.answer(|cx, store| release_drive(cx, store, &path));
}

fn type_passphrase(window: &mut Window, passphrase: &str) {
    let passphrase = passphrase.to_owned();
    window.answer(|cx, store| store.drives.unlock.passphrase.set(cx, passphrase));
}

#[test]
fn the_devices_section_lists_drives_and_paired_phones() {
    let mut window = window();
    assert!(window.shows(role(Role::MenuItem).name("CrownBook Eject CrownBook")));
    assert!(window.shows(role(Role::MenuItem).name("Pixel 9 Pro, connected")));
    plug(&mut window, stick(), Arrival::Present);
    assert!(window.shows(ejectable()));
    assert!(
        commands(&mut window).is_empty(),
        "drives present at start are not mounted"
    );
    let mut internal = volume("sda1", "Archive", false);
    internal.hints.auto = false;
    plug(&mut window, internal, Arrival::Present);
    assert!(
        window.shows(role(Role::MenuItem).name("Archive")),
        "an unmounted internal volume has no eject button"
    );
    device(&mut window, DeviceEvent::Removed(stick().object_path));
    assert!(!window.shows(row(STICK)));
}

#[test]
fn a_click_mounts_a_drive_then_opens_it() {
    let mut window = window();
    plug(&mut window, stick(), Arrival::Present);
    window.app.click(row(STICK)).settle();
    let volume = stick();
    assert!(matches!(
        commands(&mut window).as_slice(),
        [DriveCommand::Run(mounted, DriveAction::Open)] if *mounted == volume
    ));
    assert_eq!(
        activity(&mut window, &volume),
        Some(DriveActivity::Mounting)
    );
    assert!(
        window.shows(role(Role::MenuItem).name(STICK)),
        "a spinner stands in for the eject button"
    );
    window.app.click(row(STICK)).settle();
    assert!(
        commands(&mut window).is_empty(),
        "a drive being mounted ignores clicks"
    );

    let at = mount_point(&window);
    finish(
        &mut window,
        &volume,
        DriveAction::Open,
        Ok(Some(at.clone())),
    );
    assert_eq!(cwd(&mut window), at);
    assert_eq!(activity(&mut window, &volume), None);
    assert!(window.shows(ejectable()));
}

#[test]
fn a_plugged_drive_is_mounted_when_automount_is_on() {
    let mut window = window();
    let before = cwd(&mut window);
    plug(&mut window, stick(), Arrival::Plugged);
    let plugged = stick();
    assert!(matches!(
        commands(&mut window).as_slice(),
        [DriveCommand::Run(mounted, DriveAction::Automount)] if *mounted == plugged
    ));
    assert_eq!(
        activity(&mut window, &plugged),
        Some(DriveActivity::Mounting)
    );
    let at = mount_point(&window);
    finish(&mut window, &plugged, DriveAction::Automount, Ok(Some(at)));
    assert!(window.shows(text(format!("Mounted {STICK}"))));
    assert_eq!(cwd(&mut window), before, "automount_open is off by default");

    let files = Files {
        automount: false,
        ..Files::default()
    };
    window.answer(|cx, store| store.drives.configure(cx, &files));
    plug(&mut window, volume("sdc1", "CARD", true), Arrival::Plugged);
    assert!(commands(&mut window).is_empty());
}

#[test]
fn ejecting_says_when_the_drive_can_be_removed_and_retries_when_busy() {
    let mut window = window();
    let mut volume = stick();
    volume.mount_points.push(mount_point(&window));
    plug(&mut window, volume.clone(), Arrival::Present);
    eject(&mut window, &volume);
    assert!(matches!(
        commands(&mut window).as_slice(),
        [DriveCommand::Run(ejected, DriveAction::Eject)] if *ejected == volume
    ));
    assert_eq!(
        activity(&mut window, &volume),
        Some(DriveActivity::Ejecting)
    );
    eject(&mut window, &volume);
    assert!(commands(&mut window).is_empty(), "one eject at a time");

    finish(
        &mut window,
        &volume,
        DriveAction::Eject,
        Err(DeviceError::Busy),
    );
    assert!(window.shows(text(format!("{STICK} is in use"))));
    window
        .app
        .click(role(Role::Button).name("Try Again"))
        .settle();
    assert!(matches!(
        commands(&mut window).as_slice(),
        [DriveCommand::Run(_, DriveAction::Eject)]
    ));
    assert!(!window.shows(text(format!("{STICK} is in use"))));
    finish(&mut window, &volume, DriveAction::Eject, Ok(None));
    assert!(window.shows(text(format!("{STICK} can be removed safely"))));
}

#[test]
fn refused_and_cancelled_authentication_are_toasts() {
    let mut window = window();
    let volume = stick();
    plug(&mut window, volume.clone(), Arrival::Present);
    finish(
        &mut window,
        &volume,
        DriveAction::Open,
        Err(DeviceError::NotAuthorized),
    );
    assert!(window.shows(text(format!("You aren't allowed to mount {STICK}"))));
    finish(
        &mut window,
        &volume,
        DriveAction::Eject,
        Err(DeviceError::Cancelled),
    );
    assert!(window.shows(text(format!(
        "{STICK} wasn't ejected: authentication was cancelled"
    ))));
}

#[test]
fn a_drive_that_goes_away_takes_the_window_home() {
    let mut window = window();
    let at = mount_point(&window);
    let mut volume = stick();
    volume.mount_points.push(at.clone());
    plug(&mut window, volume.clone(), Arrival::Present);
    window.app.click(row(STICK)).settle();
    assert_eq!(cwd(&mut window), at);
    assert!(
        commands(&mut window).is_empty(),
        "a mounted drive just opens"
    );

    device(
        &mut window,
        DeviceEvent::Removed(volume.object_path.clone()),
    );
    assert_eq!(cwd(&mut window), window.home.path());
    assert!(!window.shows(row(STICK)));
}

#[test]
fn an_encrypted_drive_asks_for_its_passphrase_until_it_unlocks() {
    let mut window = window();
    let mut volume = volume("sdd1", "Secret", true);
    volume.is_encrypted = true;
    plug(&mut window, volume.clone(), Arrival::Plugged);
    assert!(
        commands(&mut window).is_empty(),
        "locked drives are not mounted on their own"
    );
    window.app.click(row("Secret")).settle();
    assert!(window.shows(text("Unlock “Secret”")));

    assert!(window.app.find(role(Role::TextField)).states.secure);
    type_passphrase(&mut window, "hunter2");
    window.app.click(role(Role::Button).name("Unlock")).settle();
    assert!(matches!(
        commands(&mut window).as_slice(),
        [DriveCommand::Unlock(unlocked, passphrase)] if *unlocked == volume && passphrase.0 == "hunter2"
    ));
    assert!(window.shows(role(Role::Button).name("Unlocking…")));
    let typed = window.answer(|cx, store| store.drives.unlock.passphrase.get(cx));
    assert!(typed.is_empty(), "the passphrase is not kept once sent");
    let failed = DeviceError::Failed("Incorrect passphrase".to_owned());
    finish(&mut window, &volume, DriveAction::Open, Err(failed));
    assert!(window.shows(text("Couldn't unlock: Incorrect passphrase")));

    type_passphrase(&mut window, "correct horse");
    window.app.click(role(Role::Button).name("Unlock")).settle();
    assert!(matches!(
        commands(&mut window).as_slice(),
        [DriveCommand::Unlock(_, passphrase)] if passphrase.0 == "correct horse"
    ));
    let at = mount_point(&window);
    finish(
        &mut window,
        &volume,
        DriveAction::Open,
        Ok(Some(at.clone())),
    );
    assert!(!window.shows(text("Unlock “Secret”")));
    assert_eq!(cwd(&mut window), at);
}
