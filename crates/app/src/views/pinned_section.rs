//! The sidebar's Pinned folders, and the zone that opens under them while a
//! folder is dragged, to pin it.

use std::{path::Path, sync::Arc};

use crownui::{
    kit::{
        ButtonOptions, ButtonVariant, SidebarItemOptions, Theme, icon_button, icon_colored, icons,
        sidebar_item, sidebar_section, use_theme,
    },
    prelude::{
        AnyView, Cx, Listeners, Runtime, Semantic, Signal, Spring, Styled, TRANSPARENT, View, bind,
        for_each, hstack, show, text, vstack,
    },
};

use crate::{
    app::{
        navigation::open_dir,
        pinning::{drag_over_pin_zone, drop_on_pin_zone, unpin},
    },
    state::Store,
    util::theme::drop_wash,
    views::drop_target::folder_drop_target,
};

const ZONE_HEIGHT: f32 = 40.0;
const ZONE_BORDER: f32 = 1.5;
const ZONE_HOVER_SCALE: f32 = 1.04;
const PIN_STROKE: f32 = 2.0;
const PIN_SIZE: f32 = 16.0;

pub fn pinned_section(cx: &mut Cx, store: Store) -> impl View + use<> {
    let theme = use_theme(cx);
    let zone_hovered = cx.signal(false);
    show(
        bind(move |runtime: &mut Runtime| {
            store.drag.pin_zone.get(runtime)
                || store.pinned.with(runtime, |pinned| !pinned.is_empty())
        }),
        move || {
            sidebar_section(
                "Pinned",
                vstack((
                    for_each(
                        store.pinned,
                        |dir: &Arc<Path>| dir.clone(),
                        move |cx, dir| pinned_row(cx, store, dir),
                    )
                    .flex_col()
                    .gap(4.0),
                    pin_zone(store, theme, zone_hovered),
                ))
                .gap(4.0),
            )
        },
        || vstack(()),
    )
    .flex_col()
}

fn pinned_row(cx: &mut Cx, store: Store, dir: Signal<Arc<Path>>) -> AnyView {
    let dir = dir.get(cx);
    let name = dir.file_name().map_or_else(
        || dir.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let hovered = cx.signal(false);
    let (current, opened, unpinned) = (dir.clone(), dir.clone(), dir.clone());
    let unpin_button = move || {
        let unpinned = unpinned.clone();
        icon_button(icons::X, "Unpin")
            .variant(ButtonVariant::Ghost)
            .on_click(move |cx, event| {
                event.stop_propagation();
                unpin(cx, store, &unpinned);
            })
    };
    let item = sidebar_item(
        name.clone(),
        icons::FOLDER,
        bind(move |runtime: &mut Runtime| store.cwd.with(runtime, |cwd| *cwd == current)),
    )
    .trailing(
        show(
            bind(move |runtime: &mut Runtime| hovered.get(runtime)),
            unpin_button,
            || hstack(()),
        )
        .flex_row(),
    )
    .label(name)
    .on_pointer_enter(move |cx, _| hovered.set(cx, true))
    .on_pointer_leave(move |cx, _| hovered.set(cx, false))
    .on_click(move |cx, _| open_dir(cx, store, opened.clone()));
    folder_drop_target(item, store, dir, cx.signal(false)).boxed()
}

/// Folded to nothing until a folder drag starts, then springs open; swells
/// and fills with the accent while the drag is over it.
fn pin_zone(store: Store, theme: Signal<Theme>, hovered: Signal<bool>) -> impl View {
    let shown = move |runtime: &mut Runtime| store.drag.pin_zone.get(runtime);
    let caption = move |runtime: &mut Runtime| {
        if hovered.get(runtime) {
            "Release to pin"
        } else {
            "Drop a folder to pin it"
        }
        .to_owned()
    };
    hstack((
        icon_colored(icons::PIN, PIN_STROKE, |theme| theme.accent.start).size(PIN_SIZE),
        text(bind(caption)).text_size(13.0).font_medium().truncate(),
    ))
    .items_center()
    .justify_center()
    .gap(8.0)
    .shrink(0.0)
    .overflow_hidden()
    .h(bind(
        move |runtime: &mut Runtime| {
            if shown(runtime) { ZONE_HEIGHT } else { 0.0 }
        },
    ))
    .opacity(bind(
        move |runtime: &mut Runtime| {
            if shown(runtime) { 1.0 } else { 0.0 }
        },
    ))
    .scale(bind(move |runtime: &mut Runtime| {
        if hovered.get(runtime) {
            ZONE_HOVER_SCALE
        } else {
            1.0
        }
    }))
    .rounded(ZONE_HEIGHT / 2.0)
    .border(ZONE_BORDER)
    .border_color(bind(move |runtime: &mut Runtime| {
        let hovered = hovered.get(runtime);
        theme.with(runtime, |theme| {
            if hovered {
                theme.accent.start
            } else {
                theme.surface.border
            }
        })
    }))
    .bg(bind(move |runtime: &mut Runtime| {
        let hovered = hovered.get(runtime);
        theme.with(runtime, |theme| {
            if hovered {
                drop_wash(theme)
            } else {
                TRANSPARENT
            }
        })
    }))
    .text_color(bind(move |runtime: &mut Runtime| {
        let hovered = hovered.get(runtime);
        theme.with(runtime, |theme| {
            if hovered {
                theme.text.primary
            } else {
                theme.text.muted
            }
        })
    }))
    .transition(Spring::bouncy())
    .label("Pin folder")
    .on_drag_over(move |cx, event| {
        let accepted = drag_over_pin_zone(cx, store, event);
        if hovered.get(cx) != accepted {
            hovered.set(cx, accepted);
        }
    })
    .on_drag_leave(move |cx, _| hovered.set(cx, false))
    .on_drop(move |cx, event| {
        hovered.set(cx, false);
        drop_on_pin_zone(cx, store, event);
    })
}
