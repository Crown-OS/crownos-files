//! "Open “name” with": every application for the file's type, with its icon,
//! and whether to make the choice the default.

use crownui::{
    kit::{
        ButtonOptions, ButtonVariant, Theme, ToggleOptions, button, dialog, icon, icons, toggle,
        use_theme,
    },
    prelude::{
        AnyView, Cx, Listeners, Role, Runtime, Semantic, Signal, Spring, Styled, TRANSPARENT, View,
        bind, component, cover_image, hstack, scroll_view, spacer, text, vstack,
    },
};

use crate::{
    app::opening::{APP_ICON_SIZE, Opening, PromptApp, confirm_choice},
    state::Store,
    util::theme::{hover_wash, tint},
};

const ROW_HEIGHT: f32 = 40.0;
const LIST_HEIGHT: f32 = 260.0;
const ICON_RADIUS: f32 = 6.0;
const FALLBACK_ICON: f32 = 20.0;

pub fn open_with_dialog(store: Store) -> impl View {
    let opening = store.opening;
    dialog(opening.open, move || {
        component(move |cx| {
            let theme = use_theme(cx);
            let Some(prompt) = opening.prompt.with_untracked(cx, Clone::clone) else {
                return vstack(()).boxed();
            };
            for app in &prompt.apps {
                if let Some((image, _)) = app.icon {
                    cx.release_image_on_cleanup(image);
                }
            }
            let rows: Vec<AnyView> = prompt
                .apps
                .into_iter()
                .enumerate()
                .map(|(index, app)| app_row(cx, theme, opening, index, app))
                .collect();
            let empty = rows.is_empty();
            let list = if empty {
                text("No installed application opens this type of file")
                    .text_color(tint(theme, |theme| theme.text.muted))
                    .boxed()
            } else {
                scroll_view(vstack(rows).gap(2.0))
                    .max_h(LIST_HEIGHT)
                    .boxed()
            };
            vstack((
                text(prompt.title).font_semibold().truncate(),
                list,
                toggle(opening.make_default).caption("Always open this type of file with it"),
                hstack((
                    spacer(),
                    button("Cancel")
                        .variant(ButtonVariant::Secondary)
                        .on_click(move |cx, _| opening.open.set(cx, false)),
                    button("Open")
                        .variant(ButtonVariant::Primary)
                        .disabled(bind(move |runtime: &mut Runtime| {
                            opening.chosen.get(runtime).is_none()
                        }))
                        .on_click(move |cx, _| confirm_choice(cx, store)),
                ))
                .gap(8.0),
            ))
            .gap(12.0)
            .boxed()
        })
    })
}

fn app_row(
    cx: &mut Cx,
    theme: Signal<Theme>,
    opening: Opening,
    index: usize,
    app: PromptApp,
) -> AnyView {
    let hovered = cx.signal(false);
    let chosen = move |runtime: &mut Runtime| opening.chosen.get(runtime) == Some(index);
    let picture = match app.icon {
        Some((image, size)) => cover_image(image, size, ICON_RADIUS)
            .size(APP_ICON_SIZE)
            .boxed(),
        None => hstack(icon(icons::APP_WINDOW).size(FALLBACK_ICON))
            .size(APP_ICON_SIZE)
            .items_center()
            .justify_center()
            .boxed(),
    };
    hstack((picture, text(app.name.clone()).truncate().min_w(0.0)))
        .items_center()
        .gap(10.0)
        .h(ROW_HEIGHT)
        .px(8.0)
        .rounded(8.0)
        .bg(bind(move |runtime: &mut Runtime| {
            let chosen = chosen(runtime);
            let hovered = hovered.get(runtime);
            theme.with(runtime, |theme| match (chosen, hovered) {
                (true, _) => theme.accent.end,
                (false, true) => hover_wash(theme),
                (false, false) => TRANSPARENT,
            })
        }))
        .text_color(bind(move |runtime: &mut Runtime| {
            let chosen = chosen(runtime);
            theme.with(runtime, |theme| {
                if chosen {
                    theme.text.on_accent
                } else {
                    theme.text.body
                }
            })
        }))
        .transition(Spring::crisp())
        .role(Role::ListItem)
        .label(app.name)
        .value(bind(move |runtime: &mut Runtime| {
            if chosen(runtime) {
                "selected".to_owned()
            } else {
                String::new()
            }
        }))
        .on_pointer_enter(move |cx, _| hovered.set(cx, true))
        .on_pointer_leave(move |cx, _| hovered.set(cx, false))
        .on_click(move |cx, _| opening.chosen.set(cx, Some(index)))
        .boxed()
}
