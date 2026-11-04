use std::sync::{LazyLock, PoisonError, RwLock};

use crate::{model::mime_id::MimeId, util::hash::FastIndexSet};

/// Every MIME type name seen so far, indexed by [`MimeId`]. Names are never
/// removed, so they are leaked once and handed out as `&'static str`; the
/// whole shared-mime-info database is a few tens of kilobytes.
static NAMES: LazyLock<RwLock<FastIndexSet<&'static str>>> =
    LazyLock::new(|| RwLock::new(std::iter::once("").collect()));

/// The process-wide MIME type interner.
pub struct MimeTable;

impl MimeTable {
    /// The id of `name`, adding it on first sight. Returns
    /// [`MimeId::UNKNOWN`] for an empty name or once the table is full.
    pub fn intern(name: &str) -> MimeId {
        if let Some(id) = Self::lookup(name) {
            return id;
        }
        let mut names = NAMES.write().unwrap_or_else(PoisonError::into_inner);
        if let Some(index) = names.get_index_of(name) {
            return to_id(index);
        }
        if name.is_empty() || names.len() > usize::from(u16::MAX) {
            return MimeId::UNKNOWN;
        }
        let (index, _) = names.insert_full(Box::leak(name.into()));
        to_id(index)
    }

    /// The id of `name` if it was interned before.
    pub fn lookup(name: &str) -> Option<MimeId> {
        let names = NAMES.read().unwrap_or_else(PoisonError::into_inner);
        names.get_index_of(name).map(to_id)
    }

    pub fn name(id: MimeId) -> Option<&'static str> {
        if id == MimeId::UNKNOWN {
            return None;
        }
        let names = NAMES.read().unwrap_or_else(PoisonError::into_inner);
        names.get_index(usize::from(id.0)).copied()
    }
}

fn to_id(index: usize) -> MimeId {
    u16::try_from(index).map_or(MimeId::UNKNOWN, MimeId)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interning_is_stable_and_round_trips() {
        let first = MimeTable::intern("application/x-table-test");
        assert_ne!(first, MimeId::UNKNOWN);
        assert_eq!(MimeTable::intern("application/x-table-test"), first);
        assert_eq!(MimeTable::name(first), Some("application/x-table-test"));
        assert_eq!(MimeTable::lookup("application/x-never-interned"), None);
    }

    #[test]
    fn unknown_has_no_name() {
        assert_eq!(MimeTable::intern(""), MimeId::UNKNOWN);
        assert_eq!(MimeTable::name(MimeId::UNKNOWN), None);
    }
}
