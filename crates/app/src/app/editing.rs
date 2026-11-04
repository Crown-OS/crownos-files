//! The two inline text edits: renaming an entry (F2) and typing a path (Ctrl+L).

use std::path::PathBuf;

use crownui::prelude::{Cx, Runtime};

use crate::{
    app::{listing::AfterChange, navigation::open_dir, tasks::Task},
    state::Store,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Editing {
    Nothing,
    Rename(usize),
    Path,
}

pub fn start_rename(runtime: &mut Runtime, store: Store, position: usize) {
    let Some(name) = store.folder.with(runtime, |folder| {
        folder.get(position).map(|entry| entry.name.to_string())
    }) else {
        return;
    };
    store.edit_text.set(runtime, name);
    store.editing.set(runtime, Editing::Rename(position));
}

pub fn start_path_edit(runtime: &mut Runtime, store: Store) {
    let path = store.cwd.with(runtime, |dir| dir.display().to_string());
    store.edit_text.set(runtime, path);
    store.editing.set(runtime, Editing::Path);
}

/// Applies the edit in progress: renames the entry or opens the typed folder.
pub fn commit(cx: &mut Cx, store: Store) {
    let editing = store.editing.get(cx);
    if editing == Editing::Nothing {
        return;
    }
    let typed = store.edit_text.get(cx);
    store.editing.set(cx, Editing::Nothing);
    match editing {
        Editing::Rename(position) => rename(cx, store, position, typed),
        Editing::Path => open_typed_path(cx, store, &typed),
        Editing::Nothing => {}
    }
    store.file_area.focus(cx);
}

pub fn cancel(cx: &mut Cx, store: Store) {
    if store.editing.get(cx) != Editing::Nothing {
        store.editing.set(cx, Editing::Nothing);
    }
    store.file_area.focus(cx);
}

fn rename(runtime: &mut Runtime, store: Store, position: usize, typed: String) {
    let Some(path) = store.folder.with(runtime, |folder| {
        folder
            .get(position)
            .filter(|entry| !typed.is_empty() && entry.name != typed)
            .map(|entry| entry.path_in(folder.dir()))
    }) else {
        return;
    };
    store.after_change.update(runtime, |slot| {
        *slot = Some(AfterChange::Select(typed.as_str().into()));
    });
    store.run(
        runtime,
        Task::Rename {
            path,
            name: typed.into(),
        },
    );
}

fn open_typed_path(cx: &mut Cx, store: Store, typed: &str) {
    let home = store.places.with(cx, |places| places.home.clone());
    let path = match (typed.strip_prefix('~'), home) {
        (Some(rest), Some(home)) => home.join(rest.trim_start_matches('/')),
        _ => PathBuf::from(typed),
    };
    if path.is_dir() {
        open_dir(cx, store, path.into());
    } else {
        store
            .toasts
            .show(cx, format!("There is no folder at {typed}"));
    }
}
