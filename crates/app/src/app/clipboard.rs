//! Cut, Copy and Paste of files through the system clipboard, in the types
//! other file managers read and write.

use crownui::prelude::Cx;

use crate::{
    app::{
        file_jobs::{Purpose, TransferMode, submit, transfer},
        selection::selected_paths,
        virtual_folder::VirtualFolder,
    },
    jobs::Job,
    state::Store,
    util::{
        clipboard_items::{FILE_MIME_TYPES, file_items, read_files},
        clipboard_payload::ClipboardAction,
    },
};

pub fn copy_selection(cx: &mut Cx, store: Store) {
    offer(cx, store, ClipboardAction::Copy);
}

pub fn cut_selection(cx: &mut Cx, store: Store) {
    offer(cx, store, ClipboardAction::Cut);
}

fn offer(cx: &mut Cx, store: Store, action: ClipboardAction) {
    let paths = selected_paths(cx, store);
    if !paths.is_empty() {
        cx.set_clipboard(file_items(action, &paths));
    }
}

/// Pastes the files on the clipboard into the open folder: a cut moves them
/// and is cleared once the move succeeds, a copy copies them.
pub fn paste(cx: &mut Cx, store: Store) {
    let dir = store.dir(cx);
    if let Some(folder) = VirtualFolder::of(&dir) {
        store
            .toasts
            .show(cx, format!("Files can't be pasted into {}", folder.label()));
        return;
    }
    cx.read_clipboard(&FILE_MIME_TYPES, move |cx, data| {
        let Some((action, paths)) = data.and_then(|data| read_files(&data.mime_type, &data.data))
        else {
            store.toasts.show(cx, "There are no files to paste");
            return;
        };
        let (mode, purpose) = match action {
            ClipboardAction::Cut => (TransferMode::Move, Purpose::PasteCut),
            ClipboardAction::Copy => (TransferMode::Copy, Purpose::Plain),
        };
        transfer(cx, store, paths, &dir, mode, purpose);
    });
}

/// Copies the selection next to itself.
pub fn duplicate_selection(cx: &mut Cx, store: Store) {
    let paths = selected_paths(cx, store);
    submit(cx, store, Job::duplicate(paths), Purpose::Plain);
}
