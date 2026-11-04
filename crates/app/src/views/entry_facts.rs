use std::{path::Path, sync::Arc};

use crate::{
    app::{thumbnails::thumb_source, virtual_folder::VirtualFolder},
    model::{entry::Entry, places::Places},
    util::{
        friendly_date::Today, kind_label::kind_label, size_label::size_label,
        thumb_store::ThumbSource,
    },
    views::glyph::Glyph,
};

/// What a tile or list row shows of one entry, read once when it mounts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryFacts {
    pub name: String,
    pub size: String,
    pub kind: String,
    pub modified: String,
    pub glyph: Glyph,
    pub thumb: Option<ThumbSource>,
    /// The entry's path when it is a folder, a place to drop files on.
    pub folder: Option<Arc<Path>>,
}

impl EntryFacts {
    pub fn of(entry: &Entry, dir: &Path, places: &Places, today: Today) -> Self {
        let glyph = glyph_of(entry, dir, places);
        Self {
            name: entry.name.to_string(),
            size: size_label(entry).into_owned(),
            kind: kind_label(entry).into_owned(),
            modified: today.describe(entry.stat.modified).unwrap_or_default(),
            glyph,
            thumb: thumb_source(entry, dir, glyph),
            folder: (entry.is_dir() && VirtualFolder::of(dir) != Some(VirtualFolder::Trash))
                .then(|| entry.path_in(dir).into()),
        }
    }
}

impl Default for EntryFacts {
    fn default() -> Self {
        Self {
            name: String::new(),
            size: String::new(),
            kind: String::new(),
            modified: String::new(),
            glyph: Glyph::Page(None),
            thumb: None,
            folder: None,
        }
    }
}

impl EntryFacts {
    /// The facts of the entry listed at `position` in the store's folder.
    pub fn at(
        runtime: &crownui::prelude::Runtime,
        store: crate::state::Store,
        position: usize,
    ) -> Self {
        let today = store.today.with(runtime, |today| *today);
        store
            .places
            .with(runtime, |places| {
                store.folder.with(runtime, |folder| {
                    folder
                        .get(position)
                        .map(|entry| Self::of(entry, folder.dir(), places, today))
                })
            })
            .unwrap_or_default()
    }
}

/// The glyph a tile draws for `entry` of `dir`, special folders marked.
pub fn glyph_of(entry: &Entry, dir: &Path, places: &Places) -> Glyph {
    let special = entry
        .is_dir()
        .then(|| places.special(&entry.path_in(dir)))
        .flatten();
    Glyph::for_entry(entry, special)
}
