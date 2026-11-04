//! Folders pinned to the sidebar: dropped on its pin zone, listed under
//! Pinned and saved in the `files` config section.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crownconfig::Files;
use crownui::{
    ext::{DragOverEvent, DropEvent},
    prelude::{Cx, DndAction, Event, Runtime},
};

use crate::{
    state::Store,
    util::{clipboard_items::read_files, clipboard_payload::URI_LIST_MIME},
};

pub fn pinned_in(files: &Files) -> Vec<Arc<Path>> {
    files
        .pinned
        .iter()
        .map(|dir| Arc::from(dir.as_path()))
        .collect()
}

pub fn same_pins(saved: &[PathBuf], pinned: &[Arc<Path>]) -> bool {
    saved
        .iter()
        .map(PathBuf::as_path)
        .eq(pinned.iter().map(AsRef::as_ref))
}

pub fn unpin(runtime: &mut Runtime, store: Store, dir: &Path) {
    store
        .pinned
        .update(runtime, |pinned| pinned.retain(|pin| **pin != *dir));
}

fn pin(runtime: &mut Runtime, store: Store, dirs: Vec<PathBuf>) {
    let fresh: Vec<Arc<Path>> = store.pinned.with(runtime, |pinned| {
        dirs.into_iter()
            .filter(|dir| dir.is_dir() && !pinned.iter().any(|pin| **pin == **dir))
            .map(Arc::from)
            .collect()
    });
    if !fresh.is_empty() {
        store.pinned.update(runtime, |pinned| pinned.extend(fresh));
    }
}

/// Opens or folds the sidebar's pin zone.
pub fn show_pin_zone(runtime: &mut Runtime, store: Store, shown: bool) {
    if store.drag.pin_zone.get(runtime) != shown {
        store.drag.pin_zone.set(runtime, shown);
    }
}

/// A drag from another app reached the window: it may carry folders.
pub fn drag_entered_window(runtime: &mut Runtime, store: Store, event: &Event<DragOverEvent>) {
    if event.offers(URI_LIST_MIME) && !store.drag.is_internal(runtime) {
        show_pin_zone(runtime, store, true);
    }
}

pub fn drag_left_window(runtime: &mut Runtime, store: Store) {
    if !store.drag.is_internal(runtime) {
        show_pin_zone(runtime, store, false);
    }
}

/// Pinning only links to the folders, so it takes them as a copy: the source
/// never deletes anything.
pub fn drag_over_pin_zone(cx: &mut Cx, store: Store, event: &mut Event<DragOverEvent>) -> bool {
    event.stop_propagation();
    let accepted = store.drag.pin_zone.get(cx)
        && event.offers(URI_LIST_MIME)
        && event.source_actions.contains(DndAction::Copy);
    if accepted {
        event.accept_with(URI_LIST_MIME, DndAction::Copy);
    }
    accepted
}

pub fn drop_on_pin_zone(cx: &mut Cx, store: Store, event: &mut Event<DropEvent>) {
    event.stop_propagation();
    show_pin_zone(cx, store, false);
    if let Some((_, paths)) = read_files(&event.mime_type, &event.data) {
        pin(cx, store, paths);
    }
}
