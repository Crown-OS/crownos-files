//! The right-click menu: entry actions over a selection, folder actions
//! over the empty area, the trash's own actions, and an "Open With" page.

use crownconfig::ViewMode;
use crownui::{
    kit::{
        MenuRowOptions, context_menu_at, icons, menu_header, menu_item, menu_item_disabled,
        menu_separator, menu_submenu,
    },
    prelude::{AnyView, Cx, Listeners, Runtime, Styled, View, keyed, vstack},
};

use crate::{
    app::{
        actions::{Deferred, deferred, new_folder, rename_cursor, set_view_mode, toggle_hidden},
        clipboard::{copy_selection, cut_selection, duplicate_selection, paste},
        deleting::{delete_selection, empty_trash, restore_selection, trash_selection},
        opening::{MenuApp, choose_app, open_selection, open_with, request_menu_apps},
        quick_look::toggle_quick_look,
        selection::select_all,
        virtual_folder::VirtualFolder,
    },
    state::Store,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuPage {
    Main,
    OpenWith,
}

/// What the menu shows, rebuilt whenever it changes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MenuContent {
    has_selection: bool,
    page: MenuPage,
    in_trash: bool,
    apps: Option<Vec<MenuApp>>,
}

/// Opens the menu where the pointer last went down.
pub fn open_menu(cx: &mut Cx, store: Store) {
    let at = store.pointer.with(cx, |pointer| *pointer);
    store.menu_page.set(cx, MenuPage::Main);
    store.menu.set(cx, Some(at));
    request_menu_apps(cx, store);
}

pub fn file_menu(store: Store) -> impl View {
    context_menu_at(store.menu, move || {
        keyed(
            move |runtime: &mut Runtime| content(runtime, store),
            move |_, content: MenuContent| {
                let items = match (content.page, content.has_selection, content.in_trash) {
                    (MenuPage::OpenWith, _, _) => open_with_items(store, content.apps),
                    (MenuPage::Main, true, true) => trash_items(store),
                    (MenuPage::Main, true, false) => entry_items(store),
                    (MenuPage::Main, false, in_trash) => folder_items(store, in_trash),
                };
                vstack(items).items_stretch()
            },
        )
        .flex_col()
        .items_stretch()
    })
}

fn content(runtime: &mut Runtime, store: Store) -> MenuContent {
    store.selection_rev.get(runtime);
    let page = store.menu_page.get(runtime);
    MenuContent {
        has_selection: store
            .folder
            .with(runtime, |folder| !folder.selection().is_empty()),
        page,
        in_trash: store.cwd.with(runtime, |dir| {
            VirtualFolder::of(dir) == Some(VirtualFolder::Trash)
        }),
        apps: match page {
            MenuPage::OpenWith => store.opening.menu_apps.get(runtime),
            MenuPage::Main => None,
        },
    }
}

fn item(label: &str, icon: &'static str, store: Store, run: fn(&mut Cx, Store)) -> AnyView {
    menu_item(label)
        .leading_icon(icon)
        .on_click(move |cx, _| run(cx, store))
        .boxed()
}

fn later(label: &str, icon: &'static str, store: Store, action: Deferred) -> AnyView {
    menu_item(label)
        .leading_icon(icon)
        .on_click(move |cx, _| deferred(cx, store, action))
        .boxed()
}

fn open_items(store: Store) -> [AnyView; 2] {
    [
        menu_item("Open")
            .leading_icon(icons::EXTERNAL_LINK)
            .shortcut_hint("Enter")
            .on_click(move |cx, _| open_selection(cx, store))
            .boxed(),
        menu_item("Quick Look")
            .leading_icon(icons::EYE)
            .shortcut_hint("Space")
            .on_click(move |cx, _| toggle_quick_look(cx, store))
            .boxed(),
    ]
}

