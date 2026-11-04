//! Browsing: listing, selecting, keyboard, view modes, search, history,
//! renaming, the rubber band and the snapshots of both views.

use std::path::Path;

use crowntest::{Role, name, role, text};
use crownui::{
    ext::Modifiers,
    kit::{Theme, provide_theme},
    prelude::component,
};

use super::window::{CTRL, HEIGHT, LISTED, SHIFT, WIDTH, window, window_with};
use crate::{
    app::{
        fixtures::{LAUNCH, mockup_home, mockup_store},
        selection::click,
        shell::shell,
    },
    fs::engine::EngineCmd,
};

#[test]
fn the_grid_shows_every_visible_entry_with_its_size() {
    let mut window = window();
    for entry in LISTED {
        assert!(window.app.exists(window.entry(entry)), "{entry} is listed");
    }
    assert!(
        !window.app.exists(window.entry(".git")),
        "hidden files stay hidden"
    );
    assert!(window.shows(text("18 MB")));
    assert!(window.shows(text("4.8 GB")));
    assert_eq!(window.status(), "16 items");
    assert!(window.shows(text("588 GB of 1 TB used")));
}

#[test]
fn clicks_select_one_toggle_with_ctrl_and_extend_with_shift() {
    let mut window = window();
    window.click("benchmarks.csv");
    assert_eq!(window.selected(), ["benchmarks.csv"]);
    window.click_with("keynote.pdf", CTRL);
    assert_eq!(window.selected(), ["benchmarks.csv", "keynote.pdf"]);
    window.click_with("benchmarks.csv", CTRL);
    assert_eq!(window.selected(), ["keynote.pdf"]);
    window
        .click("compositor.rs")
        .click_with("interview.flac", SHIFT);
    assert_eq!(
        window.selected(),
        [
            "compositor.rs",
            "crownbook-hero.png",
            "crownos-2026.11-x86_64.iso",
            "interview.flac"
        ]
    );
    assert_eq!(window.status(), "16 items, 4 selected");
}

#[test]
fn arrow_keys_walk_the_grid_by_its_columns() {
    let mut window = window();
    window.click("Brand");
    let first = window.app.find(window.entry("Brand")).bounds;
    window.key("ArrowRight");
    assert_eq!(window.selected(), ["Press Kit"]);
    window.key("ArrowLeft").key("ArrowDown");
    let below = window.selected();
    assert_eq!(below.len(), 1);
    let bounds = window.app.find(window.entry(below[0])).bounds;
    assert_eq!(bounds.origin.x, first.origin.x, "Down keeps the column");
    assert!(bounds.origin.y > first.origin.y, "Down moves a row");
    window.key("ArrowUp");
    assert_eq!(window.selected(), ["Brand"]);
    window.key("Shift+ArrowRight").key("Shift+ArrowRight");
    assert_eq!(window.selected(), ["Brand", "Press Kit", "Renders"]);
    window.key("End");
    assert_eq!(window.selected(), ["release-notes.md"]);
    window.key("Home");
    assert_eq!(window.selected(), ["Brand"]);
    window.key("Ctrl+A");
    assert_eq!(window.selected().len(), 16);
    window.key("Escape");
    assert!(window.selected().is_empty());
}

#[test]
fn typing_jumps_to_the_first_name_with_that_prefix() {
    let mut window = window();
    window.click("Brand").key("k");
    assert_eq!(window.selected(), ["keynote.pdf"]);
}

#[test]
fn the_view_switch_and_its_shortcuts_change_between_icons_and_rows() {
    let mut window = window();
    assert!(
        !window.shows(text("FLAC audio")),
        "the grid has no Kind column"
    );
    window.key("Ctrl+2");
    assert!(window.shows(text("FLAC audio")));
    assert!(window.shows(text("Oct 24, 2026")));
    window.app.click(name("Icons")).settle();
    assert!(!window.shows(text("FLAC audio")));
    window.app.click(name("List")).settle();
    assert!(window.shows(text("Disk Image")));
    window.key("Ctrl+1");
    assert!(!window.shows(text("Disk Image")));
}

