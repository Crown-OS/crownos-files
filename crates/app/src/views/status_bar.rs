use crownui::{
    kit::{progress, use_theme},
    prelude::{Cx, Runtime, Styled, View, bind, hstack, show, spacer, text},
};

use crate::{
    state::Store,
    util::{human_size::HumanSize, theme::tint},
    views::metrics::STATUS_HEIGHT,
};

const METER_WIDTH: f32 = 72.0;
const METER_HEIGHT: f32 = 6.0;

/// "16 items, 1 selected" on the left, the free space of this disk on the right.
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
    let free = move || {
        hstack((
            progress(bind(move |runtime: &mut Runtime| {
                store.storage.with(runtime, |usage| {
                    usage.map_or(0.0, |usage| usage.used_fraction())
                })
            }))
            .w(METER_WIDTH)
            .h(METER_HEIGHT),
            text(bind(move |runtime: &mut Runtime| {
                store.storage.with(runtime, |usage| {
                    usage.map_or_else(String::new, |usage| {
                        format!("{} available", HumanSize(usage.available))
                    })
                })
            })),
        ))
        .items_center()
        .gap(8.0)
    };
    hstack((
        text(bind(summary)),
        spacer(),
        show(
            bind(move |runtime: &mut Runtime| store.storage.with(runtime, Option::is_some)),
            free,
            || hstack(()),
        ),
    ))
    .items_center()
    .h(STATUS_HEIGHT)
    .shrink(0.0)
    .px(16.0)
    .text_size(12.0)
    .text_color(tint(theme, |theme| theme.text.muted))
}
