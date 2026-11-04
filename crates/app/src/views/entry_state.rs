//! The per-entry reactive reads tiles and rows bind their look to.

use crownui::prelude::Runtime;

use crate::{app::editing::Editing, state::Store};

/// Whether the entry at `position` is selected, re-read when the selection moves.
pub fn is_selected(runtime: &mut Runtime, store: Store, position: usize) -> bool {
    store.selection_rev.get(runtime);
    store
        .folder
        .with(runtime, |folder| folder.is_selected(position))
}

pub fn is_renaming(runtime: &mut Runtime, store: Store, position: usize) -> bool {
    store.editing.get(runtime) == Editing::Rename(position)
}

/// The accessible value of a selected tile or row.
pub fn selection_value(runtime: &mut Runtime, store: Store, position: usize) -> String {
    if is_selected(runtime, store, position) {
        "selected".to_owned()
    } else {
        String::new()
    }
}
