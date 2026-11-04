//! File jobs: trash and delete with confirmation, undo, conflicts, the
//! jobs popover and the retry with privileges.

use std::{io, path::PathBuf};

use crowntest::{Role, role, text};
use tokio::sync::oneshot;

use super::window::{Window, window};
use crate::{
    app::{
        file_jobs::{JobCommand, JobUpdate, finish_jobs},
        fixtures::LAUNCH,
    },
    jobs::{
        ConflictPolicy, ConflictReply, Job, JobError, JobEvent, JobId, JobKind, Outcome, Priority,
        Reversible,
    },
    model::trash::{TrashInfo, TrashedItem},
};
use crownos_files_devices::privileged::PrivilegedRequest;

fn events(window: &mut Window, updates: Vec<JobUpdate>) {
    window.answer(|cx, store| finish_jobs(cx, store, updates));
}

fn finished(id: JobId, undo: Option<Reversible>) -> JobUpdate {
    JobUpdate::Event(JobEvent::Finished {
        id,
        outcome: Outcome::Completed,
        undo,
    })
}

#[test]
fn delete_moves_to_the_trash_and_undo_restores_it() {
    let mut window = window();
    let keynote = window.dir(LAUNCH).join("keynote.pdf");
    window.click("keynote.pdf").key("Delete");
    assert!(matches!(
        window.job_commands().as_slice(),
        [JobCommand::Submit { job: Job::Trash(paths), .. }] if *paths == [keynote.clone()]
    ));
    let id = JobId::next();
    let trashed = TrashedItem {
        trash: window.home.path().join("Trash").into(),
        name: "keynote.pdf".into(),
        info: TrashInfo {
            original: keynote,
            deleted_at: None,
        },
        is_dir: false,
        size: None,
    };
    events(
        &mut window,
        vec![
            JobUpdate::Submitted(id),
            finished(id, Some(Reversible::Trash(vec![trashed.clone()]))),
        ],
    );
    assert!(window.shows(text("Moved 1 item to the Trash")));
    window.key("Ctrl+Z");
    assert!(matches!(
        window.job_commands().as_slice(),
        [JobCommand::Submit { job: Job::Restore(restored), priority: Priority::High }]
            if *restored == [trashed.clone()]
    ));
    let undo = JobId::next();
    events(
        &mut window,
        vec![
            JobUpdate::Submitted(undo),
            finished(undo, Some(Reversible::Trash(vec![trashed]))),
        ],
    );
    window.key("Ctrl+Z");
    assert!(
        window.job_commands().is_empty(),
        "an undo is not undone again"
    );
}

#[test]
fn shift_delete_asks_first_and_deletes_on_confirmation() {
    let mut window = window();
    let keynote = window.dir(LAUNCH).join("keynote.pdf");
    window
        .click("keynote.pdf")
        .click_with("benchmarks.csv", super::window::CTRL)
        .key("Shift+Delete");
    assert!(window.shows(text("Delete 2 items permanently?")));
    assert!(window.job_commands().is_empty());
    window.app.click(role(Role::Button).name("Delete")).settle();
    assert!(matches!(
        window.job_commands().as_slice(),
        [JobCommand::Submit { job: Job::Delete(paths), .. }] if paths.contains(&keynote)
    ));
    assert!(!window.shows(text("Delete 2 items permanently?")));
}

#[test]
fn without_confirmation_shift_delete_deletes_at_once() {
    let mut window = window();
    window.answer(|cx, store| store.confirm_permanent_delete.set(cx, false));
    window.click("keynote.pdf").key("Shift+Delete");
    assert!(matches!(
        window.job_commands().as_slice(),
        [JobCommand::Submit {
            job: Job::Delete(_),
            ..
        }]
    ));
}

fn conflict(window: &mut Window, id: JobId, name: &str) -> oneshot::Receiver<ConflictReply> {
    let (reply, answer) = oneshot::channel();
    let target = window.dir(LAUNCH).join(name);
    events(
        window,
        vec![JobUpdate::Event(JobEvent::Conflict {
            id,
            source: PathBuf::from("/elsewhere").join(name),
            target,
            reply,
        })],
    );
    answer
}

