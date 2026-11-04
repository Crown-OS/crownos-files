//! Drag and drop: what a drag over each target is accepted as, what a drop
//! does, dragging files out, and spring-loaded folders.

use crowntest::{Role, role};
use crownui::{
    ext::{DropAcceptance, Modifiers},
    prelude::{DndAction, DndActions},
};

use super::window::{CTRL, SHIFT, Window, window};
use crate::{
    app::{file_jobs::JobCommand, fixtures::LAUNCH},
    jobs::Job,
    util::clipboard_payload::{GNOME_COPIED_FILES_MIME, URI_LIST_MIME, encode_uri_list},
};

fn center(window: &mut Window, query: crowntest::Query) -> (f32, f32) {
    let bounds = window.app.find(query).bounds;
    (bounds.center().x, bounds.center().y)
}

fn accepted(window: &mut Window) -> Option<DndAction> {
    window
        .app
        .drag_acceptance()
        .flatten()
        .map(|DropAcceptance { mime_type, action }| {
            assert_eq!(mime_type, URI_LIST_MIME);
            action
        })
}

#[test]
fn a_drop_from_another_app_copies_unless_shift_forces_a_move() {
    let mut window = window();
    let brand = window.entry("Brand");
    let (x, y) = center(&mut window, brand);
    window
        .app
        .drag_enter(x, y, &[URI_LIST_MIME], DndActions::COPY_MOVE);
    assert_eq!(accepted(&mut window), Some(DndAction::Copy));
    window.app.hold_modifiers(SHIFT).drag_over(x + 1.0, y);
    assert_eq!(accepted(&mut window), Some(DndAction::Move));
    window.app.hold_modifiers(Modifiers::NONE);
    let photo = window.dir("Downloads").join("photo.jpg");
    let list = encode_uri_list([photo.as_path()]);
    window
        .app
        .drop_data(x, y, URI_LIST_MIME, list.as_bytes(), DndAction::Move);
    let brand = window.dir(LAUNCH).join("Brand");
    assert!(matches!(
        window.job_commands().as_slice(),
        [JobCommand::Submit { job: Job::Move(moves), .. }]
            if moves[0].from == photo && moves[0].to == brand.join("photo.jpg")
    ));
}

#[test]
fn files_and_the_empty_area_take_drops_into_the_open_folder() {
    let mut window = window();
    let keynote = window.entry("keynote.pdf");
    let (x, y) = center(&mut window, keynote);
    window
        .app
        .drag_enter(x, y, &[URI_LIST_MIME], DndActions::COPY_MOVE);
    assert_eq!(accepted(&mut window), Some(DndAction::Copy));
    let photo = window.dir("Downloads").join("photo.jpg");
    let list = encode_uri_list([photo.as_path()]);
    window
        .app
        .drop_data(x, y, URI_LIST_MIME, list.as_bytes(), DndAction::Copy);
    let launch = window.dir(LAUNCH);
    assert!(matches!(
        window.job_commands().as_slice(),
        [JobCommand::Submit { job: Job::Copy(copies), .. }]
            if copies[0].to == launch.join("photo.jpg")
    ));
}

#[test]
fn drags_without_files_are_refused() {
    let mut window = window();
    let brand = window.entry("Brand");
    let (x, y) = center(&mut window, brand);
    window
        .app
        .drag_enter(x, y, &["text/plain"], DndActions::COPY_MOVE);
    assert_eq!(window.app.drag_acceptance(), None);
}

#[test]
fn dragging_entries_out_offers_them_and_moves_them_within_the_disk() {
    let mut window = window();
    std::fs::write(window.dir(LAUNCH).join("keynote.pdf"), b"%PDF").unwrap();
    let keynote_query = window.entry("keynote.pdf");
    let (x, y) = center(&mut window, keynote_query);
    window
        .app
        .pointer_down(x, y)
        .pointer_move(x + 20.0, y + 20.0)
        .pointer_move(x + 40.0, y + 40.0);
    let source = window
        .app
        .ui()
        .take_drag_request()
        .expect("dragging an entry starts a drag");
    let keynote = window.dir(LAUNCH).join("keynote.pdf");
    let offered: Vec<&str> = source.items.iter().map(|(mime, _)| mime.as_str()).collect();
    assert!(offered.contains(&URI_LIST_MIME) && offered.contains(&GNOME_COPIED_FILES_MIME));
    assert!(
        String::from_utf8_lossy(&source.items[0].1).contains("keynote.pdf"),
        "{keynote:?}"
    );
    let icon = source.icon.expect("the drag has an icon");
    assert_eq!((icon.logical_width, icon.logical_height), (64, 64));
    assert_eq!(source.actions, DndActions::COPY_MOVE);
    assert_eq!(window.selected(), ["keynote.pdf"]);

    let renders = window.entry("Renders");
    let (bx, by) = center(&mut window, renders);
    window
        .app
        .drag_enter(bx, by, &[URI_LIST_MIME], DndActions::COPY_MOVE);
    assert_eq!(accepted(&mut window), Some(DndAction::Move));
    window.app.hold_modifiers(CTRL).drag_over(bx + 1.0, by);
    assert_eq!(accepted(&mut window), Some(DndAction::Copy));
    window.app.hold_modifiers(Modifiers::NONE).drag_over(x, y);
    assert_eq!(
        accepted(&mut window),
        None,
        "a move into the folder it is already in is refused"
    );
}

#[test]
fn resting_a_drag_on_a_folder_opens_it() {
    let mut window = window();
    let renders = window.entry("Renders");
    let (x, y) = center(&mut window, renders);
    window
        .app
        .drag_enter(x, y, &[URI_LIST_MIME], DndActions::COPY_MOVE)
        .advance_ms(400);
    assert!(window.app.exists(window.entry("keynote.pdf")), "not yet");
    window.app.advance_ms(400);
    assert!(!window.app.exists(window.entry("keynote.pdf")));
    assert!(window.app.exists(role(Role::Button).name("Renders")));
}

#[test]
fn the_trash_place_takes_files_to_the_trash() {
    let mut window = window();
    let trash = role(Role::MenuItem).name("Trash");
    let (x, y) = center(&mut window, trash);
    window
        .app
        .drag_enter(x, y, &[URI_LIST_MIME], DndActions::COPY_MOVE);
    assert_eq!(accepted(&mut window), Some(DndAction::Move));
    let keynote = window.dir(LAUNCH).join("keynote.pdf");
    let list = encode_uri_list([keynote.as_path()]);
    window
        .app
        .drop_data(x, y, URI_LIST_MIME, list.as_bytes(), DndAction::Move);
    assert!(matches!(
        window.job_commands().as_slice(),
        [JobCommand::Submit { job: Job::Trash(paths), .. }] if *paths == [keynote]
    ));
}
