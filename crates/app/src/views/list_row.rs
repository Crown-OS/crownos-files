use crownui::{
    kit::{INTER_FAMILY, use_theme},
    prelude::{
        AnyView, Cx, Listeners, Role, Runtime, Semantic, Spring, Styled, TRANSPARENT, View, bind,
        hstack, show, text, vstack,
    },
};

use crate::{
    app::{
        actions::open_position,
        selection::{click, ensure_selected},
    },
    state::Store,
    util::theme::{drop_wash, hover_wash, stripe_wash},
    views::{
        drop_target::folder_drop_target,
        entry_facts::EntryFacts,
        entry_picture::entry_picture,
        entry_state::{is_renaming, is_selected, selection_value},
        file_menu::open_menu,
        metrics::{LIST_GLYPH_SCALE, LIST_ICON, ROW_HEIGHT},
        rename_field::rename_field,
    },
};

const KIND_WIDTH: f32 = 110.0;
const DATE_WIDTH: f32 = 110.0;
const SIZE_WIDTH: f32 = 68.0;
const NAME_SIZE: f32 = 14.0;
const DETAIL_SIZE: f32 = 13.0;

/// One row of the list view: glyph, name, kind, date and size.
pub fn list_row(cx: &mut Cx, store: Store, position: usize) -> AnyView {
    let theme = use_theme(cx);
    let facts = EntryFacts::at(cx, store, position);
    let hovered = cx.signal(false);
    let drop_hovered = cx.signal(false);
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

    let icon = vstack(entry_picture(
        store,
        facts.glyph,
        facts.thumb.clone(),
        LIST_GLYPH_SCALE,
    ))
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

    let folder = facts.folder.clone();
    let row = hstack((
        icon,
        name,
        detail(facts.kind, KIND_WIDTH),
        detail(facts.modified, DATE_WIDTH),
        detail(facts.size, SIZE_WIDTH).text_right(),
    ))
    .items_center()
    .gap(12.0)
    .px(14.0)
    .h(ROW_HEIGHT)
    .rounded(ROW_HEIGHT / 2.0)
    .font_family(INTER_FAMILY)
    .bg(bind(move |runtime: &mut Runtime| {
        let selected = selected(runtime);
        let hovered = hovered.get(runtime);
        let dropping = drop_hovered.get(runtime);
        theme.with(runtime, |theme| {
            match (dropping, selected, hovered, striped) {
                (true, ..) => drop_wash(theme),
                (false, true, _, _) => theme.accent.end,
                (false, false, true, _) => hover_wash(theme),
                (false, false, false, true) => stripe_wash(theme),
                (false, false, false, false) => TRANSPARENT,
            }
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
    });
    match folder {
        Some(dir) => folder_drop_target(row, store, dir, drop_hovered).boxed(),
        None => row.boxed(),
    }
}
