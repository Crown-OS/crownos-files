//! The header row: back and forward, the path, the view switch and search.

use crownconfig::ViewMode;
use crownui::{
    kit::{
        ButtonOptions, ButtonVariant, FieldShape, PathSegment, TextInputOptions, ToggleItem,
        icon_button, icons, path_bar, search, text_input, toggle_group,
    },
    prelude::{Cx, Listeners, Runtime, Semantic, Spring, Styled, View, ViewSeq, bind, show},
};

use crate::{
    app::{
        editing::{Editing, cancel, commit},
        navigation::{go_back, go_forward, open_dir},
    },
    state::Store,
    util::breadcrumbs::breadcrumbs,
    views::focus_slot::{FocusSlotOptions, autofocused, focus_slot},
};

const SEARCH_WIDTH: f32 = 136.0;
const SEARCH_FOCUSED_WIDTH: f32 = 240.0;
const VIEW_MODES: [ViewMode; 2] = [ViewMode::Grid, ViewMode::List];

pub fn toolbar(cx: &mut Cx, store: Store) -> impl ViewSeq + use<> {
    (
        icon_button(icons::CHEVRON_LEFT, "Back")
            .variant(ButtonVariant::Ghost)
            .disabled(bind(move |runtime: &mut Runtime| {
                !store.navigation.get(runtime).can_go_back
            }))
            .on_click(move |cx, _| go_back(cx, store)),
        icon_button(icons::CHEVRON_RIGHT, "Forward")
            .variant(ButtonVariant::Ghost)
            .disabled(bind(move |runtime: &mut Runtime| {
                !store.navigation.get(runtime).can_go_forward
            }))
            .on_click(move |cx, _| go_forward(cx, store)),
        location(cx, store),
        view_switch(cx, store),
        search_box(store),
    )
}

fn segments(runtime: &mut Runtime, store: Store) -> Vec<PathSegment> {
    let home = store.places.with(runtime, |places| places.home.clone());
    store.cwd.with(runtime, |dir| {
        breadcrumbs(dir, home.as_deref())
            .into_iter()
            .map(|crumb| {
                let segment = PathSegment::new(crumb.label);
                match (crumb.is_home, crumb.label.is_empty()) {
                    (true, _) => segment.icon(icons::HOUSE),
                    (false, true) => segment.icon(icons::HARD_DRIVE),
                    (false, false) => segment,
                }
            })
            .collect()
    })
}

/// The path bar, or the path typed after Ctrl+L.
fn location(cx: &mut Cx, store: Store) -> impl View + use<> {
    let initial = segments(cx, store);
    let shown = cx.signal(initial);
    cx.effect(move |runtime| {
        let next = segments(runtime, store);
        if shown.with_untracked(runtime, |current| *current != next) {
            shown.set(runtime, next);
        }
    });
    let crumbs = move || {
        path_bar(shown, move |cx, index| {
            let home = store.places.with(cx, |places| places.home.clone());
            let target = store.cwd.with(cx, |dir| {
                breadcrumbs(dir, home.as_deref())
                    .get(index)
                    .map(|crumb| crumb.path.into())
            });
            if let Some(dir) = target {
                open_dir(cx, store, dir);
            }
        })
    };
    let editor = move || {
        autofocused(
            text_input(store.edit_text)
                .shape(FieldShape::Pill)
                .stretch()
                .label("Location")
                .on_submit(move |cx, _| commit(cx, store)),
        )
        .on_focus_out(move |cx| cancel(cx, store))
        .flex_1()
    };
    show(
        bind(move |runtime: &mut Runtime| store.editing.get(runtime) == Editing::Path),
        editor,
        crumbs,
    )
    .flex_row()
    .items_center()
    .flex_1()
    .min_w(0.0)
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

/// Search: an instant filter of this folder, wider while in use.
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
    .shrink(0.0)
    .transition(Spring::snappy())
}
