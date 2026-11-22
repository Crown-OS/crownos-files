use capture::Carrier;
use crowntest::{Role, role};

use crate::{
    Stage,
    fixture::LAUNCH,
    notices::start_jobs,
    window::{Held, Window, phone, tablet},
};

const MENUS: &str = "Menus & popovers";
const LAST_ENTRY: &str = "release-notes.md";
const GAP_AFTER_LAST: f32 = 60.0;

pub fn capture(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    file_menu(stage, carrier)?;
    for (page, name, description) in [
        (
            "Open With",
            "Open With page",
            "The menu turned to the applications for the type",
        ),
        (
            "Tags",
            "Tags page",
            "Every tag, checked when the selection carries it",
        ),
        (
            "Send to",
            "Send to page",
            "The paired devices that are reachable",
        ),
    ] {
        menu_page(stage, carrier, page, name, description)?;
    }
    looking_for_apps(stage, carrier)?;
    folder_menu(stage, carrier)?;
    trash_item_menu(stage, carrier)?;
    trash_menu(stage, carrier)?;
    jobs_popover(stage, carrier)
}

fn open_file_menu<'f>(stage: &'f Stage, held: Held) -> anyhow::Result<Window<'f>> {
    let mut window = stage.window_held(LAUNCH, held)?;
    window.act(|cx, store| store.peers.paired.set(cx, vec![phone(), tablet(true)]));
    window.click_entry("keynote.pdf").key("Shift+F10");
    Ok(window)
}

fn file_menu(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = open_file_menu(stage, Held::default())?;
    window.capture(
        carrier,
        MENUS,
        "File menu",
        "Right-click on a file: open, clipboard, rename, tags, send and trash",
    )
}

fn menu_page(
    stage: &Stage,
    carrier: &mut Carrier,
    page: &str,
    name: &str,
    description: &str,
) -> anyhow::Result<()> {
    let mut window = open_file_menu(stage, Held::default())?;
    window.click(role(Role::MenuItem).name(page));
    window.capture(carrier, MENUS, name, description)
}

fn looking_for_apps(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let held = Held {
        menu_apps: true,
        ..Held::default()
    };
    let mut window = open_file_menu(stage, held)?;
    window.click(role(Role::MenuItem).name("Open With"));
    window.capture(
        carrier,
        MENUS,
        "Open With looking",
        "The applications are still being looked up",
    )
}

/// Clicks the empty space after the folder's last entry and opens the menu there.
fn open_folder_menu(window: &mut Window, last_entry: &str) {
    let area = window.app.find(role(Role::List).name("Files")).bounds;
    let last = window
        .app
        .find(role(Role::ListItem).name(last_entry))
        .bounds;
    let x = (last.max_x() + GAP_AFTER_LAST).min(area.max_x() - 8.0);
    let y = last.min_y() + 8.0;
    window.app.pointer_down(x, y).pointer_up(x, y);
    window.pump();
    window.key("Shift+F10");
}

fn folder_menu(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    open_folder_menu(&mut window, LAST_ENTRY);
    window.capture(
        carrier,
        MENUS,
        "Folder menu",
        "Right-click on the empty area: new folder, paste, hidden files and view",
    )
}

fn open_trash(stage: &Stage) -> anyhow::Result<Window<'_>> {
    let mut window = stage.window(LAUNCH)?;
    window.click(role(Role::MenuItem).name("Trash"));
    Ok(window)
}

fn trash_item_menu(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = open_trash(stage)?;
    window.click_entry("draft one.md").key("Shift+F10");
    window.capture(
        carrier,
        MENUS,
        "Trash item menu",
        "Right-click on a trashed file: restore or delete for good",
    )
}

fn trash_menu(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = open_trash(stage)?;
    open_folder_menu(&mut window, "screenshot-0412.png");
    window.capture(
        carrier,
        MENUS,
        "Trash menu",
        "Right-click on the Trash's empty area",
    )
}

fn jobs_popover(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    start_jobs(&mut window, &stage.fixture);
    window.click(role(Role::Button).name("File operations"));
    window.capture(
        carrier,
        MENUS,
        "File operations",
        "The toolbar ring opens each running job with its progress and Cancel",
    )
}