#[test]
fn ctrl_h_shows_and_hides_hidden_files() {
    let mut window = window();
    window.click("Brand").key("Ctrl+H");
    assert!(window.app.exists(window.entry(".git")));
    assert_eq!(window.status(), "17 items, 1 selected");
    window.key("Ctrl+H");
    assert!(!window.app.exists(window.entry(".git")));
}

#[test]
fn the_focused_search_widens_without_covering_the_history_buttons() {
    let mut window = window();
    let resting = window.app.find(role(Role::SearchField)).bounds;
    window
        .app
        .focus(role(Role::SearchField))
        .type_text("a")
        .settle();
    let focused = window.app.find(role(Role::SearchField)).bounds;
    let up = window.app.find(role(Role::Button).name("Up")).bounds;
    let view = window.app.find(role(Role::Group).name("View")).bounds;
    assert!(focused.size.width > resting.size.width);
    assert!(focused.min_x() >= up.max_x(), "{focused:?} overlaps {up:?}");
    assert!(
        focused.max_x() <= view.min_x(),
        "{focused:?} overlaps {view:?}"
    );
}

#[test]
fn the_search_box_filters_the_folder_with_smart_case() {
    let mut window = window();
    window
        .app
        .focus(role(Role::SearchField))
        .type_text("launch")
        .settle();
    let listed: Vec<_> = LISTED
        .into_iter()
        .filter(|entry| window.app.exists(window.entry(entry)))
        .collect();
    assert_eq!(
        listed,
        [
            "launch-assets.tar.zst",
            "launch-film.webm",
            "launch-plan.odt"
        ]
    );
    assert_eq!(window.status(), "3 items");
    window.app.type_text("-P").settle();
    assert_eq!(window.status(), "0 items");
    assert!(window.shows(text("Searching…")));
    let Some(walk) = window.last_search() else {
        panic!("typing asks for a walk");
    };
    assert_eq!(walk.query, "launch-P");
    window.finish_search(&walk, &[]);
    assert!(
        window.last_search().is_none(),
        "an ended walk starts no other"
    );
    assert!(window.shows(text("Nothing here matches “launch-P”")));
    window.key("Escape");
    assert_eq!(window.status(), "16 items");
}

#[test]
fn search_finds_matches_in_every_subfolder_and_escape_returns_to_the_folder() {
    let mut window = window();
    window
        .app
        .focus(role(Role::SearchField))
        .type_text("Render")
        .settle();
    let Some(walk) = window.last_search() else {
        panic!("typing asks for a walk");
    };
    assert_eq!(
        walk.scope,
        window.dir(LAUNCH),
        "the walk looks under the open folder"
    );
    let nested = format!("{LAUNCH}/Renders/Render notes");
    std::fs::create_dir_all(window.dir(&nested)).unwrap();
    window.finish_search(&walk, &[&format!("{LAUNCH}/Renders"), &nested]);
    assert!(window.app.exists(window.entry("Renders")));
    assert!(window.app.exists(window.entry("Render notes")));
    assert_eq!(window.status(), "2 items");
    window.key("Escape");
    assert_eq!(window.status(), "16 items");
    assert_eq!(window.store.dir(window.app.ui().cx()), window.dir(LAUNCH));
}

#[test]
fn the_path_bar_and_history_buttons_navigate() {
    let mut window = window();
    window.engine_commands();
    assert!(
        window
            .app
            .find(role(Role::Button).name("Back"))
            .states
            .disabled
    );
    window
        .app
        .click(role(Role::Button).name("Documents"))
        .settle();
    let documents = window.dir("Documents");
    let commands = window.engine_commands();
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, EngineCmd::Load(dir) if *dir == documents)),
        "{commands:?}"
    );
    assert!(commands.iter().any(
        |command| matches!(command, EngineCmd::Watch(dirs) if dirs.as_slice() == [documents.clone()])
    ));
    assert!(!window.app.exists(role(Role::Button).name("Launch")));
    assert!(
        !window
            .app
            .find(role(Role::Button).name("Back"))
            .states
            .disabled
    );

    window.app.click(role(Role::Button).name("Back")).settle();
    let launch = window.dir(LAUNCH);
    assert!(
        window
            .engine_commands()
            .iter()
            .any(|command| matches!(command, EngineCmd::Revalidate { dir, .. } if *dir == launch))
    );
    assert!(
        window.app.exists(window.entry("keynote.pdf")),
        "the cached listing shows at once"
    );
    assert!(
        !window
            .app
            .find(role(Role::Button).name("Forward"))
            .states
            .disabled
    );
}