fn entry_items(store: Store) -> Vec<AnyView> {
    let [open, quick_look] = open_items(store);
    vec![
        open,
        menu_submenu("Open With")
            .leading_icon(icons::APP_WINDOW)
            .on_click(move |cx, event| {
                event.stop_propagation();
                store.menu_page.set(cx, MenuPage::OpenWith);
            })
            .boxed(),
        quick_look,
        menu_separator().boxed(),
        menu_item("Cut")
            .leading_icon(icons::SCISSORS)
            .shortcut_hint("Ctrl+X")
            .on_click(move |cx, _| cut_selection(cx, store))
            .boxed(),
        menu_item("Copy")
            .leading_icon(icons::COPY)
            .shortcut_hint("Ctrl+C")
            .on_click(move |cx, _| copy_selection(cx, store))
            .boxed(),
        menu_item("Paste")
            .leading_icon(icons::CLIPBOARD_PASTE)
            .shortcut_hint("Ctrl+V")
            .on_click(move |cx, _| paste(cx, store))
            .boxed(),
        menu_separator().boxed(),
        menu_item("Rename…")
            .leading_icon(icons::PENCIL)
            .shortcut_hint("F2")
            .on_click(move |cx, _| rename_cursor(cx, store))
            .boxed(),
        item("Duplicate", icons::COPY_PLUS, store, duplicate_selection),
        later("Compress", icons::ARCHIVE, store, Deferred::Compress),
        menu_separator().boxed(),
        later("Tags…", icons::TAG, store, Deferred::Tags),
        later(
            "Send to Device",
            icons::SMARTPHONE,
            store,
            Deferred::SendToDevice,
        ),
        menu_separator().boxed(),
        menu_item("Move to Trash")
            .leading_icon(icons::TRASH_2)
            .shortcut_hint("Delete")
            .on_click(move |cx, _| trash_selection(cx, store))
            .boxed(),
        later("Properties", icons::INFO, store, Deferred::Properties),
    ]
}

fn trash_items(store: Store) -> Vec<AnyView> {
    let [open, quick_look] = open_items(store);
    vec![
        open,
        quick_look,
        menu_separator().boxed(),
        item("Restore", icons::ARCHIVE_RESTORE, store, restore_selection),
        menu_item("Delete Permanently…")
            .leading_icon(icons::TRASH_2)
            .shortcut_hint("Delete")
            .on_click(move |cx, _| delete_selection(cx, store))
            .boxed(),
    ]
}

fn open_with_items(store: Store, apps: Option<Vec<MenuApp>>) -> Vec<AnyView> {
    let mut items = vec![
        menu_item("Back")
            .leading_icon(icons::CHEVRON_LEFT)
            .on_click(move |cx, event| {
                event.stop_propagation();
                store.menu_page.set(cx, MenuPage::Main);
            })
            .boxed(),
        menu_header("Open With").boxed(),
    ];
    match apps {
        None => items.push(menu_item_disabled("Looking for applications…").boxed()),
        Some(apps) if apps.is_empty() => {
            items.push(menu_item_disabled("No applications for this type").boxed());
        }
        Some(apps) => items.extend(apps.into_iter().map(|app| {
            let id = app.id;
            menu_item(app.name)
                .leading_icon(icons::APP_WINDOW)
                .on_click(move |cx, _| open_with(cx, store, id.clone()))
                .boxed()
        })),
    }
    items.push(menu_separator().boxed());
    items.push(item(
        "Other Application…",
        icons::LAYOUT_GRID,
        store,
        choose_app,
    ));
    items
}

fn folder_items(store: Store, in_trash: bool) -> Vec<AnyView> {
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
    let first = if in_trash {
        item("Empty Trash", icons::TRASH_2, store, empty_trash)
    } else {
        menu_item("New Folder")
            .leading_icon(icons::FOLDER_PLUS)
            .shortcut_hint("Shift+Ctrl+N")
            .on_click(move |cx, _| new_folder(cx, store))
            .boxed()
    };
    let mut items = vec![first];
    if !in_trash {
        items.push(
            menu_item("Paste")
                .leading_icon(icons::CLIPBOARD_PASTE)
                .shortcut_hint("Ctrl+V")
                .on_click(move |cx, _| paste(cx, store))
                .boxed(),
        );
    }
    items.extend([
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
    ]);
    items
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