#[test]
fn the_conflict_dialog_answers_each_conflict_in_turn() {
    let mut window = window();
    let id = JobId::next();
    let mut first = conflict(&mut window, id, "keynote.pdf");
    let mut second = conflict(&mut window, id, "compositor.rs");
    assert!(window.shows(text("“keynote.pdf” already exists")));
    window
        .app
        .click(role(Role::Button).name("Keep Both"))
        .settle();
    assert_eq!(
        first.try_recv(),
        Ok(ConflictReply {
            policy: ConflictPolicy::KeepBoth,
            apply_to_all: false
        })
    );
    assert!(window.shows(text("“compositor.rs” already exists")));
    window
        .app
        .click(role(Role::Switch).name("Do this for every conflict"))
        .click(role(Role::Button).name("Replace"))
        .settle();
    assert_eq!(
        second.try_recv(),
        Ok(ConflictReply {
            policy: ConflictPolicy::Replace,
            apply_to_all: true
        })
    );
    assert!(!window.shows(text("“compositor.rs” already exists")));
}

#[test]
fn closing_the_conflict_dialog_cancels_the_job() {
    let mut window = window();
    let mut answer = conflict(&mut window, JobId::next(), "keynote.pdf");
    window.app.press_key("Escape").settle();
    assert!(!window.shows(text("“keynote.pdf” already exists")));
    assert_eq!(answer.try_recv(), Err(oneshot::error::TryRecvError::Closed));
}

#[test]
fn running_jobs_show_in_the_toolbar_and_can_be_cancelled() {
    let mut window = window();
    assert!(
        !window
            .app
            .exists(role(Role::Button).name("File operations"))
    );
    window.click("keynote.pdf").key("Delete");
    window.job_commands();
    let id = JobId::next();
    events(
        &mut window,
        vec![
            JobUpdate::Submitted(id),
            JobUpdate::Event(JobEvent::Started {
                id,
                kind: JobKind::Trash,
                total_bytes: 0,
                total_items: 4,
            }),
            JobUpdate::Event(JobEvent::Progress {
                id,
                done_bytes: 0,
                done_items: 1,
                current: PathBuf::from("/x"),
            }),
        ],
    );
    window
        .app
        .click(role(Role::Button).name("File operations"))
        .settle();
    assert!(window.shows(text("Moving 1 item to the Trash")));
    window.app.click(role(Role::Button).name("Cancel")).settle();
    assert!(matches!(
        window.job_commands().as_slice(),
        [JobCommand::Cancel(cancelled)] if *cancelled == id
    ));
    events(
        &mut window,
        vec![JobUpdate::Event(JobEvent::Finished {
            id,
            outcome: Outcome::Cancelled,
            undo: None,
        })],
    );
    assert!(
        !window
            .app
            .exists(role(Role::Button).name("File operations"))
    );
}

#[test]
fn a_refused_copy_offers_to_run_again_as_root() {
    let mut window = window();
    let photo = window.dir("Downloads").join("photo.jpg");
    window.answer(|cx, store| {
        crate::app::file_jobs::submit(
            cx,
            store,
            Job::copy_into([photo.clone()], std::path::Path::new("/opt")),
            crate::app::file_jobs::Purpose::Plain,
        );
    });
    window.job_commands();
    let id = JobId::next();
    events(
        &mut window,
        vec![
            JobUpdate::Submitted(id),
            JobUpdate::Event(JobEvent::Failed {
                id,
                error: JobError::Io {
                    path: "/opt/photo.jpg".into(),
                    source: io::Error::from_raw_os_error(13),
                },
                needs_privilege: true,
            }),
            JobUpdate::Event(JobEvent::Finished {
                id,
                outcome: Outcome::Failed,
                undo: None,
            }),
        ],
    );
    assert!(window.shows(text("You don't have permission for this")));
    window
        .app
        .click(role(Role::Button).name("Authenticate"))
        .settle();
    assert!(matches!(
        window.job_commands().as_slice(),
        [JobCommand::Elevate(PrivilegedRequest::Copy { sources, destination, .. })]
            if *sources == [photo.clone()] && destination == std::path::Path::new("/opt")
    ));
    assert!(!window.shows(text("You don't have permission for this")));
}
