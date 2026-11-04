//! The `files` config section: read into the store at start, followed while
//! the window is open, and written back when the user changes a view option.

use crownconfig::{Files, SectionWatch, SortKey as ConfigSortKey, save_files, watch_files};
use crownui::prelude::Cx;

use crate::{
    app::listing::reorder,
    model::sort::{SortKey, SortOrder},
    state::Store,
};

pub fn sort_order(files: &Files) -> SortOrder {
    SortOrder {
        key: match files.sort_by {
            ConfigSortKey::Name => SortKey::Name,
            ConfigSortKey::Modified => SortKey::Modified,
            ConfigSortKey::Created => SortKey::Created,
            ConfigSortKey::Size => SortKey::Size,
            ConfigSortKey::Kind => SortKey::Kind,
        },
        reverse: files.sort_reverse,
        dirs_first: files.dirs_first,
    }
}

/// Follows edits of the section made elsewhere and saves the view options
/// the user changes here, keeping every other key as it was.
pub fn sync_config(cx: &mut Cx, store: Store, files: Files) {
    let known = cx.store(files.clone());
    cx.effect(move |runtime| {
        let view_mode = store.view_mode.get(runtime);
        let show_hidden = store.show_hidden.get(runtime);
        let edited = known.update(runtime, |known| {
            let changed = known.view_mode != view_mode || known.show_hidden != show_hidden;
            known.view_mode = view_mode;
            known.show_hidden = show_hidden;
            changed.then(|| known.clone())
        });
        if let Some(files) = edited
            && let Err(error) = save_files(&files)
        {
            eprintln!("crownos-files: saving the files section failed: {error}");
        }
    });
    follow(cx, store, known, watch_files(files));
}

fn follow(
    cx: &mut Cx,
    store: Store,
    known: crownui::prelude::Store<Files>,
    mut watch: SectionWatch<Files>,
) {
    let next_edit = async move {
        let files = watch.changed().await;
        (watch, files)
    };
    cx.spawn(next_edit, move |cx, (watch, files)| {
        known.update(cx, |known| known.clone_from(&files));
        apply(cx, store, &files);
        follow(cx, store, known, watch);
    });
}

fn apply(cx: &mut Cx, store: Store, files: &Files) {
    if store.view_mode.get(cx) != files.view_mode {
        store.view_mode.set(cx, files.view_mode);
    }
    let sort = sort_order(files);
    let reordered = store.show_hidden.get(cx) != files.show_hidden || store.sort.get(cx) != sort;
    store.show_hidden.set(cx, files.show_hidden);
    store.sort.set(cx, sort);
    if reordered {
        reorder(cx, store);
    }
}
