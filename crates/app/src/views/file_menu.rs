//! The right-click menu: entry actions over a selection, folder actions
//! over the empty area.

use crownconfig::ViewMode;
use crownui::{
    kit::{MenuRowOptions, context_menu_at, icons, menu_item, menu_separator},
    prelude::{AnyView, Cx, Listeners, Runtime, Styled, View, bind, keyed, show, vstack},
};

use crate::{
    app::{
        actions::{
            Deferred, deferred, new_folder, open_selection, rename_cursor, set_view_mode,
            toggle_hidden,
        },
        selection::select_all,
    },
    state::Store,
};

/// Opens the menu where the pointer last went down.
pub fn open_menu(cx: &mut Cx, store: Store) {
    let at = store.pointer.with(cx, |pointer| *pointer);
    store.menu.set(cx, Some(at));
}

pub fn file_menu(store: Store) -> impl View {
    context_menu_at(store.menu, move || {
        show(
            bind(move |runtime: &mut Runtime| {
                store.selection_rev.get(runtime);
                store
                    .folder
                    .with(runtime, |folder| !folder.selection().is_empty())
            }),
            move || vstack(entry_items(store)).items_stretch(),
            move || vstack(folder_items(store)).items_stretch(),
        )
        .flex_col()
        .items_stretch()
    })
}

fn later(label: &str, icon: &'static str, store: Store, action: Deferred) -> AnyView {
    menu_item(label)
        .leading_icon(icon)
        .on_click(move |cx, _| deferred(cx, store, action))
        .boxed()
}

fn entry_items(store: Store) -> Vec<AnyView> {
    let has_receiver = move |runtime: &mut Runtime| {
        store.devices.with(runtime, |devices| {
            devices.iter().any(|device| device.can_receive_files())
        })
    };
    vec![
        menu_item("Open")
            .leading_icon(icons::EXTERNAL_LINK)
            .shortcut_hint("Enter")
            .on_click(move |cx, _| open_selection(cx, store))
            .boxed(),
        later("Open With…", icons::APP_WINDOW, store, Deferred::OpenWith),
        later("Quick Look", icons::EYE, store, Deferred::QuickLook),
        menu_separator().boxed(),
        later("Cut", icons::SCISSORS, store, Deferred::Cut),
        later("Copy", icons::COPY, store, Deferred::Copy),
        later("Paste", icons::CLIPBOARD_PASTE, store, Deferred::Paste),
        menu_separator().boxed(),
        menu_item("Rename…")
            .leading_icon(icons::PENCIL)
            .shortcut_hint("F2")
            .on_click(move |cx, _| rename_cursor(cx, store))
            .boxed(),
        later("Duplicate", icons::COPY_PLUS, store, Deferred::Duplicate),
        later("Compress", icons::ARCHIVE, store, Deferred::Compress),
        menu_separator().boxed(),
        later("Tags…", icons::TAG, store, Deferred::Tags),
        show(
            bind(has_receiver),
            move || {
                later(
                    "Send to Device",
                    icons::SMARTPHONE,
                    store,
                    Deferred::SendToDevice,
                )
            },
            || vstack(()).boxed(),
        )
        .flex_col()
        .boxed(),
        menu_separator().boxed(),
        later("Move to Trash", icons::TRASH_2, store, Deferred::Trash),
        later("Properties", icons::INFO, store, Deferred::Properties),
    ]
}

fn folder_items(store: Store) -> Vec<AnyView> {
    let hidden_label = move |runtime: &mut Runtime| {
        if store.show_hidden.get(runtime) {
            "Hide Hidden Files"
        } else {
            "Show Hidden Files"
        }
    };
    let view_label = move |runtime: &mut Runtime| match store.view_mode.get(runtime) {
        ViewMode::Grid => "View as List",
        ViewMode::List => "View as Icons",
    };
    vec![
        menu_item("New Folder")
            .leading_icon(icons::FOLDER_PLUS)
            .shortcut_hint("Shift+Ctrl+N")
            .on_click(move |cx, _| new_folder(cx, store))
            .boxed(),
        later("Paste", icons::CLIPBOARD_PASTE, store, Deferred::Paste),
        menu_item("Select All")
            .shortcut_hint("Ctrl+A")
            .on_click(move |cx, _| select_all(cx, store))
            .boxed(),
        menu_separator().boxed(),
        toggled_item(store, hidden_label, icons::EYE, |cx, store| {
            toggle_hidden(cx, store)
        }),
        toggled_item(store, view_label, icons::LAYOUT_GRID, |cx, store| {
            let next = match store.view_mode.get(cx) {
                ViewMode::Grid => ViewMode::List,
                ViewMode::List => ViewMode::Grid,
            };
            set_view_mode(cx, store, next);
        }),
    ]
}

/// A row whose label follows a setting, e.g. "Show Hidden Files" / "Hide Hidden Files".
fn toggled_item(
    store: Store,
    label: impl Fn(&mut Runtime) -> &'static str + Copy + 'static,
    icon: &'static str,
    run: fn(&mut Cx, Store),
) -> AnyView {
    keyed(label, move |_, label: &'static str| {
        menu_item(label)
            .leading_icon(icon)
            .on_click(move |cx, _| run(cx, store))
    })
    .flex_col()
    .boxed()
}
