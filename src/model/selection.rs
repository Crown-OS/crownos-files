use crate::{model::entries::EntryId, util::hash::FastIndexSet};

/// The selected entries in the order they were picked, plus the anchor a
/// Shift-click extends from.
#[derive(Debug, Clone, Default)]
pub struct Selection {
    ids: FastIndexSet<EntryId>,
    anchor: Option<EntryId>,
}

impl Selection {
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn contains(&self, id: EntryId) -> bool {
        self.ids.contains(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = EntryId> {
        self.ids.iter().copied()
    }

    pub fn anchor(&self) -> Option<EntryId> {
        self.anchor
    }

    /// A plain click.
    pub fn select_only(&mut self, id: EntryId) {
        self.ids.clear();
        self.ids.insert(id);
        self.anchor = Some(id);
    }

    /// A Ctrl-click.
    pub fn toggle(&mut self, id: EntryId) {
        if !self.ids.shift_remove(&id) {
            self.ids.insert(id);
        }
        self.anchor = Some(id);
    }

    /// A Shift-click: everything listed between the anchor and `to`.
    pub fn select_range(&mut self, order: &[EntryId], to: EntryId) {
        let position = |id| order.iter().position(|&listed| listed == id);
        let Some(end) = position(to) else {
            return;
        };
        let start = self.anchor.and_then(position).unwrap_or(end);
        self.anchor = Some(order[start]);
        self.ids.clear();
        self.ids
            .extend(order[start.min(end)..=start.max(end)].iter().copied());
    }

    pub fn select_all(&mut self, order: &[EntryId]) {
        self.ids.clear();
        self.ids.extend(order.iter().copied());
    }

    pub fn clear(&mut self) {
        self.ids.clear();
        self.anchor = None;
    }

    /// Follows entries that moved, forgetting the ones that are gone.
    pub fn remap(&mut self, moved: impl Fn(EntryId) -> Option<EntryId>) {
        self.ids = self.ids.iter().filter_map(|&id| moved(id)).collect();
        self.anchor = self.anchor.and_then(moved);
    }

    pub(crate) fn replace(&mut self, ids: impl IntoIterator<Item = EntryId>) {
        self.ids.clear();
        self.ids.extend(ids);
        self.anchor = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        entries::Entries,
        entry::{Entry, EntryExtra},
        stat::Stat,
    };

    fn order(count: usize) -> Vec<EntryId> {
        let mut entries = Entries::default();
        (0..count)
            .map(|index| {
                entries.push(Entry::new(
                    index.to_string().into(),
                    Stat::default(),
                    EntryExtra::default(),
                ))
            })
            .collect()
    }

    #[test]
    fn range_runs_from_the_anchor_in_either_direction() {
        let order = order(8);
        let mut selection = Selection::default();
        selection.select_only(order[2]);
        selection.select_range(&order, order[5]);
        assert_eq!(selection.iter().collect::<Vec<_>>(), &order[2..=5]);
        selection.select_range(&order, order[0]);
        assert_eq!(selection.iter().collect::<Vec<_>>(), &order[0..=2]);
        assert_eq!(selection.anchor(), Some(order[2]));
    }

    #[test]
    fn a_range_without_anchor_selects_the_target() {
        let order = order(3);
        let mut selection = Selection::default();
        selection.select_range(&order, order[1]);
        assert_eq!(selection.iter().collect::<Vec<_>>(), [order[1]]);
    }

    #[test]
    fn toggle_adds_and_removes_while_keeping_pick_order() {
        let order = order(4);
        let mut selection = Selection::default();
        selection.toggle(order[3]);
        selection.toggle(order[1]);
        selection.toggle(order[2]);
        selection.toggle(order[1]);
        assert_eq!(selection.iter().collect::<Vec<_>>(), [order[3], order[2]]);
    }

    #[test]
    fn select_all_then_remap_drops_removed_entries() {
        let order = order(3);
        let mut selection = Selection::default();
        selection.select_all(&order);
        assert_eq!(selection.len(), 3);
        selection.remap(|id| (id != order[1]).then_some(id));
        assert_eq!(selection.iter().collect::<Vec<_>>(), [order[0], order[2]]);
    }
}
