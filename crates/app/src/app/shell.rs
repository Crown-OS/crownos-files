use crownui::{
    kit::{INTER_FAMILY, sidebar_with_header, toast_host, use_theme},
    prelude::{Curve, Cx, Listeners, Styled, View, hstack, vstack, zstack},
};

use crate::{
    app::{keys::handle_key, listing::reorder},
    state::Store,
    util::theme::{tint, with_translucency_factor},
    views::{
        details::details, file_area::file_area, file_menu::file_menu, metrics::DETAILS_WIDTH,
        sidebar::sidebar, status_bar::status_bar, toolbar::toolbar,
    },
};

/// The kit's palette cross-fade, so the window backdrop changes with the cards.
const THEME_FADE_MILLIS: u64 = 240;
/// How much of the surface's translucency the backdrop behind the sidebar and
/// toolbar keeps; the content card paints the surface as it is.
const CHROME_TRANSLUCENCY: f32 = 0.5;

/// Everything the window draws over `store`; starts no worker, so tests can
/// drive it with a store filled from fixtures.
pub fn shell(cx: &mut Cx, store: Store) -> impl View + use<> {
    cx.effect(move |runtime| reorder(runtime, store));
    let theme = use_theme(cx);
    let hairline = move || {
        vstack(())
            .shrink(0.0)
            .bg(tint(theme, |theme| theme.surface.hairline))
    };
    let files = hstack((
        file_area(cx, store),
        hairline().w(1.0),
        vstack(details(store)).w(DETAILS_WIDTH).shrink(0.0),
    ))
    .flex_1()
    .min_h(0.0);
    let content = vstack((files, hairline().h(1.0), status_bar(cx, store)))
        .flex_1()
        .min_h(0.0);
    let backdrop = vstack(())
        .w_full()
        .h_full()
        .bg(tint(theme, |theme| {
            with_translucency_factor(theme.surface.bg, CHROME_TRANSLUCENCY)
        }))
        .transition(Curve::ease_in_out(THEME_FADE_MILLIS));
    let window = sidebar_with_header(sidebar(cx, store), toolbar(cx, store), content);
    zstack((
        backdrop,
        toast_host(window).w_full().h_full(),
        file_menu(store),
    ))
    .font_family(INTER_FAMILY)
    .on_key_down(move |cx, event| handle_key(cx, store, event))
}
