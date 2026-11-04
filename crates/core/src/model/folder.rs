use std::{cmp::Ordering, path::Path, sync::Arc};

use compact_str::CompactString;
use rustix::io::Errno;

use crate::{
    model::{
        dir_stamp::DirStamp,
        entries::{Entries, EntryId},
        entry::Entry,
        files_op::FilesOp,
        filter::Filter,
        selection::Selection,
        sort::{SortKey, SortOrder},
        ticket::Ticket,
        view_options::ViewOptions,
    },
    util::hash::{FastHashMap, FastHashSet},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Loading,
    Loaded,
    Failed(Errno),
}

/// One directory as the UI shows it.
///
/// `order` is the sorted, filtered permutation the views index into, so
/// re-sorting or filtering never moves an [`Entry`]. Listing chunks are
/// merge-inserted as they stream in; a reload of an already loaded folder is
/// staged and swapped in whole at `Done`, so the old listing stays on screen.
#[derive(Debug, Clone)]
pub struct Folder {
    dir: Arc<Path>,
    entries: Entries,
    staged: Option<Entries>,
    order: Vec<EntryId>,
    view: ViewOptions,
    stage: Stage,
    ticket: Ticket,
    stamp: Option<DirStamp>,
    revision: u64,
    sorted_revision: u64,
    cursor: usize,
    cursor_trace: Option<CompactString>,
    selection: Selection,
    pub scroll_offset: f32,
}

impl Folder {
    pub fn new(dir: Arc<Path>, view: ViewOptions) -> Self {
        Self {
            dir,
            entries: Entries::default(),
            staged: None,
            order: Vec::new(),
            view,
            stage: Stage::Loading,
            ticket: Ticket::default(),
            stamp: None,
            revision: 0,
            sorted_revision: 0,
            cursor: 0,
            cursor_trace: None,
            selection: Selection::default(),
            scroll_offset: 0.0,
        }
    }

    pub fn dir(&self) -> &Arc<Path> {
        &self.dir
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    pub fn is_reloading(&self) -> bool {
        self.staged.is_some()
    }

    /// The directory as it was when last listed, for revalidation.
    pub fn stamp(&self) -> Option<DirStamp> {
        self.stamp
    }

    /// Bumped by every change the views must redraw for.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn view(&self) -> &ViewOptions {
        &self.view
    }

    /// How many entries are listed.
    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// Every child, listed or not.
    pub fn entries(&self) -> &Entries {
        &self.entries
    }

    pub fn order(&self) -> &[EntryId] {
        &self.order
    }

    pub fn get(&self, position: usize) -> Option<&Entry> {
        self.order.get(position).map(|&id| self.entries.entry(id))
    }

    pub fn iter(&self) -> impl Iterator<Item = &Entry> {
        self.order.iter().map(|&id| self.entries.entry(id))
    }

    pub fn set_sort(&mut self, sort: SortOrder) {
        if self.view.sort != sort {
            self.view.sort = sort;
            self.invalidate_order();
        }
    }

    pub fn set_show_hidden(&mut self, show_hidden: bool) {
        if self.view.show_hidden != show_hidden {
            self.view.show_hidden = show_hidden;
            self.invalidate_order();
        }
    }

    pub fn set_filter(&mut self, filter: Option<Filter>) {
        if self.view.filter != filter {
            self.view.filter = filter;
            self.invalidate_order();
        }
    }

    /// Re-sorts after the view options changed. The setters only mark the
    /// order stale, so several changes in one frame cost a single sort.
    pub fn refresh_order(&mut self) {
        if self.sorted_revision == self.revision {
            return;
        }
        self.order.clear();
        self.order.extend(
            self.entries
                .ids(self.view.show_hidden)
                .filter(|&id| self.view.admits(self.entries.entry(id))),
        );
        self.view.sort.sort(&mut self.order, &self.entries);
        self.sorted_revision = self.revision;
        self.reposition();
    }

