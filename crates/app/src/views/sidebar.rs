//! The sidebar: brand, favorite places, devices and the storage card.

use std::{path::Path, sync::Arc};

use crownui::{
    kit::{icons, sidebar_brand, sidebar_item, sidebar_section},
    prelude::{AnyView, Cx, Listeners, Runtime, View, ViewSeq, bind, spacer},
};

use crate::{
    app::{navigation::open_dir, virtual_folder::VirtualFolder},
    model::special_dir::SpecialDir,
    state::Store,
    views::{
        devices_section::devices_section,
        drop_target::{folder_drop_target, trash_drop_target},
        storage_card::storage_card,
    },
};

pub fn sidebar(cx: &mut Cx, store: Store) -> impl ViewSeq + use<> {
    (
        sidebar_brand("Files", icons::FOLDER),
        favorites(cx, store),
        devices_section(store),
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
