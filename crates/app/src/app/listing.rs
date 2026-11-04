//! Keeping the current [`Folder`] in step with the engine and the view
//! options, and telling the views when to redraw.

use compact_str::CompactString;
use crownui::prelude::Runtime;

use crate::{
    app::editing::start_rename,
    model::{files_op::FilesOp, filter::Filter, view_options::ViewOptions},
    state::Store,
};

/// What to do with an entry the moment it is listed: one just renamed or created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AfterChange {
    Select(CompactString),
    Rename(CompactString),
}

/// Applies a burst of engine changes to the current folder and the cached ones.
pub fn apply_ops(runtime: &mut Runtime, store: Store, ops: Vec<FilesOp>) {
    let current = store.dir(runtime);
    let mut changed = false;
    for op in FilesOp::merge(ops) {
        if *op.dir() == current {
            changed |= store.folder.update(runtime, |folder| {
                let before = folder.revision();
                folder.apply(op);
                folder.revision() != before
            });
        } else {
            store.history.update(runtime, |history| {
                if let Some(folder) = history.get_mut(op.dir()) {
                    folder.apply(op);
                }
            });
        }
    }
    if changed {
        publish(runtime, store);
        resolve_after_change(runtime, store);
    }
}

/// The view options the toolbar and config ask for, with the search box's filter.
pub fn view_options(runtime: &mut Runtime, store: Store) -> ViewOptions {
    ViewOptions {
        sort: store.sort.get(runtime),
        show_hidden: store.show_hidden.get(runtime),
        filter: store.query.with(runtime, |query| Filter::new(query)),
    }
}

/// Re-sorts and re-filters the current folder after a view option changed.
pub fn reorder(runtime: &mut Runtime, store: Store) {
    let view = view_options(runtime, store);
    store.folder.update(runtime, |folder| {
        folder.set_sort(view.sort);
        folder.set_show_hidden(view.show_hidden);
        folder.set_filter(view.filter);
        folder.refresh_order();
    });
    publish(runtime, store);
}

/// Tells every view the listing changed.
pub fn publish(runtime: &mut Runtime, store: Store) {
    let count = store.folder.with(runtime, |folder| folder.len());
    if store.count.with_untracked(runtime, |known| *known != count) {
        store.count.set(runtime, count);
    }
    store.revision.update(runtime, |revision| *revision += 1);
    store
        .selection_rev
        .update(runtime, |revision| *revision += 1);
}

/// Selects (or starts renaming) the entry an earlier action is waiting for, once it is listed.
pub fn resolve_after_change(runtime: &mut Runtime, store: Store) {
    let Some(pending) = store.after_change.with(runtime, Clone::clone) else {
        return;
    };
    let (AfterChange::Select(name) | AfterChange::Rename(name)) = &pending;
    let Some(position) = store
        .folder
        .with(runtime, |folder| folder.position_of(name))
    else {
        return;
    };
    store.after_change.update(runtime, |slot| *slot = None);
    store.folder.update(runtime, |folder| {
        folder.select_only(position);
        folder.set_cursor(position);
    });
    store
        .selection_rev
        .update(runtime, |revision| *revision += 1);
    store.scroll.ensure_visible(runtime, position);
    if matches!(pending, AfterChange::Rename(_)) {
        start_rename(runtime, store, position);
    }
}
