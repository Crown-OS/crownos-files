use crownui::{
    kit::{INTER_FAMILY, use_theme},
    prelude::{
        Cx, Listeners, Role, Runtime, Semantic, Spring, Styled, TRANSPARENT, View, bind, hstack,
        show, text, vstack,
    },
};

use crate::{
    app::{
        actions::open_position,
        selection::{click, ensure_selected},
    },
    state::Store,
    util::theme::{hover_wash, stripe_wash},
    views::{
        entry_facts::EntryFacts,
        entry_state::{is_renaming, is_selected, selection_value},
        file_menu::open_menu,
        glyph::glyph_view,
        metrics::{LIST_ICON, ROW_HEIGHT},
        rename_field::rename_field,
    },
};

const GLYPH_SCALE: f32 = 0.32;
const KIND_WIDTH: f32 = 96.0;
const DATE_WIDTH: f32 = 96.0;
const SIZE_WIDTH: f32 = 60.0;
const NAME_SIZE: f32 = 12.0;
const DETAIL_SIZE: f32 = 11.0;

/// One row of the list view: glyph, name, kind, date and size.
pub fn list_row(cx: &mut Cx, store: Store, position: usize) -> impl View + use<> {
    let theme = use_theme(cx);
    let facts = EntryFacts::at(cx, store, position);
    let hovered = cx.signal(false);
    let striped = position % 2 == 1;
    let selected = move |runtime: &mut Runtime| is_selected(runtime, store, position);
    let detail_color = move |runtime: &mut Runtime| {
        let selected = selected(runtime);
        theme.with(runtime, |theme| {
            if selected {
                theme.text.on_accent
            } else {
                theme.text.muted
            }
        })
    };
    let detail = move |content: String, width: f32| {
        text(content)
            .w(width)
            .shrink(0.0)
            .text_size(DETAIL_SIZE)
            .text_color(bind(detail_color))
            .truncate()
    };

    let icon = vstack(glyph_view(facts.glyph, GLYPH_SCALE))
        .w(LIST_ICON)
        .h(LIST_ICON)
        .shrink(0.0)
        .items_center()
        .justify_center();
    let label = facts.name.clone();
    let name = hstack(
        show(
            bind(move |runtime: &mut Runtime| is_renaming(runtime, store, position)),
            move || rename_field(store),
            move || {
                text(label.clone())
                    .text_size(NAME_SIZE)
                    .font_medium()
                    .truncate()
                    .min_w(0.0)
            },
        )
        .flex_row()
        .flex_1()
        .min_w(0.0),
    )
    .flex_1()
    .min_w(0.0);

    hstack((
        icon,
        name,
        detail(facts.kind, KIND_WIDTH),
        detail(facts.modified, DATE_WIDTH),
        detail(facts.size, SIZE_WIDTH).text_right(),
    ))
    .items_center()
    .gap(10.0)
    .px(8.0)
    .h(ROW_HEIGHT)
    .rounded(8.0)
    .font_family(INTER_FAMILY)
    .bg(bind(move |runtime: &mut Runtime| {
        let selected = selected(runtime);
        let hovered = hovered.get(runtime);
        theme.with(runtime, |theme| match (selected, hovered, striped) {
            (true, _, _) => theme.accent.end,
            (false, true, _) => hover_wash(theme),
            (false, false, true) => stripe_wash(theme),
            (false, false, false) => TRANSPARENT,
        })
    }))
    .text_color(bind(move |runtime: &mut Runtime| {
        let selected = selected(runtime);
        theme.with(runtime, |theme| {
            if selected {
                theme.text.on_accent
            } else {
                theme.text.body
            }
        })
    }))
    .transition(Spring::crisp())
    .role(Role::ListItem)
    .label(facts.name)
    .value(bind(move |runtime: &mut Runtime| {
        selection_value(runtime, store, position)
    }))
    .on_pointer_enter(move |cx, _| hovered.set(cx, true))
    .on_pointer_leave(move |cx, _| hovered.set(cx, false))
    .on_click(move |cx, event| {
        event.stop_propagation();
        click(cx, store, position, event.modifiers);
    })
    .on_double_click(move |cx, event| {
        event.stop_propagation();
        open_position(cx, store, position);
    })
    .on_context_menu(move |cx, event| {
        event.stop_propagation();
        ensure_selected(cx, store, position);
        open_menu(cx, store);
    })
}
