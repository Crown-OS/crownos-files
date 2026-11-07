//! The Files app icon, its gradients painted from the theme's accent.

use crownui::{
    kit::GradientStops,
    prelude::{Color, Size, WHITE},
};

use crate::util::{draw_svg::hex, svg_raster::rasterize};

/// The installed icon; its gradient stops are the slots the accent fills.
const APP_ICON_SVG: &str = include_str!("../../../../data/crownos-files.svg");
const APP_ICON_ARTBOARD: f32 = 1024.0;

/// The installed icon's stop colors, in the order [`accent_stops`] fills them.
const STOP_SLOTS: [&str; 5] = ["#5cc4fb", "#3a8af6", "#84cefc", "#5aa9f9", "#3171f5"];

/// The back panel from the top, then the front pocket from its rim down.
fn accent_stops(accent: GradientStops) -> [Color; 5] {
    let GradientStops { start, end } = accent;
    [
        start.lerp(WHITE, 0.08),
        start.lerp(end, 0.45),
        start.lerp(WHITE, 0.3),
        start,
        start.lerp(end, 0.8),
    ]
}

/// The icon as an SVG document in `accent`.
pub fn app_icon_svg(accent: GradientStops) -> String {
    STOP_SLOTS
        .iter()
        .zip(accent_stops(accent))
        .fold(APP_ICON_SVG.to_owned(), |svg, (slot, color)| {
            svg.replace(slot, &hex(color).0)
        })
}

/// The icon in `accent` at `edge` logical pixels, painted at `scale`.
pub fn render_app_icon(accent: GradientStops, edge: f32, scale: f32) -> Option<Vec<u8>> {
    let artwork = format!(
        r#"<g transform="scale({})">{}</g>"#,
        edge / APP_ICON_ARTBOARD,
        app_icon_svg(accent)
    );
    rasterize(&artwork, Size::new(edge, edge), scale)
}

#[cfg(test)]
mod tests {
    use crownui::prelude::rgb;

    use super::*;

    const ORANGE: GradientStops = GradientStops {
        start: rgb(0xfb923c),
        end: rgb(0xea580c),
    };

    #[test]
    fn every_stop_slot_names_exactly_one_stop_of_the_installed_icon() {
        for slot in STOP_SLOTS {
            assert_eq!(APP_ICON_SVG.matches(slot).count(), 1, "{slot}");
        }
    }

    #[test]
    fn the_accent_replaces_every_slot() {
        let svg = app_icon_svg(ORANGE);
        assert!(STOP_SLOTS.iter().all(|slot| !svg.contains(slot)));
        assert!(svg.contains(&hex(ORANGE.start).0));
    }

    #[test]
    fn the_pocket_takes_the_accent_and_the_corners_stay_clear() {
        let Some(rgba) = render_app_icon(ORANGE, 64.0, 1.0) else {
            panic!("the icon parses");
        };
        let bottom_center = (56 * 64 + 32) * 4;
        let [red, green, blue, alpha] = rgba[bottom_center..bottom_center + 4] else {
            panic!("the pixel is in bounds");
        };
        assert_eq!(alpha, 255);
        assert!(red > green && green > blue);
        assert_eq!(rgba[3], 0);
    }
}
