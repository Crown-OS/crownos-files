use crownui::{
    ext::{DrawList, Path},
    prelude::Point,
};

use super::{paint::stroke_path, palette::badge_color};
use crate::model::badge::BadgeColor;

const WAVE_ORIGIN: Point = Point::new(6.0, 18.0);
const WAVE_STROKE: f32 = 1.4;
const WAVE: [(f32, f32); 15] = [
    (1.0, 11.0),
    (4.0, 11.0),
    (6.0, 5.0),
    (8.0, 17.0),
    (10.0, 8.0),
    (12.0, 14.0),
    (14.0, 4.0),
    (16.0, 18.0),
    (18.0, 10.0),
    (20.0, 14.0),
    (22.0, 7.0),
    (24.0, 16.0),
    (26.0, 11.0),
    (28.0, 13.0),
    (31.0, 13.0),
];

/// Audio: a waveform in the badge color.
pub fn paint_wave(list: &mut DrawList, accent: BadgeColor) {
    let color = badge_color(accent);
    let point = |(x, y): (f32, f32)| Point::new(WAVE_ORIGIN.x + x, WAVE_ORIGIN.y + y);
    let wave = WAVE[1..]
        .iter()
        .fold(Path::new().move_to(point(WAVE[0])), |path, &next| {
            path.line_to(point(next))
        });
    stroke_path(list, wave, WAVE_STROKE, color);
}
