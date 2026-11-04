use std::path::Path;

use crate::{
    model::{entry::Entry, places::Places},
    util::{friendly_date::Today, kind_label::kind_label, size_label::size_label},
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
}

impl EntryFacts {
    pub fn of(entry: &Entry, dir: &Path, places: &Places, today: Today) -> Self {
        let special = entry
            .is_dir()
            .then(|| places.special(&entry.path_in(dir)))
            .flatten();
        Self {
            name: entry.name.to_string(),
            size: size_label(entry).into_owned(),
            kind: kind_label(entry).into_owned(),
            modified: today.describe(entry.stat.modified).unwrap_or_default(),
            glyph: Glyph::for_entry(entry, special),
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
