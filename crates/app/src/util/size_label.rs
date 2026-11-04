use std::borrow::Cow;

use crate::{model::entry::Entry, util::human_size::HumanSize};

/// The size under a tile and in the Size column: folders say so until measured.
pub fn size_label(entry: &Entry) -> Cow<'static, str> {
    if entry.is_dir() && entry.stat.len == 0 {
        return Cow::Borrowed("Folder");
    }
    Cow::Owned(HumanSize(entry.stat.len).to_string())
}
