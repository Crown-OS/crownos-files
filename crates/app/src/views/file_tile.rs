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
    util::theme::{hover_wash, selected_wash, tint},
    views::{
        entry_facts::EntryFacts,
        entry_state::{is_renaming, is_selected, selection_value},
        file_menu::open_menu,
        glyph::glyph_view,
        metrics::THUMB_WELL,
        rename_field::rename_field,
    },
};

const NAME_SIZE: f32 = 12.0;
const META_SIZE: f32 = 11.0;

/// One cell of the icon grid: the thumbnail well, the name pill and the size.
pub fn file_tile(cx: &mut Cx, store: Store, position: usize) -> impl View + use<> {
    let theme = use_theme(cx);
    let facts = EntryFacts::at(cx, store, position);
    let hovered = cx.signal(false);
    let selected = move |runtime: &mut Runtime| is_selected(runtime, store, position);

    let well = vstack(glyph_view(facts.glyph, 1.0))
        .w(THUMB_WELL.width)
        .h(THUMB_WELL.height)
        .items_center()
        .justify_center()
        .rounded(10.0)
        .bg(bind(move |runtime: &mut Runtime| {
            let selected = selected(runtime);
            let hovered = hovered.get(runtime);
            theme.with(runtime, |theme| match (selected, hovered) {
                (true, _) => selected_wash(theme),
                (false, true) => hover_wash(theme),
                (false, false) => TRANSPARENT,
            })
        }))
        .transition(Spring::crisp());

    let label = facts.name.clone();
    let name = move || {
        text(label.clone())
            .text_size(NAME_SIZE)
            .font_medium()
            .truncate()
            .min_w(0.0)
            .shrink(1.0)
            .px(6.0)
            .py(2.0)
            .rounded(6.0)
            .bg(bind(move |runtime: &mut Runtime| {
                let selected = selected(runtime);
                theme.with(runtime, |theme| {
                    if selected {
                        theme.accent.end
                    } else {
                        TRANSPARENT
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
            .transition(Spring::snappy())
    };
    let title = hstack(
        show(
            bind(move |runtime: &mut Runtime| is_renaming(runtime, store, position)),
            move || rename_field(store),
            name,
        )
        .flex_row()
        .min_w(0.0)
        .shrink(1.0),
    )
    .w_full()
    .min_w(0.0)
    .justify_center();

    let meta = text(facts.size)
        .text_size(META_SIZE)
        .text_color(tint(theme, |theme| theme.text.muted))
        .truncate();

    vstack((well, title, meta))
        .items_center()
        .gap(4.0)
        .py(4.0)
        .px(2.0)
        .rounded(12.0)
        .font_family(INTER_FAMILY)
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
