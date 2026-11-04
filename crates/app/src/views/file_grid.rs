use crownui::prelude::{LazyListOptions, ScrollOptions, Styled, View, lazy_grid};

use crate::{
    state::Store,
    views::{
        file_tile::file_tile,
        metrics::{GRID_PADDING, TILE, TILE_GAP},
    },
};

/// The icon view: only the rows in view are mounted.
pub fn file_grid(store: Store) -> impl View {
    lazy_grid(
        store.count,
        store.revision,
        TILE,
        TILE_GAP,
        move |cx, position| file_tile(cx, store, position),
    )
    .content_padding(GRID_PADDING)
    .scroll_handle(store.scroll)
    .flex_1()
}
