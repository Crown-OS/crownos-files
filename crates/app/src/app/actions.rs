//! What the menus, buttons and shortcuts do with the selection.

use crownconfig::ViewMode;
use crownui::prelude::{Cx, Runtime};

use crate::{
    app::{editing::start_rename, navigation::open_dir, tasks::Task},
    state::Store,
};

/// Actions whose backends arrive with the next milestones; each says so when used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deferred {
    Recents,
    OpenFile,
    OpenWith,
    QuickLook,
    Copy,
    Cut,
    Paste,
    Duplicate,
    Compress,
    Tags,
    SendToDevice,
    Trash,
    DeletePermanently,
    Undo,
    Properties,
}

impl Deferred {
    fn label(self) -> &'static str {
        match self {
            Self::Recents => "Recents",
            Self::OpenFile => "Opening files",
            Self::OpenWith => "Open With",
            Self::QuickLook => "Quick Look",
            Self::Copy => "Copy",
            Self::Cut => "Cut",
            Self::Paste => "Paste",
            Self::Duplicate => "Duplicate",
            Self::Compress => "Compress",
            Self::Tags => "Tags",
            Self::SendToDevice => "Send to device",
            Self::Trash => "Move to Trash",
            Self::DeletePermanently => "Delete",
            Self::Undo => "Undo",
            Self::Properties => "Properties",
        }
    }
}

pub fn deferred(cx: &mut Cx, store: Store, action: Deferred) {
    store
        .toasts
        .show(cx, format!("{} is not available yet", action.label()));
}

/// Opens the entry at `position`: a folder navigates, a file goes to its opener.
pub fn open_position(cx: &mut Cx, store: Store, position: usize) {
    let target = store.folder.with(cx, |folder| {
        folder
            .get(position)
            .map(|entry| (entry.is_dir(), entry.path_in(folder.dir())))
    });
    match target {
        Some((true, path)) => open_dir(cx, store, path.into()),
        Some((false, _)) => open_selection(cx, store),
        None => {}
    }
}

/// Enter and the details panel's Open: one selected folder navigates, files open.
pub fn open_selection(cx: &mut Cx, store: Store) {
    let (folder_path, any_selected) = store.folder.with(cx, |folder| {
        let mut selected = folder.selected_entries();
        let first = selected.next();
        let only_dir = first
            .filter(|entry| entry.is_dir())
            .filter(|_| selected.next().is_none())
            .map(|entry| entry.path_in(folder.dir()));
        (only_dir, first.is_some())
    });
    match folder_path {
        Some(path) => open_dir(cx, store, path.into()),
        None if any_selected => deferred(cx, store, Deferred::OpenFile),
        None => {}
    }
}

pub fn rename_cursor(runtime: &mut Runtime, store: Store) {
    let position = store.folder.with(runtime, |folder| {
        let cursor = folder.cursor();
        folder.is_selected(cursor).then_some(cursor)
    });
    if let Some(position) = position {
        start_rename(runtime, store, position);
    }
}

pub fn new_folder(runtime: &mut Runtime, store: Store) {
    let dir = store.dir(runtime).to_path_buf();
    store.run(runtime, Task::CreateFolder { dir });
}

pub fn toggle_hidden(runtime: &mut Runtime, store: Store) {
    store.show_hidden.update(runtime, |shown| *shown = !*shown);
}

pub fn set_view_mode(runtime: &mut Runtime, store: Store, mode: ViewMode) {
    if store.view_mode.get(runtime) != mode {
        store.view_mode.set(runtime, mode);
    }
}
