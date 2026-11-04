//! Tags of the entries in view, read off the UI thread a page at a time and
//! kept until the listing changes; the context menu's toggles, the details
//! panel's chips and the search box's `tag:` filters read them here.

use std::{
    ffi::OsString,
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
};

use compact_str::CompactString;
use crownos_files_devices::tags::{SearchQuery, Tags, add_tag, read_tags_in, remove_tag};
use crownui::prelude::{Cx, Runtime, Signal, Store as Cell};

use crate::{
    app::{listing::reorder, tasks::Task, virtual_folder::VirtualFolder},
    model::{entry::Entry, folder::Folder},
    state::Store,
    util::hash::{FastHashMap, FastHashSet},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagChange {
    Add,
    Remove,
}

/// The tags read in one folder. A new `generation` drops answers to older
/// requests and asks again, keeping what is known until the answers come.
#[derive(Debug, Default)]
struct TagCache {
    dir: Option<Arc<Path>>,
    generation: u64,
    tags: FastHashMap<CompactString, Tags>,
    asked: FastHashSet<CompactString>,
}

#[derive(Clone, Copy)]
pub struct Tagging {
    cache: Cell<TagCache>,
    /// Moves whenever tags arrive, for the views that show them.
    pub revision: Signal<u64>,
}

impl Tagging {
    pub fn new(cx: &mut Cx) -> Self {
        Self {
            cache: cx.store(TagCache::default()),
            revision: cx.signal(0),
        }
    }

    /// The tags known for `name` in the current folder.
    pub fn of(self, runtime: &Runtime, name: &str) -> Tags {
        self.cache
            .with(runtime, |cache| cache.tags.get(name).cloned())
            .unwrap_or_default()
    }

    /// Reads everything again: the listing changed.
    pub fn forget(self, runtime: &mut Runtime) {
        self.cache.update(runtime, |cache| {
            cache.generation += 1;
            cache.asked.clear();
        });
    }
}

/// Keeps the tags of the page in view coming, or of the whole folder while
/// the search box filters by tag.
pub fn follow_visible_tags(cx: &mut Cx, store: Store) {
    cx.effect(move |runtime| {
        store.revision.get(runtime);
        let visible = store.scroll.visible_range(runtime);
        let whole_folder = store
            .query
            .with(runtime, |query| SearchQuery::parse(query).has_tags());
        request_tags(runtime, store, visible, whole_folder);
    });
}

fn request_tags(runtime: &mut Runtime, store: Store, visible: Range<usize>, whole_folder: bool) {
    let dir = store.dir(runtime);
    if VirtualFolder::of(&dir).is_some() {
        return;
    }
    let cache = store.tagging.cache;
    let wanted: Vec<(CompactString, OsString)> = store.folder.with(runtime, |folder| {
        cache.with(runtime, |cache| {
            let same_dir = cache.dir.as_ref() == Some(&dir);
            let unasked = |entry: &&Entry| !same_dir || !cache.asked.contains(entry.name.as_str());
            let pick = |entry: &Entry| (entry.name.clone(), entry.os_name().to_os_string());
            if whole_folder {
                every_entry(folder).filter(unasked).map(pick).collect()
            } else {
                visible
                    .filter_map(|position| folder.get(position))
                    .filter(unasked)
                    .map(pick)
                    .collect()
            }
        })
    });
    let generation = cache.update(runtime, |cache| {
        if cache.dir.as_ref() != Some(&dir) {
            *cache = TagCache {
                dir: Some(dir.clone()),
                generation: cache.generation + 1,
                ..TagCache::default()
            };
        }
        cache
            .asked
            .extend(wanted.iter().map(|(name, _)| name.clone()));
        cache.generation
    });
    if !wanted.is_empty() {
        store.run(
            runtime,
            Task::ReadTags {
                dir,
                generation,
                entries: wanted,
            },
        );
    }
}

fn every_entry(folder: &Folder) -> impl Iterator<Item = &Entry> {
    let entries = folder.entries();
    entries.ids(true).filter_map(|id| entries.get(id))
}

/// Reads the tags of `entries` in `dir`, on a blocking thread.
pub fn read_entry_tags(
    dir: &Path,
    entries: Vec<(CompactString, OsString)>,
) -> Vec<(CompactString, Tags)> {
    let tags = read_tags_in(dir, entries.iter().map(|(_, raw)| raw.as_os_str()));
    entries
        .into_iter()
        .map(|(name, _)| name)
        .zip(tags)
        .collect()
}

/// Adds or removes `tag` on each of `entries`, on a blocking thread; the
/// first failure is reported and the rest are still tried.
pub fn change_tags(
    entries: Vec<(CompactString, PathBuf)>,
    tag: &str,
    change: TagChange,
) -> (Vec<(CompactString, Tags)>, Option<String>) {
    let mut failure = None;
    let changed = entries
        .into_iter()
        .filter_map(|(name, path)| {
            let result = match change {
                TagChange::Add => add_tag(&path, tag),
                TagChange::Remove => remove_tag(&path, tag),
            };
            result
                .map_err(|error| failure.get_or_insert(error.to_string()).clone())
                .ok()
                .map(|tags| (name, tags))
        })
        .collect();
    (changed, failure)
}

/// Adopts tags read or written for `dir` in `generation` (any, for writes).
pub fn adopt_tags(
    runtime: &mut Runtime,
    store: Store,
    dir: &Path,
    generation: Option<u64>,
    tags: Vec<(CompactString, Tags)>,
) {
    let adopted = store.tagging.cache.update(runtime, |cache| {
        let current = cache.dir.as_deref() == Some(dir)
            && generation.is_none_or(|generation| generation == cache.generation);
        if current {
            cache.tags.extend(tags);
        }
        current
    });
    if !adopted {
        return;
    }
    store
        .tagging
        .revision
        .update(runtime, |revision| *revision += 1);
    if store
        .query
        .with(runtime, |query| SearchQuery::parse(query).has_tags())
    {
        reorder(runtime, store);
    }
}

/// The names in the current folder carrying every tag `query` asks for.
pub fn tagged_names(runtime: &Runtime, store: Store, query: &SearchQuery) -> Vec<CompactString> {
    store.tagging.cache.with(runtime, |cache| {
        cache
            .tags
            .iter()
            .filter(|(_, tags)| query.matches_tags(tags))
            .map(|(name, _)| name.clone())
            .collect()
    })
}

/// The tag names the menu offers for the selection: the configured ones,
/// then any other the selection carries, each with whether every selected
/// entry has it.
pub fn selection_tags(runtime: &mut Runtime, store: Store) -> Vec<(String, bool)> {
    let selected: Vec<Tags> = store.folder.with(runtime, |folder| {
        folder
            .selected_entries()
            .map(|entry| store.tagging.of(runtime, &entry.name))
            .collect()
    });
    let mut offered: Vec<String> = store.tags.with(runtime, |palette| {
        palette.iter().map(|tag| tag.name.clone()).collect()
    });
    for tag in selected.iter().flatten() {
        if !offered.iter().any(|known| known.as_str() == tag.as_str()) {
            offered.push(tag.to_string());
        }
    }
    offered
        .into_iter()
        .map(|tag| {
            let everywhere = !selected.is_empty()
                && selected
                    .iter()
                    .all(|tags| tags.iter().any(|known| known.as_str() == tag));
            (tag, everywhere)
        })
        .collect()
}

/// The menu's tag row: removes `tag` from the selection when every entry has
/// it, adds it to all of them otherwise.
pub fn toggle_selection_tag(cx: &mut Cx, store: Store, tag: &str) {
    let everywhere = selection_tags(cx, store)
        .into_iter()
        .any(|(known, everywhere)| known == tag && everywhere);
    let dir = store.dir(cx);
    let entries: Vec<(CompactString, PathBuf)> = store.folder.with(cx, |folder| {
        folder
            .selected_entries()
            .map(|entry| (entry.name.clone(), entry.path_in(&dir)))
            .collect()
    });
    if entries.is_empty() || VirtualFolder::of(&dir).is_some() {
        return;
    }
    store.run(
        cx,
        Task::Tag {
            dir,
            entries,
            tag: tag.into(),
            change: if everywhere {
                TagChange::Remove
            } else {
                TagChange::Add
            },
        },
    );
}
