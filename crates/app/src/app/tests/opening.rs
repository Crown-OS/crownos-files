//! Opening files: quick look for what crownos-preview shows, applications
//! for the rest, the "Open With" dialog and submenu, and the viewer and the
//! selection following each other.

use crowntest::{Role, role, text};

use super::window::{Window, drain, window};
use crate::{
    app::{
        fixtures::LAUNCH,
        opener::{AppChoice, AppChoices, AppsPurpose, OpenCommand, OpenEvent},
        opening::finish_opening,
        quick_look::{QuickLookCommand, finish_quick_look},
    },
    model::mime_id::MimeId,
    preview::PreviewEvent,
    thumbs::Frame,
};

/// The mockup's previewable files, in view order.
const PREVIEWABLE: [&str; 9] = [
    "aurora-wallpaper.avif",
    "benchmarks.csv",
    "compositor.rs",
    "crownbook-hero.png",
    "interview.flac",
    "keynote.pdf",
    "launch-film.webm",
    "palette.svg",
    "release-notes.md",
];

fn viewer(window: &mut Window) -> Vec<QuickLookCommand> {
    drain(&mut window.workers.viewer)
}

fn opener(window: &mut Window) -> Vec<OpenCommand> {
    drain(&mut window.workers.opener)
}

#[test]
fn enter_previews_a_previewable_file_among_its_previewable_neighbours() {
    let mut window = window();
    window.click("keynote.pdf").key("Enter");
    let launch = window.dir(LAUNCH);
    let expected: Vec<String> = PREVIEWABLE
        .iter()
        .map(|name| launch.join(name).display().to_string())
        .collect();
    assert_eq!(
        viewer(&mut window),
        [QuickLookCommand::Show {
            paths: expected,
            index: 5
        }]
    );
    assert!(opener(&mut window).is_empty());
}

#[test]
fn enter_hands_other_files_to_their_application() {
    let mut window = window();
    window.click("launch-assets.tar.zst").key("Enter");
    assert!(viewer(&mut window).is_empty());
    let sent = opener(&mut window);
    assert!(
        matches!(
            sent.as_slice(),
            [OpenCommand::Launch { targets, .. }]
                if targets.len() == 1 && targets[0].path.ends_with("launch-assets.tar.zst")
        ),
        "{sent:?}"
    );
}

#[test]
fn quick_look_and_the_selection_follow_each_other() {
    let mut window = window();
    window.click("aurora-wallpaper.avif").key("Space");
    assert!(matches!(
        viewer(&mut window).as_slice(),
        [QuickLookCommand::Show { index: 0, .. }]
    ));
    window.key("ArrowRight");
    assert_eq!(viewer(&mut window), [QuickLookCommand::Select(1)]);
    window.answer(|cx, store| {
        finish_quick_look(cx, store, vec![PreviewEvent::Navigated { index: 5 }]);
    });
    assert_eq!(window.selected(), ["keynote.pdf"]);
    assert!(
        viewer(&mut window).is_empty(),
        "the viewer is not told where it already is"
    );
    window.key("ArrowRight");
    assert!(
        viewer(&mut window).is_empty(),
        "the archive is not in the viewer's list"
    );
    window.answer(|cx, store| finish_quick_look(cx, store, vec![PreviewEvent::Closed]));
    window.key("Space");
    assert!(
        viewer(&mut window).is_empty(),
        "an archive cannot be quick-looked"
    );
    window.key("ArrowLeft").key("Space");
    assert!(matches!(
        viewer(&mut window).as_slice(),
        [QuickLookCommand::Show { index: 5, .. }]
    ));
    window.key("Space");
    assert_eq!(viewer(&mut window), [QuickLookCommand::Close]);
}

fn choices(window: &Window, purpose: AppsPurpose) -> AppChoices {
    let icon = Frame::new(2, 2, vec![255; 16]);
    AppChoices {
        mime: MimeId::intern("application/zstd"),
        files: vec![
            window
                .dir(LAUNCH)
                .join("launch-assets.tar.zst")
                .to_path_buf(),
        ],
        apps: vec![
            AppChoice {
                id: "org.example.Archiver.desktop".into(),
                name: "Archiver".to_owned(),
                icon,
            },
            AppChoice {
                id: "org.example.Editor.desktop".into(),
                name: "Editor".to_owned(),
                icon: None,
            },
        ],
        purpose,
    }
}

#[test]
fn the_open_with_dialog_opens_the_chosen_app_and_can_make_it_the_default() {
    let mut window = window();
    let offered = choices(&window, AppsPurpose::Dialog);
    let (mime, files) = (offered.mime, offered.files.clone());
    window.answer(|cx, store| finish_opening(cx, store, vec![OpenEvent::Apps(offered)]));
    assert!(window.shows(text("Open “launch-assets.tar.zst” with")));
    assert_eq!(
        window
            .app
            .find(role(Role::ListItem).name("Archiver"))
            .value
            .as_deref(),
        Some("selected")
    );
    window
        .app
        .click(role(Role::ListItem).name("Editor"))
        .click(role(Role::Switch).name("Always open this type of file with it"))
        .click(role(Role::Button).name("Open"))
        .settle();
    assert_eq!(
        opener(&mut window),
        [OpenCommand::OpenWith {
            app: "org.example.Editor.desktop".into(),
            files,
            default_for: Some(mime),
        }]
    );
    assert!(!window.shows(text("Open “launch-assets.tar.zst” with")));
}

#[test]
fn the_context_menus_open_with_page_lists_the_apps_for_the_type() {
    let mut window = window();
    window.click("launch-assets.tar.zst").key("Shift+F10");
    assert!(matches!(
        opener(&mut window).as_slice(),
        [OpenCommand::Apps {
            purpose: AppsPurpose::Menu,
            ..
        }]
    ));
    let offered = choices(&window, AppsPurpose::Menu);
    let files = offered.files.clone();
    window.answer(|cx, store| finish_opening(cx, store, vec![OpenEvent::Apps(offered)]));
    window
        .app
        .click(role(Role::MenuItem).name("Open With"))
        .settle();
    assert!(window.app.exists(role(Role::Menu)), "the menu stays open");
    window
        .app
        .click(role(Role::MenuItem).name("Archiver"))
        .settle();
    assert_eq!(
        opener(&mut window),
        [OpenCommand::OpenWith {
            app: "org.example.Archiver.desktop".into(),
            files,
            default_for: None,
        }]
    );
    assert!(!window.app.exists(role(Role::Menu)));
}
