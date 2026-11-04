use crownui::{
    kit::use_theme,
    prelude::{Cx, Runtime, Styled, View, bind, hstack, text},
};

use crate::{
    state::Store,
    util::theme::tint,
    views::{location_bar::location_bar, metrics::STATUS_HEIGHT},
};

/// The path of the open folder on the left, "16 items, 1 selected" on the right.
pub fn status_bar(cx: &mut Cx, store: Store) -> impl View + use<> {
    let theme = use_theme(cx);
    let summary = move |runtime: &mut Runtime| {
        store.selection_rev.get(runtime);
        let count = store.count.get(runtime);
        let selected = store
            .folder
            .with(runtime, |folder| folder.selection().len());
        let items = if count == 1 { "item" } else { "items" };
        match selected {
            0 => format!("{count} {items}"),
            selected => format!("{count} {items}, {selected} selected"),
        }
    };
    hstack((
        location_bar(cx, store),
        text(bind(summary))
            .shrink(0.0)
            .text_size(13.0)
            .text_color(tint(theme, |theme| theme.text.muted)),
    ))
    .items_center()
    .gap(12.0)
    .h(STATUS_HEIGHT)
    .shrink(0.0)
    .px(10.0)
}
