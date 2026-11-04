//! The sidebar: brand, favorite places, devices and the storage card.

use std::{path::Path, sync::Arc};

use crownui::{
    kit::{
        SidebarItemOptions, icon_button, icons, sidebar_brand, sidebar_item, sidebar_section,
        use_theme,
    },
    prelude::{
        AnyView, Cx, Listeners, Runtime, Signal, Styled, View, ViewSeq, bind, for_each, hstack,
        show, spacer, vstack,
    },
};

use crate::{
    app::{navigation::open_dir, virtual_folder::VirtualFolder},
    model::{
        device_entry::{DeviceEntry, DeviceKind},
        special_dir::SpecialDir,
    },
    state::Store,
    util::theme::tint,
    views::{
        drop_target::{folder_drop_target, trash_drop_target},
        storage_card::storage_card,
    },
};

const STATUS_DOT: f32 = 7.0;

pub fn sidebar(cx: &mut Cx, store: Store) -> impl ViewSeq + use<> {
    (
        sidebar_brand("Files", icons::FOLDER),
        favorites(cx, store),
        devices(store),
        spacer(),
        storage_card(cx, store),
    )
}

fn favorites(cx: &mut Cx, store: Store) -> impl View + use<> {
    let candidates = store.places.with(cx, |places| {
        let special = |dir| places.path(dir).map(Arc::<Path>::from);
        let has_trash = places.trash.is_some();
        [
            ("Recents", icons::CLOCK, Some(VirtualFolder::Recents.path())),
            ("Home", icons::HOUSE, places.home.as_deref().map(Arc::from)),
            ("Desktop", icons::MONITOR, special(SpecialDir::Desktop)),
            (
                "Documents",
                icons::FILE_TEXT,
                special(SpecialDir::Documents),
            ),
            ("Downloads", icons::DOWNLOAD, special(SpecialDir::Downloads)),
            ("Pictures", icons::IMAGE, special(SpecialDir::Pictures)),
            ("Videos", icons::VIDEO, special(SpecialDir::Videos)),
            ("Music", icons::MUSIC, special(SpecialDir::Music)),
            (
                "Trash",
                icons::TRASH_2,
                has_trash.then(|| VirtualFolder::Trash.path()),
            ),
        ]
    });
    let rows: Vec<AnyView> = candidates
        .into_iter()
        .filter_map(|(label, icon, dir)| {
            let dir = dir.filter(|dir| VirtualFolder::of(dir).is_some() || dir.is_dir())?;
            Some(place(cx, store, label, icon, dir))
        })
        .collect();
    sidebar_section("Favorites", rows)
}

fn place(
    cx: &mut Cx,
    store: Store,
    label: &'static str,
    icon: &'static str,
    dir: Arc<Path>,
) -> AnyView {
    let current = dir.clone();
    let opened = dir.clone();
    let item = sidebar_item(
        label,
        icon,
        bind(move |runtime: &mut Runtime| store.cwd.with(runtime, |cwd| *cwd == current)),
    )
    .on_click(move |cx, _| open_dir(cx, store, opened.clone()));
    match VirtualFolder::of(&dir) {
        Some(VirtualFolder::Trash) => trash_drop_target(item, store).boxed(),
        Some(VirtualFolder::Recents) => item.boxed(),
        None => folder_drop_target(item, store, dir, cx.signal(false)).boxed(),
    }
}

fn devices(store: Store) -> impl View {
    show(
        bind(move |runtime: &mut Runtime| {
            store.devices.with(runtime, |devices| !devices.is_empty())
        }),
        move || {
            sidebar_section(
                "Devices",
                for_each(
                    store.devices,
                    |device: &DeviceEntry| device.name.clone(),
                    move |cx, device| device_row(cx, store, device),
                )
                .flex_col()
                .gap(4.0),
            )
        },
        || vstack(()),
    )
    .flex_col()
}

fn device_row(cx: &mut Cx, store: Store, device: Signal<DeviceEntry>) -> AnyView {
    let theme = use_theme(cx);
    let entry = device.get(cx);
    match entry.kind {
        DeviceKind::Drive => {
            let mount = entry.mount.clone();
            let selected = mount.clone();
            let item = sidebar_item(
                entry.name.clone(),
                icons::HARD_DRIVE,
                bind(move |runtime: &mut Runtime| {
                    selected
                        .as_ref()
                        .is_some_and(|dir| store.cwd.with(runtime, |cwd| cwd == dir))
                }),
            )
            .trailing(icon_button(
                icons::ARROW_UP_FROM_LINE,
                format!("Eject {}", entry.name),
            ))
            .on_click(move |cx, _| {
                if let Some(dir) = &mount {
                    open_dir(cx, store, dir.clone());
                }
            });
            match entry.mount {
                Some(dir) => folder_drop_target(item, store, dir, cx.signal(false)).boxed(),
                None => item.boxed(),
            }
        }
        DeviceKind::Phone => sidebar_item(entry.name, icons::SMARTPHONE, false)
            .trailing(
                hstack(())
                    .size(STATUS_DOT)
                    .rounded(STATUS_DOT / 2.0)
                    .bg(tint(theme, move |theme| {
                        if entry.online {
                            theme.status.success
                        } else {
                            theme.status.neutral
                        }
                    })),
            )
            .boxed(),
    }
}
