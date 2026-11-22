use capture::Carrier;
use crownos_files::app::{
    listing::apply_ops,
    virtual_folder::{VirtualFolder, VirtualListing, apply_listing},
};
use crownos_files_core::model::files_op::FilesOp;
use crowntest::{Role, role};

use crate::{
    Stage,
    fixture::LAUNCH,
    window::{Held, Window},
};

const MAIN: &str = "Main window";
const SEARCH: &str = "Search";
const PLACES: &str = "Sidebar places";
const EMPTY: &str = "Empty & error states";

/// What a walk under home finds for "launch".
const LAUNCH_MATCHES: [&str; 4] = [
    LAUNCH,
    "Documents/Launch/launch-film.webm",
    "Documents/Launch/launch-assets.tar.zst",
    "Documents/Launch/launch-plan.odt",
];

pub fn capture(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    icons_view(stage, carrier)?;
    file_selected(stage, carrier)?;
    several_selected(stage, carrier)?;
    list_view(stage, carrier)?;
    rubber_band(stage, carrier)?;
    rename(stage, carrier)?;
    rename_in_list(stage, carrier)?;
    path_entry(stage, carrier)?;
    searching(stage, carrier)?;
    search_results(stage, carrier)?;
    no_matches(stage, carrier)?;
    tag_search(stage, carrier)?;
    for (place, description) in [
        ("Home", "The home folder: the user folders and a few files"),
        ("Downloads", "Downloads in the list view"),
        ("Pictures", "Pictures: photo thumbnails in the icon view"),
        ("Videos", "Videos: poster frames with their length"),
        ("Music", "Music: audio files and an album folder"),
        ("Recents", "Recents: recently used files from every folder"),
        ("Trash", "The Trash with its Restore / Empty Trash strip"),
    ] {
        place_view(stage, carrier, place, description)?;
    }
    empty_folder(stage, carrier)?;
    loading(stage, carrier)?;
    unreadable(stage, carrier)?;
    empty_trash(stage, carrier)
}

fn icons_view(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    window.capture(
        carrier,
        MAIN,
        "Icons view",
        "Documents › Launch in the icon view, the folder described on the right",
    )
}

fn file_selected(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    window.click_entry("aurora-wallpaper.avif");
    window.capture(
        carrier,
        MAIN,
        "File selected",
        "One image selected: preview, dates, tags and the Open / Send to buttons",
    )
}

fn several_selected(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    window.click_entry("keynote.pdf");
    window.key("Shift+ArrowRight").key("Shift+ArrowRight");
    window.capture(
        carrier,
        MAIN,
        "Several selected",
        "Three files selected: their count and total size",
    )
}

fn list_view(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    window.key("Ctrl+2");
    window.click_entry("keynote.pdf");
    window.capture(
        carrier,
        MAIN,
        "List view",
        "The list view with kind, size and date columns, one file selected",
    )
}

fn rubber_band(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    let area = window.app.find(role(Role::List).name("Files")).bounds;
    let target = window
        .app
        .find(role(Role::ListItem).name("compositor.rs"))
        .bounds
        .center();
    let (start_x, start_y) = (area.max_x() - 24.0, area.max_y() - 24.0);
    window.app.pointer_down(start_x, start_y);
    for step in 1..=8 {
        let progress = step as f32 / 8.0;
        window.app.pointer_move(
            start_x + (target.x - start_x) * progress,
            start_y + (target.y - start_y) * progress,
        );
    }
    window.app.settle();
    window.capture_here(
        carrier,
        MAIN,
        "Rubber band",
        "Dragging over the empty area selects every tile the band touches",
    )
}

fn rename(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    window.click_entry("launch-plan.odt").key("F2");
    window.capture(carrier, MAIN, "Rename", "F2 puts an editor over the name")
}

fn rename_in_list(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    window.key("Ctrl+2");
    window.click_entry("launch-plan.odt").key("F2");
    window.capture(
        carrier,
        MAIN,
        "Rename in list",
        "F2 in the list view edits the name in its row",
    )
}

fn path_entry(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    window.key("Ctrl+L");
    window.act(|cx, store| store.edit_text.set(cx, "~/Documents/Launch".to_owned()));
    window.capture(
        carrier,
        MAIN,
        "Path entry",
        "Ctrl+L turns the path bar into a field for typing a location",
    )
}

fn type_search<'f>(stage: &'f Stage, start: &str, query: &str) -> anyhow::Result<Window<'f>> {
    let mut window = stage.window(start)?;
    window.key("Ctrl+F");
    window.app.type_text(query);
    window.pump();
    Ok(window)
}

fn searching(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = type_search(stage, "", "launch")?;
    window.capture(
        carrier,
        SEARCH,
        "Searching",
        "The search box widens; the walk under home has found nothing yet",
    )
}

fn search_results(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = type_search(stage, "", "launch")?;
    let entries = LAUNCH_MATCHES
        .iter()
        .filter_map(|relative| stage.fixture.absolute_entry(relative))
        .collect();
    window.act(|cx, store| {
        apply_ops(
            cx,
            store,
            vec![FilesOp::Upsert {
                dir: VirtualFolder::Search.path(),
                entries,
            }],
        );
        store.searching.active.set(cx, false);
    });
    window.click_entry("launch-film.webm");
    window.capture(
        carrier,
        SEARCH,
        "Search results",
        "Everything under home matching “launch”, one result selected",
    )
}

fn no_matches(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = type_search(stage, LAUNCH, "quarterly report")?;
    window.act(|cx, store| store.searching.active.set(cx, false));
    window.capture(
        carrier,
        SEARCH,
        "No matches",
        "The walk ended without a match",
    )
}

fn tag_search(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = type_search(stage, LAUNCH, "tag:Brand")?;
    window.capture(
        carrier,
        SEARCH,
        "Tag filter",
        "“tag:Brand” filters the folder to what carries the tag",
    )
}

fn place_view(
    stage: &Stage,
    carrier: &mut Carrier,
    place: &str,
    description: &str,
) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    if place == "Downloads" {
        window.key("Ctrl+2");
    }
    window.click(role(Role::MenuItem).name(place));
    window.capture(carrier, PLACES, place, description)
}

fn empty_folder(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    window.click(role(Role::MenuItem).name("Desktop"));
    window.capture(
        carrier,
        EMPTY,
        "Empty folder",
        "A folder with nothing in it",
    )
}

fn loading(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let held = Held {
        listing: true,
        ..Held::default()
    };
    let mut window = stage.window_held(LAUNCH, held)?;
    window.capture(
        carrier,
        EMPTY,
        "Loading",
        "The folder before its listing arrives",
    )
}

fn unreadable(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window("Documents")?;
    window
        .app
        .double_click(role(Role::ListItem).name("Taxes 2026"));
    window.pump();
    window.capture(
        carrier,
        EMPTY,
        "Folder can't be opened",
        "Listing a folder failed: permission denied",
    )
}

fn empty_trash(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    window.click(role(Role::MenuItem).name("Trash"));
    window.act(|cx, store| {
        apply_listing(
            cx,
            store,
            VirtualListing {
                folder: VirtualFolder::Trash,
                entries: Vec::new(),
                trashed: Vec::new(),
            },
        );
    });
    window.capture(
        carrier,
        EMPTY,
        "Trash, empty",
        "Nothing in the Trash: its buttons are disabled",
    )
}
