//! The path of the open folder under the file list, or the path typed after Ctrl+L.

use std::{path::Path, sync::Arc};

use crownui::{
    kit::{FieldShape, PathBarOptions, PathSegment, TextInputOptions, icons, path_bar, text_input},
    prelude::{Cx, Listeners, Runtime, Semantic, Styled, View, bind, show},
};

use crate::{
    app::{
        drag_drop::{drag_over_folder, drop_on_folder},
        editing::{Editing, cancel, commit},
        navigation::open_dir,
        virtual_folder::VirtualFolder,
    },
    state::Store,
    util::breadcrumbs::breadcrumbs,
    views::focus_slot::{FocusSlotOptions, autofocused},
};

fn segments(runtime: &mut Runtime, store: Store) -> Vec<PathSegment> {
    let home = store.places.with(runtime, |places| places.home.clone());
    store.cwd.with(runtime, |dir| {
        if let Some(folder) = VirtualFolder::of(dir) {
            return vec![PathSegment::new(folder.label()).icon(folder.icon())];
        }
        breadcrumbs(dir, home.as_deref())
            .into_iter()
            .map(|crumb| {
                let segment = PathSegment::new(crumb.label);
                match (crumb.is_home, crumb.label.is_empty()) {
                    (true, _) => segment.icon(icons::HOUSE),
                    (false, true) => segment.icon(icons::HARD_DRIVE),
                    (false, false) => segment,
                }
            })
            .collect()
    })
}

pub fn location_bar(cx: &mut Cx, store: Store) -> impl View + use<> {
    let initial = segments(cx, store);
    let shown = cx.signal(initial);
    cx.effect(move |runtime| {
        let next = segments(runtime, store);
        if shown.with_untracked(runtime, |current| *current != next) {
            shown.set(runtime, next);
        }
    });
    let crumbs = move || {
        path_bar(shown, move |cx, index| {
            if let Some(dir) = crumb_path(cx, store, index) {
                open_dir(cx, store, dir);
            }
        })
        .on_segment_drag_over(move |cx, index, event| {
            if let Some(dir) = crumb_path(cx, store, index) {
                drag_over_folder(cx, store, &dir, false, event);
            }
        })
        .on_segment_drop(move |cx, index, event| {
            if let Some(dir) = crumb_path(cx, store, index) {
                drop_on_folder(cx, store, &dir, event);
            }
        })
    };
    let editor = move || {
        autofocused(
            text_input(store.edit_text)
                .shape(FieldShape::Pill)
                .stretch()
                .label("Location")
                .on_submit(move |cx, _| commit(cx, store)),
        )
        .on_focus_out(move |cx| cancel(cx, store))
        .flex_1()
    };
    show(
        bind(move |runtime: &mut Runtime| store.editing.get(runtime) == Editing::Path),
        editor,
        crumbs,
    )
    .flex_row()
    .items_center()
    .flex_1()
    .min_w(0.0)
}

/// The folder the path bar's segment `index` stands for.
fn crumb_path(runtime: &mut Runtime, store: Store, index: usize) -> Option<Arc<Path>> {
    let home = store.places.with(runtime, |places| places.home.clone());
    store.cwd.with(runtime, |dir| {
        if VirtualFolder::of(dir).is_some() {
            return None;
        }
        breadcrumbs(dir, home.as_deref())
            .get(index)
            .map(|crumb| crumb.path.into())
    })
}
