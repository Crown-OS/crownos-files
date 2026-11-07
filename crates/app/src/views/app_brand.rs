//! The sidebar's top row: the app icon in the theme's accent and the app
//! name, laid out as the kit's `sidebar_brand` lays out its glyph and label.

use crownui::{
    kit::{GradientStops, INTER_FAMILY, TEXT_SIZE, icon, icons, use_theme},
    prelude::{AnyView, Cx, Runtime, Size, Styled, View, cover_image, hstack, keyed, text},
};

use crate::{
    state::Store,
    util::{app_icon::render_app_icon, theme::tint},
};

const ICON_SIZE: f32 = 22.0;
const ROW_HEIGHT: f32 = 44.0;
const ROW_PADDING: f32 = 10.0;
const ICON_GAP: f32 = 10.0;
const LABEL_WEIGHT: u16 = 500;

pub fn app_brand(cx: &mut Cx, store: Store) -> impl View + use<> {
    let theme = use_theme(cx);
    hstack((
        keyed(
            move |runtime: &mut Runtime| {
                (
                    store.scale.get(runtime),
                    theme.with(runtime, |theme| theme.accent),
                )
            },
            app_icon,
        ),
        text("Files")
            .truncate()
            .grow(1.0)
            .shrink(1.0)
            .min_w(0.0)
            .text_size(TEXT_SIZE + 1.0)
            .font_family(INTER_FAMILY)
            .font_weight(LABEL_WEIGHT)
            .text_color(tint(theme, |theme| theme.sidebar.brand_text)),
    ))
    .items_center()
    .gap(ICON_GAP)
    .h(ROW_HEIGHT)
    .px(ROW_PADDING)
    .shrink(0.0)
}

/// The app icon for `scale` device pixels per logical one, or the kit's
/// folder glyph should the artwork fail to render.
fn app_icon(cx: &mut Cx, (scale, accent): (f32, GradientStops)) -> AnyView {
    let Some(rgba) = render_app_icon(accent, ICON_SIZE, scale) else {
        return icon(icons::FOLDER).size(ICON_SIZE).boxed();
    };
    let edge = (ICON_SIZE * scale).round().max(1.0) as u32;
    let image = cx.register_image(edge, edge, rgba);
    cx.release_image_on_cleanup(image);
    cover_image(image, Size::new(ICON_SIZE, ICON_SIZE), 0.0)
        .size(ICON_SIZE)
        .boxed()
}
