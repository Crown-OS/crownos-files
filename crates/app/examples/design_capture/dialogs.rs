use capture::Carrier;
use crownos_files::app::{
    drive_worker::{DriveAction, DriveEvent},
    drives::finish_drives,
    file_jobs::{JobUpdate, finish_jobs},
    opener::{AppChoices, AppsPurpose, OpenEvent},
    opening::{choose_app, finish_opening},
    properties::show_properties,
};
use crownos_files_core::{
    jobs::{JobEvent, JobId},
    model::mime_id::MimeId,
};
use crownos_files_devices::udisks::DeviceError;
use crowntest::{Role, role};
use tokio::sync::oneshot;

use crate::{
    Stage,
    fixture::{LAUNCH, MOUNTED_FOLDER},
    sidebar::{ENCRYPTED, plug_devices},
    window::{Held, Window, volume},
};

const DIALOGS: &str = "Dialogs";
const ARCHIVE: &str = "launch-assets.tar.zst";
const PASSPHRASE: &str = "correct horse battery";
const UNLOCK_ERROR: &str = "Failed to activate device: Incorrect passphrase";

pub fn capture(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    delete_permanently(stage, carrier)?;
    delete_from_trash(stage, carrier)?;
    empty_trash(stage, carrier)?;
    conflict(stage, carrier)?;
    open_with(stage, carrier)?;
    open_with_nothing(stage, carrier)?;
    file_properties(stage, carrier)?;
    properties_pending(stage, carrier)?;
    folder_properties(stage, carrier)?;
    disk_properties(stage, carrier)?;
    unlock(stage, carrier)?;
    unlocking(stage, carrier)?;
    unlock_error(stage, carrier)
}

fn delete_permanently(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    window
        .click_entry("keynote.pdf")
        .key("Shift+ArrowRight")
        .key("Shift+Delete");
    window.capture(
        carrier,
        DIALOGS,
        "Delete permanently",
        "Shift+Delete asks before deleting without the Trash",
    )
}

fn open_trash(stage: &Stage) -> anyhow::Result<Window<'_>> {
    let mut window = stage.window(LAUNCH)?;
    window.click(role(Role::MenuItem).name("Trash"));
    Ok(window)
}

fn delete_from_trash(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = open_trash(stage)?;
    window.click_entry("old-logo.svg").key("Delete");
    window.capture(
        carrier,
        DIALOGS,
        "Delete from Trash",
        "Deleting a trashed item asks first",
    )
}

fn empty_trash(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = open_trash(stage)?;
    window.click(role(Role::Button).name("Empty Trash"));
    window.capture(
        carrier,
        DIALOGS,
        "Empty Trash?",
        "Empty Trash asks before deleting everything in it",
    )
}

fn conflict(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    let (reply, _answer) = oneshot::channel();
    let target = stage.fixture.path(LAUNCH).join("keynote.pdf");
    let source = stage.fixture.path("Downloads").join("keynote.pdf");
    window.act(|cx, store| {
        let conflict = JobEvent::Conflict {
            id: JobId::next(),
            source,
            target,
            reply,
        };
        finish_jobs(cx, store, vec![JobUpdate::Event(conflict)]);
    });
    window.capture(
        carrier,
        DIALOGS,
        "Name conflict",
        "A paste meets an item of the same name: Skip, Keep Both or Replace",
    )
}

fn open_with(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    window.click_entry(ARCHIVE);
    window.act(choose_app);
    window.capture(
        carrier,
        DIALOGS,
        "Open With",
        "Every application for the type, the first chosen",
    )
}

fn open_with_nothing(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    window.click_entry(ARCHIVE);
    let files = vec![stage.fixture.path(LAUNCH).join(ARCHIVE)];
    window.act(|cx, store| {
        let choices = AppChoices {
            mime: MimeId::intern("application/zstd"),
            files,
            apps: Vec::new(),
            purpose: AppsPurpose::Dialog,
        };
        finish_opening(cx, store, vec![OpenEvent::Apps(choices)]);
    });
    window.capture(
        carrier,
        DIALOGS,
        "Open With, no app",
        "No installed application opens this type",
    )
}

fn properties_of<'f>(stage: &'f Stage, entry: &str, held: Held) -> anyhow::Result<Window<'f>> {
    let mut window = stage.window_held(LAUNCH, held)?;
    window.click_entry(entry).key("Shift+F10");
    window.click(role(Role::MenuItem).name("Properties"));
    Ok(window)
}

fn file_properties(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = properties_of(stage, "keynote.pdf", Held::default())?;
    window.capture(
        carrier,
        DIALOGS,
        "Properties",
        "A file's type, size, place, times, owner and permissions",
    )
}

fn properties_pending(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let held = Held {
        inspection: true,
        ..Held::default()
    };
    let mut window = properties_of(stage, "keynote.pdf", held)?;
    window.capture(
        carrier,
        DIALOGS,
        "Properties reading",
        "What the listing knows at once; times and owner still being read",
    )
}

fn folder_properties(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let held = Held {
        folder_size: true,
        ..Held::default()
    };
    let mut window = properties_of(stage, "Renders", held)?;
    window.capture(
        carrier,
        DIALOGS,
        "Folder properties",
        "A folder while its size is measured",
    )
}

fn disk_properties(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(MOUNTED_FOLDER)?;
    window.act(show_properties);
    window.capture(
        carrier,
        DIALOGS,
        "Volume properties",
        "A folder a disk is mounted on also shows the disk's capacity and free space",
    )
}

fn ask_unlock(stage: &Stage) -> anyhow::Result<Window<'_>> {
    let mut window = stage.window(LAUNCH)?;
    plug_devices(&mut window);
    window.click(role(Role::MenuItem).text(ENCRYPTED));
    Ok(window)
}

fn submit_passphrase(window: &mut Window) {
    window.act(|cx, store| {
        store
            .drives
            .unlock
            .passphrase
            .set(cx, PASSPHRASE.to_owned());
    });
    window.click(role(Role::Button).name("Unlock"));
}

fn unlock(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = ask_unlock(stage)?;
    window.capture(
        carrier,
        DIALOGS,
        "Unlock drive",
        "A click on a locked drive asks for its passphrase",
    )
}

fn unlocking(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = ask_unlock(stage)?;
    submit_passphrase(&mut window);
    window.capture(
        carrier,
        DIALOGS,
        "Unlocking",
        "The passphrase is being checked; the drive's row spins",
    )
}

fn unlock_error(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = ask_unlock(stage)?;
    submit_passphrase(&mut window);
    let failed = DriveEvent::Finished {
        volume: volume("sda2", ENCRYPTED, false).object_path,
        name: ENCRYPTED.to_owned(),
        action: DriveAction::Open,
        result: Err(DeviceError::Failed(UNLOCK_ERROR.to_owned())),
    };
    window.act(|cx, store| finish_drives(cx, store, vec![failed]));
    window.capture(
        carrier,
        DIALOGS,
        "Unlock failed",
        "A wrong passphrase: the reason under the field",
    )
}
