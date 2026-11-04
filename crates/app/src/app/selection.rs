//! Changing which entries are selected and where the keyboard cursor is.

use crownui::{ext::Modifiers, prelude::Runtime};

use crate::{model::folder::Folder, state::Store};

fn change(runtime: &mut Runtime, store: Store, edit: impl FnOnce(&mut Folder)) {
    store.folder.update(runtime, edit);
    store
        .selection_rev
        .update(runtime, |revision| *revision += 1);
}

/// A click on the entry at `position`: Ctrl toggles it, Shift extends from the anchor.
pub fn click(runtime: &mut Runtime, store: Store, position: usize, modifiers: Modifiers) {
    change(runtime, store, |folder| {
        if modifiers.shift_key {
            folder.select_range_to(position);
        } else if modifiers.ctrl_key || modifiers.meta_key {
            folder.toggle_selected(position);
        } else {
            folder.select_only(position);
        }
        folder.set_cursor(position);
    });
}

/// Selects the entry at `position` unless it already is, as a right click does.
pub fn ensure_selected(runtime: &mut Runtime, store: Store, position: usize) {
    if !store
        .folder
        .with(runtime, |folder| folder.is_selected(position))
    {
        click(runtime, store, position, Modifiers::NONE);
    }
}

/// Moves the cursor to `position`, selecting just it or, with `extend`, the range to it.
pub fn move_cursor(runtime: &mut Runtime, store: Store, position: usize, extend: bool) {
    change(runtime, store, |folder| {
        if extend {
            folder.select_range_to(position);
        } else {
            folder.select_only(position);
        }
        folder.set_cursor(position);
    });
    store.scroll.ensure_visible(runtime, position);
}

pub fn select_all(runtime: &mut Runtime, store: Store) {
    change(runtime, store, Folder::select_all);
}

pub fn clear(runtime: &mut Runtime, store: Store) {
    if store
        .folder
        .with(runtime, |folder| !folder.selection().is_empty())
    {
        change(runtime, store, Folder::clear_selection);
    }
}

/// Selects exactly what a rubber band covers.
pub fn select_positions(runtime: &mut Runtime, store: Store, positions: Vec<usize>) {
    change(runtime, store, |folder| folder.select_positions(positions));
}
