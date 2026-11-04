//! Colors that follow the kit's theme as it changes.

use crownui::{
    kit::Theme,
    prelude::{Color, IntoProp, Runtime, Signal, bind},
};

/// A color property bound to one slot of `theme`.
pub fn tint(
    theme: Signal<Theme>,
    pick: impl Fn(&Theme) -> Color + 'static,
) -> impl IntoProp<Color> {
    bind(move |runtime: &mut Runtime| theme.with(runtime, &pick))
}

/// `color` with its translucency, `1 - alpha`, multiplied by `factor`.
pub fn with_translucency_factor(color: Color, factor: f32) -> Color {
    Color {
        a: 1.0 - (1.0 - color.a) * factor,
        ..color
    }
}

/// The wash behind a hovered tile, row or crumb.
pub fn hover_wash(theme: &Theme) -> Color {
    theme.surface.hover
}

/// The stronger wash behind a selected thumbnail.
pub fn selected_wash(theme: &Theme) -> Color {
    theme.surface.hover.with_alpha_factor(SELECTED_WASH_FACTOR)
}

/// The faint stripe behind every other list row.
pub fn stripe_wash(theme: &Theme) -> Color {
    theme.surface.hover.with_alpha_factor(STRIPE_WASH_FACTOR)
}

const SELECTED_WASH_FACTOR: f32 = 1.6;
const STRIPE_WASH_FACTOR: f32 = 0.35;
