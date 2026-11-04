use crate::model::{entry::Entry, filter::Filter, sort::SortOrder};

/// Which of a folder's entries are listed, and in what order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ViewOptions {
    pub sort: SortOrder,
    pub show_hidden: bool,
    pub filter: Option<Filter>,
}

impl ViewOptions {
    pub fn admits(&self, entry: &Entry) -> bool {
        (self.show_hidden || !entry.is_hidden())
            && self
                .filter
                .as_ref()
                .is_none_or(|filter| filter.matches(entry.name.as_bytes()))
    }
}
