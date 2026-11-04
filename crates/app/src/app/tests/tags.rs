//! Tags: read for the page in view, shown as chips, toggled from the
//! context menu's Tags page, and matched by `tag:` in the search box.

use std::{ffi::OsString, path::Path, sync::Arc};

use compact_str::CompactString;
use crownos_files_devices::tags::Tags;
use crowntest::{Role, role, text};

use super::window::{LISTED, Window, drain, window};
use crate::app::{
    fixtures::LAUNCH,
    tagging::TagChange,
    tasks::{Task, TaskDone, finish_tasks},
};

/// The tag reads the window asked for: `(generation, names)`.
fn reads(window: &mut Window) -> Vec<(u64, Vec<CompactString>)> {
    drain(&mut window.workers.tasks)
        .into_iter()
        .filter_map(|task| match task {
            Task::ReadTags {
                generation,
                entries,
                ..
            } => Some((
                generation,
                entries
                    .into_iter()
                    .map(|(name, _): (CompactString, OsString)| name)
                    .collect(),
            )),
            _ => None,
        })
        .collect()
}

fn tagged(names: &[(&str, &[&str])]) -> Vec<(CompactString, Tags)> {
    names
        .iter()
        .map(|(name, tags)| ((*name).into(), tags.iter().map(|&tag| tag.into()).collect()))
        .collect()
}

fn answer_read(window: &mut Window, generation: u64, tags: Vec<(CompactString, Tags)>) {
    let dir: Arc<Path> = window.dir(LAUNCH);
    window.answer(|cx, store| {
        finish_tasks(
            cx,
            store,
            vec![TaskDone::TagsRead {
                dir,
                generation,
                tags,
            }],
        );
    });
}

fn listed(window: &mut Window) -> Vec<&'static str> {
    LISTED
        .into_iter()
        .filter(|entry| {
            let query = window.entry(entry);
            window.app.exists(query)
        })
        .collect()
}

#[test]
fn the_page_in_view_is_read_once_and_its_tags_become_chips() {
    let mut window = window();
    let asked = reads(&mut window);
    let (generation, names) = asked.last().cloned().unwrap();
    assert!(names.iter().any(|name| name == "keynote.pdf"));
    assert!(
        !names.iter().any(|name| name == ".git"),
        "hidden entries are not in view"
    );
    answer_read(
        &mut window,
        generation,
        tagged(&[("keynote.pdf", &["Final", "Draft"])]),
    );
    window.click("keynote.pdf");
    assert!(
        reads(&mut window).is_empty(),
        "known tags are not read again"
    );
    assert!(window.shows(text("Final")));
    assert!(window.shows(text("Draft")));
    window.click("palette.svg");
    assert!(!window.shows(text("Draft")));

    answer_read(
        &mut window,
        generation + 7,
        tagged(&[("palette.svg", &["Draft"])]),
    );
    assert!(
        !window.shows(text("Draft")),
        "answers to another generation are dropped"
    );
}

#[test]
fn the_tags_page_adds_a_tag_to_the_selection_then_removes_it() {
    let mut window = window();
    reads(&mut window);
    window.click("keynote.pdf").key("Shift+F10");
    window.app.click(role(Role::MenuItem).name("Tags")).settle();
    assert!(window.app.exists(role(Role::Menu)), "the menu stays open");
    assert!(window.shows(role(Role::MenuItem).name("Brand")));
    window
        .app
        .click(role(Role::MenuItem).name("Final"))
        .settle();
    let keynote = window.dir(LAUNCH).join("keynote.pdf");
    let tasks = drain(&mut window.workers.tasks);
    assert!(matches!(
        tasks.as_slice(),
        [Task::Tag { entries, tag, change: TagChange::Add, .. }]
            if tag == "Final" && entries.iter().map(|(_, path)| path).eq([&keynote])
    ));
    let dir = window.dir(LAUNCH);
    window.answer(|cx, store| {
        finish_tasks(
            cx,
            store,
            vec![TaskDone::Tagged {
                dir,
                tags: tagged(&[("keynote.pdf", &["Final"])]),
                failure: None,
            }],
        );
    });
    assert!(window.shows(text("Final")));

    window.key("Shift+F10");
    window.app.click(role(Role::MenuItem).name("Tags")).settle();
    window
        .app
        .click(role(Role::MenuItem).name("Final"))
        .settle();
    assert!(matches!(
        drain(&mut window.workers.tasks).as_slice(),
        [Task::Tag {
            change: TagChange::Remove,
            ..
        }]
    ));

    let dir = window.dir(LAUNCH);
    window.answer(|cx, store| {
        finish_tasks(
            cx,
            store,
            vec![TaskDone::Tagged {
                dir,
                tags: Vec::new(),
                failure: Some("keynote.pdf is on a filesystem without user attributes".to_owned()),
            }],
        );
    });
    assert!(window.shows(text("without user attributes")));
}

#[test]
fn tag_queries_read_the_whole_folder_and_keep_only_tagged_entries() {
    let mut window = window();
    reads(&mut window);
    window
        .app
        .focus(role(Role::SearchField))
        .type_text("tag:final")
        .settle();
    let (generation, names) = reads(&mut window).last().cloned().unwrap();
    assert_eq!(
        names,
        [".git"],
        "the rest of the folder, hidden entries too"
    );
    assert!(
        listed(&mut window).is_empty(),
        "nothing is known to match yet"
    );
    answer_read(
        &mut window,
        generation,
        tagged(&[
            ("keynote.pdf", &["Final"]),
            ("launch-plan.odt", &["final", "Brand"]),
            ("palette.svg", &["Brand"]),
        ]),
    );
    assert_eq!(listed(&mut window), ["keynote.pdf", "launch-plan.odt"]);
    assert_eq!(window.status(), "2 items");
    window.app.type_text(" plan").settle();
    assert_eq!(listed(&mut window), ["launch-plan.odt"]);
    window.app.type_text(" tag:brand").settle();
    assert_eq!(listed(&mut window), ["launch-plan.odt"]);
}