#[test]
fn enter_opens_a_folder_and_backspace_returns_with_it_selected() {
    let mut window = window();
    window.click("Renders").key("Enter");
    assert!(
        window.app.exists(role(Role::Button).name("Renders")),
        "Renders is the current crumb"
    );
    assert!(!window.app.exists(window.entry("keynote.pdf")));
    window.key("Backspace");
    assert_eq!(window.selected(), ["Renders"]);
}

#[test]
fn the_up_button_opens_the_parent_with_the_folder_left_selected() {
    let mut window = window();
    window.click("Renders").key("Enter");
    window.app.click(role(Role::Button).name("Up")).settle();
    assert_eq!(window.store.dir(window.app.ui().cx()), window.dir(LAUNCH));
    window.focus_files();
    assert_eq!(window.selected(), ["Renders"]);
}

#[test]
fn the_details_panel_describes_the_selection() {
    let mut window = window();
    assert!(window.shows(text("Folder · 16 items")));
    window.click("keynote.pdf");
    assert!(window.shows(text("PDF Document · 18 MB")));
    assert!(window.shows(text("Documents › Launch")));
    assert!(window.shows(text("Modified")));
    assert!(window.shows(text("Oct 30, 2026")), "created");
    assert!(window.app.exists(role(Role::Button).name("Open")));
    assert!(
        window
            .app
            .exists(role(Role::Button).name("Send to Pixel 9 Pro"))
    );
    window.click_with("benchmarks.csv", CTRL);
    assert!(window.shows(text("2 items")));
}

#[test]
fn f2_renames_inline_and_escape_cancels() {
    let mut window = window();
    window.click("keynote.pdf").key("F2");
    let field = role(Role::TextField).name("Rename");
    assert!(window.app.exists(field.clone()));
    assert_eq!(
        window.app.find(field.clone()).value.as_deref(),
        Some("keynote.pdf")
    );
    window.key("Escape");
    assert!(!window.app.exists(field));
    assert!(window.app.exists(window.entry("keynote.pdf")));
}

#[test]
fn ctrl_l_edits_the_path_and_escape_restores_the_crumbs() {
    let mut window = window();
    window.click("Brand").key("Ctrl+L");
    let field = role(Role::TextField).name("Location");
    assert!(window.app.exists(field.clone()));
    let typed = window.app.find(field.clone()).value.unwrap_or_default();
    assert!(typed.ends_with(LAUNCH), "{typed}");
    window.key("Escape");
    assert!(!window.app.exists(field));
    assert!(window.app.exists(role(Role::Button).name("Launch")));
}

#[test]
fn the_context_menu_offers_rename_for_the_selection() {
    let mut window = window();
    window.click("keynote.pdf").key("Shift+F10");
    assert!(window.app.exists(role(Role::Menu)));
    window
        .app
        .click(role(Role::MenuItem).name("Rename…"))
        .settle();
    assert!(!window.app.exists(role(Role::Menu)));
    assert!(window.app.exists(role(Role::TextField).name("Rename")));
}

