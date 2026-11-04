//! The Trash and Recents: virtual folders listed by a task, and the trash's
//! Restore, Delete and Empty.

use std::{fs, path::Path, sync::Arc};

use crowntest::{Role, role, text};

use super::window::{Window, drain, window};
use crate::{
    app::{
        file_jobs::JobCommand,
        tasks::Task,
        virtual_folder::{VirtualFolder, VirtualListing, apply_listing, entry_at},
    },
    fs::engine::EngineCmd,
    jobs::Job,
    model::trash::{TrashInfo, TrashedItem},
};

fn open_place(window: &mut Window, place: &str) -> Vec<Task> {
    window.engine_commands();
    window.app.click(role(Role::MenuItem).name(place)).settle();
    drain(&mut window.workers.tasks)
}

/// Two trashed files under the home's trash can, as `list_trash` reports them.
fn trashed(window: &Window) -> Vec<TrashedItem> {
    let can: Arc<Path> = window.home.path().join("Trash").into();
    ["draft one.md", "old-logo.svg"]
        .into_iter()
        .map(|name| {
            fs::write(can.join("files").join(name), name).unwrap();
            TrashedItem {
                trash: can.clone(),
                name: name.into(),
                info: TrashInfo {
                    original: window.home.path().join("Documents").join(name),
                    deleted_at: None,
                },
                is_dir: false,
                size: Some(name.len() as u64),
            }
        })
        .collect()
}

fn show_trash(window: &mut Window) -> Vec<TrashedItem> {
    let tasks = open_place(window, "Trash");
    assert!(
        tasks
            .iter()
            .any(|task| matches!(task, Task::ListVirtual(VirtualFolder::Trash))),
        "{tasks:?}"
    );
    assert!(
        window
            .engine_commands()
            .iter()
            .all(|command| matches!(command, EngineCmd::Watch(dirs) if dirs.is_empty())),
        "the engine neither lists nor watches the trash"
    );
    let items = trashed(window);
    let listing = VirtualListing {
        folder: VirtualFolder::Trash,
        entries: items
            .iter()
            .filter_map(|item| entry_at(&item.file_path(), item.info.original.file_name()))
            .collect(),
        trashed: items.clone(),
    };
    window.answer(|cx, store| apply_listing(cx, store, listing));
    window.app.settle();
    items
}

#[test]
fn the_trash_lists_what_list_trash_found_under_the_original_names() {
    let mut window = window();
    show_trash(&mut window);
    assert!(window.app.exists(window.entry("draft one.md")));
    assert!(window.app.exists(window.entry("old-logo.svg")));
    assert_eq!(window.status(), "2 items");
    window.click("draft one.md");
    assert!(window.shows(text("Documents")), "where it was trashed from");
}

#[test]
fn restore_and_delete_act_on_the_selected_trashed_items() {
    let mut window = window();
    let items = show_trash(&mut window);
    window.click("draft one.md");
    window
        .app
        .click(role(Role::Button).name("Restore"))
        .settle();
    assert!(matches!(
        window.job_commands().as_slice(),
        [JobCommand::Submit { job: Job::Restore(restored), .. }] if *restored == [items[0].clone()]
    ));
    window.click("old-logo.svg").key("Delete");
    assert!(window.shows(text("Delete this item permanently?")));
    window.app.click(role(Role::Button).name("Delete")).settle();
    assert!(matches!(
        window.job_commands().as_slice(),
        [JobCommand::Submit { job: Job::DeleteFromTrash(deleted), .. }] if *deleted == [items[1].clone()]
    ));
}

#[test]
fn empty_trash_asks_first() {
    let mut window = window();
    show_trash(&mut window);
    window
        .app
        .click(role(Role::Button).name("Empty Trash"))
        .settle();
    assert!(window.shows(text("Empty the Trash?")));
    assert!(window.job_commands().is_empty());
    window.app.click(role(Role::Button).name("Empty")).settle();
    assert!(matches!(
        window.job_commands().as_slice(),
        [JobCommand::Submit {
            job: Job::EmptyTrash,
            ..
        }]
    ));
}

#[test]
fn recents_is_listed_by_a_task_too() {
    let mut window = window();
    let tasks = open_place(&mut window, "Recents");
    assert!(
        tasks
            .iter()
            .any(|task| matches!(task, Task::ListVirtual(VirtualFolder::Recents)))
    );
    assert!(window.app.exists(role(Role::Button).name("Recents")));
}