    pub fn apply(&mut self, op: FilesOp) {
        if *op.dir() != self.dir {
            return;
        }
        self.refresh_order();
        match op {
            FilesOp::Part {
                ticket, entries, ..
            } if self.accept(ticket) => self.extend(entries),
            FilesOp::Done { ticket, stamp, .. } if self.accept(ticket) => self.finish(stamp),
            FilesOp::Fail { ticket, error, .. } if self.accept(ticket) => self.fail(error),
            FilesOp::Upsert { entries, .. } => self.upsert(entries),
            FilesOp::Delete { names, .. } => self.delete(&names),
            FilesOp::Size { sizes, .. } => self.update_sizes(&sizes),
            FilesOp::Part { .. } | FilesOp::Done { .. } | FilesOp::Fail { .. } => {}
        }
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn cursor_entry(&self) -> Option<&Entry> {
        self.get(self.cursor)
    }

    /// Moves the cursor; it follows that entry's name across reloads.
    pub fn set_cursor(&mut self, position: usize) {
        self.cursor = position.min(self.order.len().saturating_sub(1));
        self.cursor_trace = self.get(self.cursor).map(|entry| entry.name.clone());
    }

    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    pub fn is_selected(&self, position: usize) -> bool {
        self.order
            .get(position)
            .is_some_and(|&id| self.selection.contains(id))
    }

    pub fn selected_entries(&self) -> impl Iterator<Item = &Entry> {
        self.selection.iter().map(|id| self.entries.entry(id))
    }

    pub fn select_only(&mut self, position: usize) {
        if let Some(&id) = self.order.get(position) {
            self.selection.select_only(id);
        }
    }

    pub fn toggle_selected(&mut self, position: usize) {
        if let Some(&id) = self.order.get(position) {
            self.selection.toggle(id);
        }
    }

    pub fn select_range_to(&mut self, position: usize) {
        if let Some(&id) = self.order.get(position) {
            self.selection.select_range(&self.order, id);
        }
    }

    /// Selects exactly the entries listed at `positions`, as a rubber band does.
    pub fn select_positions(&mut self, positions: impl IntoIterator<Item = usize>) {
        let ids: Vec<_> = positions
            .into_iter()
            .filter_map(|position| self.order.get(position).copied())
            .collect();
        self.selection.replace(ids);
    }

    /// Where `name` is listed, if it is.
    pub fn position_of(&self, name: &str) -> Option<usize> {
        self.iter().position(|entry| entry.name == name)
    }

    pub fn select_all(&mut self) {
        self.selection.select_all(&self.order);
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }

    fn accept(&mut self, ticket: Ticket) -> bool {
        match ticket.cmp(&self.ticket) {
            Ordering::Less => false,
            Ordering::Equal => true,
            Ordering::Greater => {
                self.ticket = ticket;
                if self.stage == Stage::Loaded {
                    self.staged = Some(Entries::default());
                } else {
                    self.clear();
                }
                true
            }
        }
    }

    fn extend(&mut self, chunk: Vec<Entry>) {
        if let Some(staged) = &mut self.staged {
            staged.reserve(chunk.len());
            chunk.into_iter().for_each(|entry| {
                staged.push(entry);
            });
            return;
        }
        self.entries.reserve(chunk.len());
        let ids: Vec<_> = chunk
            .into_iter()
            .map(|entry| self.entries.push(entry))
            .collect();
        self.insert_listed(ids);
    }

    fn finish(&mut self, stamp: DirStamp) {
        if let Some(staged) = self.staged.take() {
            let selected: FastHashSet<CompactString> = self
                .selected_entries()
                .map(|entry| entry.name.clone())
                .collect();
            self.entries = staged;
            self.selection.replace(
                self.entries
                    .ids(true)
                    .filter(|&id| selected.contains(&self.entries.entry(id).name)),
            );
            self.invalidate_order();
            self.refresh_order();
        }
        self.stage = Stage::Loaded;
        self.stamp = Some(stamp);
        self.touch();
    }

    fn fail(&mut self, error: Errno) {
        self.clear();
        self.stage = Stage::Failed(error);
        self.stamp = None;
        self.touch();
    }

    fn clear(&mut self) {
        self.entries = Entries::default();
        self.staged = None;
        self.order.clear();
        self.selection.clear();
        self.cursor = 0;
    }

    fn upsert(&mut self, incoming: Vec<Entry>) {
        if incoming.is_empty() {
            return;
        }
        if let Some(staged) = &mut self.staged {
            staged.upsert(incoming.clone());
        }
        let written = self.entries.upsert(incoming);
        let rewritten: FastHashSet<EntryId> = written.iter().copied().collect();
        self.order.retain(|id| !rewritten.contains(id));
        self.insert_listed(written);
    }

    fn delete(&mut self, names: &[CompactString]) {
        let doomed: FastHashSet<&str> = names.iter().map(CompactString::as_str).collect();
        let survives = |entry: &Entry| !doomed.contains(entry.name.as_str());
        if let Some(staged) = &mut self.staged {
            staged.retain(survives);
        }
        let Some(remap) = self.entries.retain(survives) else {
            return;
        };
        self.order
            .retain_mut(|id| remap.apply(*id).map(|moved| *id = moved).is_some());
        self.selection.remap(|id| remap.apply(id));
        self.touch();
    }

    fn update_sizes(&mut self, sizes: &[(CompactString, u64)]) {
        let sizes: FastHashMap<&str, u64> = sizes
            .iter()
            .map(|(name, len)| (name.as_str(), *len))
            .collect();
        let mut changed = false;
        for entry in self.entries.iter_mut().filter(|entry| entry.is_dir()) {
            if let Some(&len) = sizes.get(entry.name.as_str()) {
                changed |= entry.stat.len != len;
                entry.stat.len = len;
            }
        }
        if !changed {
            return;
        }
        if self.view.sort.key == SortKey::Size {
            self.invalidate_order();
            self.refresh_order();
        } else {
            self.touch();
        }
    }

    fn insert_listed(&mut self, mut ids: Vec<EntryId>) {
        ids.retain(|&id| self.view.admits(self.entries.entry(id)));
        self.view
            .sort
            .merge_insert(&mut self.order, ids, &self.entries);
        self.touch();
    }

    /// Records a change that left `order` valid.
    fn touch(&mut self) {
        let sorted = self.sorted_revision == self.revision;
        self.revision += 1;
        if sorted {
            self.sorted_revision = self.revision;
        }
        self.reposition();
    }

    fn invalidate_order(&mut self) {
        self.revision += 1;
    }

    fn reposition(&mut self) {
        let traced = self.cursor_trace.as_ref().and_then(|name| {
            self.order
                .iter()
                .position(|&id| self.entries.entry(id).name == *name)
        });
        self.cursor = traced.unwrap_or_else(|| self.cursor.min(self.order.len().saturating_sub(1)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        entry::EntryExtra,
        stat::{Stat, StatFlags},
        timestamp::Timestamp,
    };

    const DIRECTORY_MODE: u16 = 0o040755;

    fn dir() -> Arc<Path> {
        Path::new("/folder").into()
    }

    fn entry(name: &str) -> Entry {
        let flags = if name.starts_with('.') {
            StatFlags::HIDDEN
        } else {
            StatFlags::empty()
        };
        let mode = if name.ends_with('/') {
            DIRECTORY_MODE
        } else {
            0
        };
        Entry::new(
            name.trim_end_matches('/').into(),
            Stat {
                flags,
                mode,
                ..Stat::default()
            },
            EntryExtra::default(),
        )
    }

    fn stamp() -> DirStamp {
        DirStamp {
            modified: Timestamp::UNKNOWN,
            changed: Timestamp::UNKNOWN,
            len: 0,
            inode: 1,
            device: 1,
        }
    }

    fn part(ticket: Ticket, names: &[&str]) -> FilesOp {
        FilesOp::Part {
            dir: dir(),
            ticket,
            entries: names.iter().map(|name| entry(name)).collect(),
        }
    }

    fn done(ticket: Ticket) -> FilesOp {
        FilesOp::Done {
            dir: dir(),
            ticket,
            stamp: stamp(),
        }
    }

    fn listed(folder: &Folder) -> Vec<&str> {
        folder.iter().map(|entry| entry.name.as_str()).collect()
    }

    fn loaded(names: &[&str]) -> Folder {
        let mut folder = Folder::new(dir(), ViewOptions::default());
        let ticket = Ticket::next();
        folder.apply(part(ticket, names));
        folder.apply(done(ticket));
        folder
    }

    #[test]
    fn streamed_chunks_merge_into_sorted_order() {
        let mut folder = Folder::new(dir(), ViewOptions::default());
        let ticket = Ticket::next();
        folder.apply(part(ticket, &["b", "d"]));
        assert_eq!(listed(&folder), ["b", "d"]);
        folder.apply(part(ticket, &["c", "a", "z/"]));
        assert_eq!(listed(&folder), ["z", "a", "b", "c", "d"]);
        assert_eq!(folder.stage(), Stage::Loading);
        folder.apply(done(ticket));
        assert_eq!(folder.stage(), Stage::Loaded);
        assert_eq!(folder.stamp(), Some(stamp()));
    }

    #[test]
    fn chunks_of_a_superseded_listing_are_dropped() {
        let mut folder = Folder::new(dir(), ViewOptions::default());
        let old = Ticket::next();
        let new = Ticket::next();
        folder.apply(part(new, &["fresh"]));
        folder.apply(part(old, &["stale"]));
        folder.apply(done(old));
        assert_eq!(listed(&folder), ["fresh"]);
        assert_eq!(folder.stage(), Stage::Loading);
    }

    #[test]
    fn ops_for_another_directory_are_ignored() {
        let mut folder = loaded(&["a"]);
        folder.apply(FilesOp::Delete {
            dir: Path::new("/elsewhere").into(),
            names: vec!["a".into()],
        });
        assert_eq!(listed(&folder), ["a"]);
    }

    #[test]
    fn a_reload_is_staged_and_keeps_cursor_and_selection_by_name() {
        let mut folder = loaded(&["a", "b", "c"]);
        folder.set_cursor(2);
        folder.select_only(1);
        let ticket = Ticket::next();
        folder.apply(part(ticket, &["0", "c", "b"]));
        assert_eq!(listed(&folder), ["a", "b", "c"]);
        assert!(folder.is_reloading());
        folder.apply(done(ticket));
        assert_eq!(listed(&folder), ["0", "b", "c"]);
        assert_eq!(
            folder.cursor_entry().map(|entry| entry.name.as_str()),
            Some("c")
        );
        assert_eq!(
            folder
                .selected_entries()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            ["b"]
        );
    }

    #[test]
    fn hidden_files_and_filters_reorder_without_reloading() {
        let mut folder = loaded(&[".git/", "main.rs", ".env", "Cargo.toml"]);
        assert_eq!(listed(&folder), ["Cargo.toml", "main.rs"]);
        let revision = folder.revision();
        folder.set_show_hidden(true);
        folder.set_filter(Filter::new("."));
        assert_ne!(folder.revision(), revision);
        folder.refresh_order();
        assert_eq!(listed(&folder), [".git", ".env", "Cargo.toml", "main.rs"]);
        folder.set_filter(Filter::new("rs"));
        folder.refresh_order();
        assert_eq!(listed(&folder), ["main.rs"]);
        assert_eq!(folder.entries().len(), 4);
    }

    #[test]
    fn upserts_and_deletes_keep_order_and_selection() {
        let mut folder = loaded(&["a", "c", "e"]);
        folder.select_all();
        folder.apply(FilesOp::Upsert {
            dir: dir(),
            entries: vec![entry("d"), entry("a")],
        });
        assert_eq!(listed(&folder), ["a", "c", "d", "e"]);
        folder.apply(FilesOp::Delete {
            dir: dir(),
            names: vec!["a".into(), "missing".into()],
        });
        assert_eq!(listed(&folder), ["c", "d", "e"]);
        let selected: Vec<_> = folder
            .selected_entries()
            .map(|entry| entry.name.as_str())
            .collect();
        assert_eq!(selected, ["c", "e"]);
    }

    #[test]
    fn measured_sizes_resort_when_sorting_by_size() {
        let view = ViewOptions {
            sort: SortOrder {
                key: SortKey::Size,
                reverse: true,
                dirs_first: false,
            },
            ..ViewOptions::default()
        };
        let mut folder = Folder::new(dir(), view);
        let ticket = Ticket::next();
        folder.apply(part(ticket, &["small/", "big/"]));
        folder.apply(FilesOp::Size {
            dir: dir(),
            sizes: vec![("big".into(), 9), ("small".into(), 1)],
        });
        assert_eq!(listed(&folder), ["big", "small"]);
        assert_eq!(folder.get(0).map(|entry| entry.stat.len), Some(9));
    }

    #[test]
    fn a_failure_empties_the_folder() {
        let mut folder = loaded(&["a"]);
        folder.apply(FilesOp::Fail {
            dir: dir(),
            ticket: Ticket::next(),
            error: Errno::ACCESS,
        });
        assert!(folder.is_empty());
        assert_eq!(folder.stage(), Stage::Failed(Errno::ACCESS));
    }
}