#[test]
fn dragging_over_the_empty_area_selects_with_a_rubber_band() {
    let mut window = window();
    let list = window.app.find(role(Role::List).name("Files")).bounds;
    let last = window.app.find(window.entry("release-notes.md")).bounds;
    let first_of_row = window.app.find(window.entry("Brand")).bounds;
    let below = last.max_y() + 20.0;
    assert!(below < list.max_y(), "there is empty space under the grid");
    window
        .app
        .pointer_down(first_of_row.min_x() + 5.0, below)
        .pointer_move(first_of_row.min_x() + 40.0, below - 30.0)
        .pointer_move(first_of_row.min_x() + 60.0, last.center().y)
        .pointer_up(first_of_row.min_x() + 60.0, last.center().y)
        .settle();
    let selected = window.selected();
    assert!(
        !selected.is_empty(),
        "the band selected the last row's first column"
    );
    assert!(selected.len() < 16);
}

#[test]
fn the_light_and_dark_palettes_both_draw_the_window() {
    let mut dark = window_with(Theme::dark());
    let mut light = window_with(Theme::light());
    dark.click("keynote.pdf");
    light.click("keynote.pdf");
    assert_eq!(
        dark.app.semantics_snapshot(),
        light.app.semantics_snapshot()
    );
    assert_ne!(dark.app.frame_snapshot(), light.app.frame_snapshot());
}

#[test]
fn snapshot_of_the_grid() {
    let mut window = window();
    window.click("interview.flac");
    insta::assert_snapshot!("grid_semantics", window.app.semantics_snapshot());
    insta::assert_snapshot!("grid_layout", window.app.layout_snapshot());
}

#[test]
fn snapshot_of_the_list() {
    let mut window = window();
    window.click("interview.flac").key("Ctrl+2");
    insta::assert_snapshot!("list_semantics", window.app.semantics_snapshot());
}

/// Renders the mockup's window through Impeller's offscreen target into
/// `$CROWNOS_FILES_RENDER_DIR/<shot>.png`, one shot per run (`$CROWNOS_FILES_RENDER_SHOT`:
/// `grid`, `list` or `light`), for comparing with the landing page.
#[test]
#[ignore = "needs a GPU; writes PNGs for manual inspection"]
fn render_the_mockup() {
    use crownui::{
        ext::{InlineApp, TextSystem},
        kit::{INTER_FAMILY, inter_font},
        prelude::Size,
    };
    use impeller::{ImpellerBackend, Offscreen};
    use taffy_layout::TaffyEngine;

    let Some(out) = std::env::var_os("CROWNOS_FILES_RENDER_DIR") else {
        return;
    };
    let wanted = std::env::var("CROWNOS_FILES_RENDER_SHOT").unwrap_or_else(|_| "grid".into());
    let size = Size::new(WIDTH, HEIGHT);
    for (shot, theme, list) in [
        ("grid", Theme::dark(), false),
        ("list", Theme::dark(), true),
        ("light", Theme::light(), false),
    ]
    .into_iter()
    .filter(|(shot, ..)| *shot == wanted)
    {
        let home = mockup_home();
        let path = home.path().to_path_buf();
        let root = component(move |cx| {
            provide_theme(cx, theme);
            let store = mockup_store(cx, &path);
            if list {
                store.view_mode.set(cx, crownconfig::ViewMode::List);
            }
            let aurora = store
                .folder
                .with(cx, |folder| folder.position_of("aurora-wallpaper.avif"));
            if let Some(position) = aurora {
                click(cx, store, position, Modifiers::NONE);
            }
            shell(cx, store)
        });
        let surface = Offscreen {
            width: WIDTH as u32,
            height: HEIGHT as u32,
            scale: 1.0,
        };
        let mut app = InlineApp::<ImpellerBackend, TaffyEngine>::with_surface(
            root,
            TaffyEngine::new(),
            size,
            1.0,
            &surface,
        )
        .unwrap();
        app.text_cache()
            .system_mut()
            .register_font(inter_font().to_vec(), Some(INTER_FAMILY))
            .unwrap();
        app.settle().unwrap();
        let pixels = app.renderer_mut().read_pixels().unwrap();
        let png = impeller::encode_png(&pixels).unwrap();
        std::fs::write(Path::new(&out).join(format!("{shot}.png")), png).unwrap();
    }
}
