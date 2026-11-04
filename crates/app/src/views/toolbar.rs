//! The header row: back, forward and up, search centered, the view switch,
//! with the empty space between them moving the window.

use crownconfig::ViewMode;
use crownui::{
    kit::{
        ButtonOptions, ButtonShape, ButtonVariant, TextInputOptions, ToggleItem, icon_button,
        icons, search, title_bar_spacer, toggle_group,
    },
    prelude::{Cx, Listeners, Runtime, Semantic, Spring, Styled, View, ViewSeq, bind, hstack},
};

use crate::{
    app::navigation::{go_back, go_forward, go_up},
    state::{NavigationState, Store},
    views::{focus_slot::focus_slot, jobs_indicator::jobs_indicator},
};

/// The search field's height, which the history buttons share.
const FIELD_HEIGHT: f32 = 34.0;
const SEARCH_WIDTH: f32 = 320.0;
const SEARCH_FOCUSED_WIDTH: f32 = 440.0;
const SEARCH_MIN_WIDTH: f32 = 160.0;
const VIEW_MODES: [ViewMode; 2] = [ViewMode::Grid, ViewMode::List];

pub fn toolbar(cx: &mut Cx, store: Store) -> impl ViewSeq + use<> {
    let history = hstack((
        history_button(
            store,
            icons::ARROW_LEFT,
            "Back",
            |state| state.can_go_back,
            go_back,
        ),
        history_button(
            store,
            icons::ARROW_RIGHT,
            "Forward",
            |state| state.can_go_forward,
            go_forward,
        ),
        history_button(store, icons::ARROW_UP, "Up", |state| state.can_go_up, go_up),
        title_bar_spacer(),
    ))
    .items_center()
    .gap(6.0);
    let options = hstack((
        title_bar_spacer(),
        jobs_indicator(cx, store),
        view_switch(cx, store),
    ))
    .items_center()
    .gap(6.0);
    (history.flex_1(), search_box(store), options.flex_1())
}

/// A round history button, enabled while `leads_somewhere` holds for the navigation state.
fn history_button(
    store: Store,
    glyph: &'static str,
    label: &'static str,
    leads_somewhere: fn(NavigationState) -> bool,
    navigate: fn(&mut Runtime, Store),
) -> impl View {
    icon_button(glyph, label)
        .variant(ButtonVariant::Secondary)
        .shape(ButtonShape::Pill)
        .size(FIELD_HEIGHT)
        .disabled(bind(move |runtime: &mut Runtime| {
            !leads_somewhere(store.navigation.get(runtime))
        }))
        .on_click(move |cx, _| navigate(cx, store))
}

/// The icons/list switch, kept in step with the store's view mode.
fn view_switch(cx: &mut Cx, store: Store) -> impl View + use<> {
    let index_of = |mode| {
        VIEW_MODES
            .iter()
            .position(|known| *known == mode)
            .unwrap_or(0)
    };
    let initial = index_of(store.view_mode.get(cx));
    let selected = cx.signal(initial);
    cx.effect(move |runtime| {
        let index = index_of(store.view_mode.get(runtime));
        if selected.with_untracked(runtime, |current| *current != index) {
            selected.set(runtime, index);
        }
    });
    cx.effect(move |runtime| {
        let mode = VIEW_MODES[selected.get(runtime).min(VIEW_MODES.len() - 1)];
        if store
            .view_mode
            .with_untracked(runtime, |current| *current != mode)
        {
            store.view_mode.set(runtime, mode);
        }
    });
    toggle_group(
        [
            ToggleItem::icon(icons::LAYOUT_GRID, "Icons"),
            ToggleItem::icon(icons::LIST, "List"),
        ],
        selected,
    )
    .label("View")
}

/// Search: an instant filter of this folder, wider while in use, narrower when the header is.
fn search_box(store: Store) -> impl View {
    focus_slot(
        search(store.query).placeholder("Search").stretch(),
        store.search,
    )
    .w(bind(move |runtime: &mut Runtime| {
        let active = store.search.is_focused(runtime)
            || store.query.with(runtime, |query| !query.is_empty());
        if active {
            SEARCH_FOCUSED_WIDTH
        } else {
            SEARCH_WIDTH
        }
    }))
    .min_w(SEARCH_MIN_WIDTH)
    .transition(Spring::snappy())
}
