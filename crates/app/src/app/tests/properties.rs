//! The Properties dialog: what the listing knows at once, then the stat, a
//! folder's measured size and a volume's capacity as the tasks answer.

use std::path::PathBuf;

use crownos_files_devices::storage::SpaceUsage;
use crowntest::{Role, role, text};

use super::window::{Window, drain, window};
use crate::{
    app::{
        fixtures::LAUNCH,
        properties::{FileFacts, show_properties},
        tasks::{Task, TaskDone, finish_tasks},
    },
    model::timestamp::Timestamp,
};

const GB: u64 = 1_000_000_000;

fn open_from_menu(window: &mut Window, entry: &str) {
    window.click(entry).key("Shift+F10");
    window
        .app
        .click(role(Role::MenuItem).name("Properties"))
        .settle();
}

fn facts(mode: u32, volume: Option<SpaceUsage>) -> FileFacts {
    FileFacts {
        created: Timestamp::UNKNOWN,
        modified: Timestamp::UNKNOWN,
        accessed: Timestamp::UNKNOWN,
        mode,
        owner: "alice".to_owned(),
        group: "users".to_owned(),
        len: 18_400_000,
        volume,
    }
}

fn answer(window: &mut Window, done: TaskDone) {
    window.answer(|cx, store| finish_tasks(cx, store, vec![done]));
}

fn inspected(window: &mut Window) -> Vec<PathBuf> {
    drain(&mut window.workers.tasks)
        .into_iter()
        .filter_map(|task| match task {
            Task::Inspect(path) => Some(path),
            _ => None,
        })
        .collect()
}

#[test]
fn a_file_shows_its_type_size_place_owner_and_permissions() {
    let mut window = window();
    drain(&mut window.workers.tasks);
    open_from_menu(&mut window, "keynote.pdf");
    let keynote = window.dir(LAUNCH).join("keynote.pdf");
    assert_eq!(inspected(&mut window), std::slice::from_ref(&keynote));
    assert!(window.shows(role(Role::Dialog)));
    assert!(window.shows(text("application/pdf")));
    assert!(window.shows(text("18 MB")));
    assert!(window.shows(text("Documents › Launch")));

    answer(
        &mut window,
        TaskDone::Inspected(keynote, facts(0o100640, None)),
    );
    assert!(window.shows(text("alice · rw-")));
    assert!(window.shows(text("users · r--")));
    assert!(window.shows(text("---")));
    assert!(!window.shows(text("Capacity")));
    window.app.click(role(Role::Button).name("Close")).settle();
    assert!(!window.shows(role(Role::Dialog)));
}

#[test]
fn a_folder_is_measured_until_the_dialog_closes() {
    let mut window = window();
    drain(&mut window.workers.tasks);
    open_from_menu(&mut window, "Renders");
    let renders = window.dir(LAUNCH).join("Renders");
    let tasks = drain(&mut window.workers.tasks);
    let cancel = tasks.iter().find_map(|task| match task {
        Task::MeasureFolder { path, cancel } if *path == renders => Some(cancel.clone()),
        _ => None,
    });
    let cancel = cancel.unwrap();
    assert!(
        tasks
            .iter()
            .any(|task| matches!(task, Task::Inspect(path) if *path == renders))
    );
    assert!(window.shows(text("inode/directory")));
    assert!(window.shows(text("Calculating…")));
    answer(
        &mut window,
        TaskDone::FolderMeasured(renders, 4_800_000_000),
    );
    assert!(window.shows(text("4.8 GB")));
    assert!(!cancel.is_cancelled());
    window.app.click(role(Role::Button).name("Close")).settle();
    assert!(cancel.is_cancelled(), "closing stops the walk");
}

#[test]
fn a_mounted_volume_shows_its_capacity() {
    let mut window = window();
    drain(&mut window.workers.tasks);
    window.answer(show_properties);
    let launch = window.dir(LAUNCH).to_path_buf();
    assert!(inspected(&mut window).contains(&launch));
    assert!(window.shows(text("Launch")));
    let volume = SpaceUsage {
        total: 1_000 * GB,
        free: 412 * GB,
        available: 400 * GB,
    };
    answer(
        &mut window,
        TaskDone::Inspected(launch, facts(0o040755, Some(volume))),
    );
    assert!(window.shows(text("Capacity")));
    assert!(window.shows(text("1 TB")));
    assert!(window.shows(text("400 GB")));
}
