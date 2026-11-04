//! What the menus, buttons and shortcuts do with the selection.

use crownconfig::ViewMode;
use crownui::prelude::{Cx, Runtime};

use crate::{
    app::{
        editing::start_rename, navigation::open_dir, opening::open_selection, tasks::Task,
        virtual_folder::VirtualFolder,
    },
    state::Store,
};

/// Actions whose backends arrive with the next milestones; each says so when used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deferred {
    Compress,
    Tags,
    SendToDevice,
    Properties,
}

impl Deferred {
    fn label(self) -> &'static str {
        match self {
            Self::Compress => "Compress",
            Self::Tags => "Tags",
            Self::SendToDevice => "Send to device",
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

/// F2: renames the entry under the cursor; what is in the trash keeps its name.
pub fn rename_cursor(runtime: &mut Runtime, store: Store) {
    let position = store.folder.with(runtime, |folder| {
        let cursor = folder.cursor();
        let renamable = VirtualFolder::of(folder.dir()) != Some(VirtualFolder::Trash);
        (renamable && folder.is_selected(cursor)).then_some(cursor)
    });
    if let Some(position) = position {
        start_rename(runtime, store, position);
    }
}

pub fn new_folder(cx: &mut Cx, store: Store) {
    let dir = store.dir(cx);
    if let Some(folder) = VirtualFolder::of(&dir) {
        store.toasts.show(
            cx,
            format!("Folders can't be created in {}", folder.label()),
        );
        return;
    }
    store.run(
        cx,
        Task::CreateFolder {
            dir: dir.to_path_buf(),
        },
    );
}

pub fn toggle_hidden(runtime: &mut Runtime, store: Store) {
    store.show_hidden.update(runtime, |shown| *shown = !*shown);
}

pub fn set_view_mode(runtime: &mut Runtime, store: Store, mode: ViewMode) {
    if store.view_mode.get(runtime) != mode {
        store.view_mode.set(runtime, mode);
    }
}
