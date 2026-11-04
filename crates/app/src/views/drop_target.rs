//! Making a view a place to drop files on: a folder (which springs open when
//! the drag rests on it) or the Trash.

use std::{path::Path, sync::Arc};

use crownui::prelude::{Listeners, Signal};

use crate::{
    app::drag_drop::{
        drag_over_folder, drag_over_trash, drop_on_folder, drop_on_trash, leave_folder,
    },
    state::Store,
};

/// `view` takes drops into `dir`; `hovered` is true while it would accept one.
pub fn folder_drop_target<V: Listeners>(
    view: V,
    store: Store,
    dir: Arc<Path>,
    hovered: Signal<bool>,
) -> V {
    let (left, dropped) = (dir.clone(), dir.clone());
    view.on_drag_over(move |cx, event| {
        let accepted = drag_over_folder(cx, store, &dir, true, event);
        if hovered.get(cx) != accepted {
            hovered.set(cx, accepted);
        }
    })
    .on_drag_leave(move |cx, _| {
        hovered.set(cx, false);
        leave_folder(cx, store, &left);
    })
    .on_drop(move |cx, event| {
        hovered.set(cx, false);
        drop_on_folder(cx, store, &dropped, event);
    })
}

/// `view` moves dropped files to the trash.
pub fn trash_drop_target<V: Listeners>(view: V, store: Store) -> V {
    view.on_drag_over(|_, event| drag_over_trash(event))
        .on_drop(move |cx, event| drop_on_trash(cx, store, event))
}
