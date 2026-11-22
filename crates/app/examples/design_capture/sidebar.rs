use std::{path::Path, sync::Arc};

use capture::Carrier;
use crownos_files::app::pinning::show_pin_zone;
use crowntest::{Role, role};

use crate::{
    Stage,
    fixture::{Fixture, LAUNCH},
    window::{Window, drive_row, phone, tablet, volume},
};

const SIDEBAR: &str = "Sidebar";
pub const STICK: &str = "KINGSTON";
pub const ENCRYPTED: &str = "Backup";
const PINNED: [&str; 2] = [LAUNCH, "Pictures/Wallpapers"];

pub fn capture(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    devices(stage, carrier)?;
    mounting(stage, carrier)?;
    pinned(stage, carrier)?;
    pin_zone(stage, carrier)
}

/// A USB stick, a locked internal drive and an offline tablet next to the defaults.
pub fn plug_devices(window: &mut Window) {
    window.act(|cx, store| {
        let mut encrypted = volume("sda2", ENCRYPTED, false);
        encrypted.is_encrypted = true;
        encrypted.size = 2_000_000_000_000;
        store.drives.rows.update(cx, |rows| {
            rows.push(drive_row(volume("sdb1", STICK, true)));
            rows.push(drive_row(encrypted));
        });
        store.peers.paired.set(cx, vec![phone(), tablet(false)]);
    });
}

fn devices(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    plug_devices(&mut window);
    window.capture(
        carrier,
        SIDEBAR,
        "Devices",
        "Drives with eject buttons, a locked drive, a connected phone and an offline tablet",
    )
}

fn mounting(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    plug_devices(&mut window);
    window.click(role(Role::MenuItem).text(STICK));
    window.capture(
        carrier,
        SIDEBAR,
        "Mounting drive",
        "A click on an unmounted stick: a spinner while it mounts",
    )
}

fn pin(window: &mut Window, fixture: &Fixture) {
    let pinned: Vec<Arc<Path>> = PINNED
        .iter()
        .map(|relative| fixture.path(relative))
        .collect();
    window.act(|cx, store| store.pinned.set(cx, pinned));
}

fn pinned(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    pin(&mut window, &stage.fixture);
    window.capture(
        carrier,
        SIDEBAR,
        "Pinned folders",
        "Folders pinned under Favorites; the open one highlighted",
    )
}

fn pin_zone(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    pin(&mut window, &stage.fixture);
    window.act(|cx, store| show_pin_zone(cx, store, true));
    window.capture(
        carrier,
        SIDEBAR,
        "Pin drop zone",
        "While a folder is dragged, a zone opens under Pinned to pin it",
    )
}
