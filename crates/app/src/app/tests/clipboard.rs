//! Cut, Copy and Paste through the system clipboard.

use crowntest::text;

use super::window::window;
use crate::{
    app::{
        file_jobs::{JobCommand, JobUpdate, finish_jobs},
        fixtures::LAUNCH,
    },
    jobs::{Job, JobEvent, JobId, Outcome, Transfer},
    util::{
        clipboard_items::read_files,
        clipboard_payload::{
            ClipboardAction, GNOME_COPIED_FILES_MIME, PLAIN_TEXT_MIME, URI_LIST_MIME,
            encode_gnome_copied_files, encode_uri_list,
        },
    },
};

#[test]
fn copy_and_cut_offer_the_types_other_file_managers_read() {
    let mut window = window();
    let keynote = window.dir(LAUNCH).join("keynote.pdf");
    window.click("keynote.pdf").key("Ctrl+C");
    let offered = window.app.clipboard().to_vec();
    let types: Vec<&str> = offered.iter().map(|(mime, _)| mime.as_str()).collect();
    assert_eq!(
        types,
        [URI_LIST_MIME, GNOME_COPIED_FILES_MIME, PLAIN_TEXT_MIME]
    );
    assert_eq!(
        read_files(&offered[1].0, &offered[1].1),
        Some((ClipboardAction::Copy, vec![keynote.clone()]))
    );
    assert_eq!(offered[2].1, keynote.as_os_str().as_encoded_bytes());
    window.key("Ctrl+X");
    let offered = window.app.clipboard().to_vec();
    assert_eq!(
        read_files(&offered[1].0, &offered[1].1),
        Some((ClipboardAction::Cut, vec![keynote]))
    );
}

#[test]
fn pasting_a_cut_moves_the_files_here_and_empties_the_clipboard_once_moved() {
    let mut window = window();
    let photo = window.dir("Downloads").join("photo.jpg");
    let cut = encode_gnome_copied_files(ClipboardAction::Cut, [photo.as_path()]);
    window
        .app
        .set_clipboard(&[(GNOME_COPIED_FILES_MIME, cut.as_bytes())]);
    window.key("Ctrl+V");
    window.app.serve_clipboard().settle();
    let into = window.dir(LAUNCH).join("photo.jpg");
    let submitted = window.job_commands();
    assert!(
        matches!(
            submitted.as_slice(),
            [JobCommand::Submit { job: Job::Move(moves), .. }]
                if *moves == [Transfer { from: photo.clone(), to: into.clone() }]
        ),
        "{submitted:?}"
    );
    let id = JobId::next();
    window.answer(|cx, store| {
        finish_jobs(
            cx,
            store,
            vec![
                JobUpdate::Submitted(id),
                JobUpdate::Event(JobEvent::Finished {
                    id,
                    outcome: Outcome::Completed,
                    undo: None,
                }),
            ],
        );
    });
    assert!(window.shows(text("Moved 1 item")));
    assert!(window.app.clipboard().is_empty());
}

#[test]
fn pasting_a_uri_list_copies_and_keeps_the_clipboard() {
    let mut window = window();
    let photo = window.dir("Downloads").join("photo.jpg");
    let list = encode_uri_list([photo.as_path()]);
    window
        .app
        .set_clipboard(&[(URI_LIST_MIME, list.as_bytes())]);
    window.key("Ctrl+V");
    window.app.serve_clipboard().settle();
    let submitted = window.job_commands();
    assert!(
        matches!(
            submitted.as_slice(),
            [JobCommand::Submit { job: Job::Copy(copies), .. }] if copies[0].from == photo
        ),
        "{submitted:?}"
    );
    let id = JobId::next();
    window.answer(|cx, store| {
        finish_jobs(
            cx,
            store,
            vec![
                JobUpdate::Submitted(id),
                JobUpdate::Event(JobEvent::Finished {
                    id,
                    outcome: Outcome::Completed,
                    undo: None,
                }),
            ],
        );
    });
    assert!(window.shows(text("Copied 1 item")));
    assert_eq!(window.app.clipboard().len(), 1);
}
