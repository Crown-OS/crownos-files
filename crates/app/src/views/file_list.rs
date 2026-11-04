use crownui::prelude::{LazyListOptions, ScrollOptions, Styled, View, lazy_list};

use crate::{
    state::Store,
    views::{
        list_row::list_row,
        metrics::{LIST_PADDING, ROW_HEIGHT},
    },
};

/// The list view: 30px rows, only those in view mounted.
pub fn file_list(store: Store) -> impl View {
    lazy_list(
        store.count,
        store.revision,
        ROW_HEIGHT,
        move |cx, position| list_row(cx, store, position),
    )
    .content_padding(LIST_PADDING)
    .scroll_handle(store.scroll)
    .flex_1()
}
