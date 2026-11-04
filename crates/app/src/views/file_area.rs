//! The left of the content card: the grid or list, the rubber band and the
//! empty-folder message, and the pointer handling of the empty area.

use crownconfig::ViewMode;
use crownui::{
    kit::{TEXT_SIZE, use_theme},
    prelude::{
        Cx, Listeners, Point, Role, Runtime, Semantic, Styled, View, bind, show, text, vstack,
        zstack,
    },
};

use crate::{
    app::{
        drag_drop::{drag_over_folder, drop_on_folder, start_entry_drag},
        selection::clear,
    },
    model::folder::Stage,
    state::Store,
    util::theme::tint,
    views::{
        drive_busy_callout::drive_busy_callout,
        elevation_callout::elevation_callout,
        file_grid::file_grid,
        file_list::file_list,
        file_menu::open_menu,
        focus_slot::focus_slot,
        rubber_band::{entry_at, extend_band, finish_band, rubber_band, start_band},
        trash_bar::trash_bar,
    },
};

pub fn file_area(cx: &mut Cx, store: Store) -> impl View + use<> {
    let theme = use_theme(cx);
    let anchor = cx.store(None::<Point>);
    let views = show(
        bind(move |runtime: &mut Runtime| store.view_mode.get(runtime) == ViewMode::Grid),
        move || file_grid(store),
        move || file_list(store),
    )
    .flex_col()
    .w_full()
    .h_full();
    let message = vstack(
        text(bind(move |runtime: &mut Runtime| {
            empty_message(runtime, store)
        }))
        .text_size(TEXT_SIZE)
        .text_color(tint(theme, |theme| theme.text.muted)),
    )
    .items_center()
    .justify_center()
    .pointer_events_none();

    let area = zstack((
        views,
        rubber_band(cx, store),
        message,
        elevation_callout(cx, store),
        drive_busy_callout(store),
    ))
    .flex_1()
    .min_w(0.0)
    .min_h(0.0)
    .focusable(true)
    .role(Role::List)
    .label("Files")
    .on_pointer_down(move |cx, event| {
        store
            .pointer
            .update(cx, |pointer| *pointer = event.position);
    })
    .on_click(move |cx, _| clear(cx, store))
    .on_context_menu(move |cx, _| {
        clear(cx, store);
        open_menu(cx, store);
    })
    .on_drag_start(move |cx, event| {
        if let Some(position) = entry_at(cx, store, event.position) {
            let (badge, badge_text) =
                theme.with(cx, |theme| (theme.accent.start, theme.text.on_accent));
            start_entry_drag(cx, store, position, badge, badge_text);
            return;
        }
        start_band(cx, store, event.position);
        let origin = store.band.with(cx, |band| band.map(|band| band.origin));
        anchor.update(cx, |anchor| *anchor = origin);
    })
    .on_drag(move |cx, event| {
        if let Some(origin) = anchor.with(cx, |anchor| *anchor) {
            extend_band(cx, store, event.position, origin);
        }
    })
    .on_drag_end(move |cx, _| {
        anchor.update(cx, |anchor| *anchor = None);
        finish_band(cx, store);
    })
    .on_drag_over(move |cx, event| {
        let dir = store.dir(cx);
        drag_over_folder(cx, store, &dir, false, event);
    })
    .on_drop(move |cx, event| {
        let dir = store.dir(cx);
        drop_on_folder(cx, store, &dir, event);
    });
    vstack((
        trash_bar(cx, store),
        focus_slot(area, store.file_area).flex_1().min_h(0.0),
    ))
    .flex_1()
    .min_w(0.0)
    .min_h(0.0)
}

fn empty_message(runtime: &mut Runtime, store: Store) -> String {
    store.revision.get(runtime);
    let (stage, empty) = store
        .folder
        .with(runtime, |folder| (folder.stage(), folder.is_empty()));
    if !empty {
        return String::new();
    }
    if store.searching.active.get(runtime) {
        return "Searching…".to_owned();
    }
    match stage {
        Stage::Loading => "Loading…".to_owned(),
        Stage::Failed(error) => format!("This folder can't be opened: {error}"),
        Stage::Loaded if store.query.with(runtime, |query| !query.is_empty()) => store
            .query
            .with(runtime, |query| format!("Nothing here matches “{query}”")),
        Stage::Loaded => "This folder is empty".to_owned(),
    }
}
