//! The literal content colors of the file glyphs, from the landing-page mockup.
//! Badge colors live in `model::badge`; everything else on screen is themed.

use crownui::prelude::{Color, rgb, rgba};

use crate::model::badge::BadgeColor;

pub const FOLDER_BACK: (Color, Color) = (rgb(0x5a5670), rgb(0x3b384b));
pub const FOLDER_FRONT: (Color, Color) = (rgb(0xa29dbd), rgb(0x6c6886));
pub const FOLDER_SHEET: Color = rgba(0xececf1eb);
pub const FOLDER_RIM: Color = rgba(0xffffff73);
pub const FOLDER_INK: Color = rgba(0x19162880);

pub const PAGE: (Color, Color) = (rgb(0xfbfbfc), rgb(0xe6e6ea));
pub const PAGE_FOLD: Color = rgb(0xc8c8d0);
pub const PAGE_EDGE: Color = rgba(0x00000014);
pub const PAGE_LINE: Color = rgba(0xb9b9c2cc);

pub const CODE_PAGE: (Color, Color) = (rgb(0x2a2a30), rgb(0x1c1c21));
pub const CODE_FOLD: Color = rgb(0x45454d);
pub const CODE_EDGE: Color = rgba(0xffffff1a);
pub const CODE_LINES: [(Color, f32); 6] = [
    (rgb(0xc792ea), 0.55),
    (rgb(0x8fd0ff), 0.80),
    (rgb(0xd4d4d8), 0.40),
    (rgb(0xc3e88d), 0.70),
    (rgb(0xd4d4d8), 0.60),
    (rgb(0xf78c6c), 0.35),
];

pub const SWATCHES: [Color; 4] = [rgb(0x8f6dfb), rgb(0x1fb0e6), rgb(0xeb9d4c), rgb(0xe2577a)];
pub const SHEET_RULE: Color = rgb(0xcfd3d6);

pub const PHOTO: (Color, Color) = (rgb(0x1b1633), rgb(0x0b0b14));
pub const PHOTO_GLOWS: [Color; 2] = [rgb(0x8f6dfb), rgb(0x1fb0e6)];
pub const PHOTO_RIM: Color = rgba(0xffffff24);
pub const PLAY: Color = rgba(0xffffffe6);

pub const DISC: [Color; 4] = [rgb(0xc9c9d2), rgb(0x8f8fa0), rgb(0xe2e2ea), rgb(0xa39fc4)];
pub const DISC_RING: Color = rgb(0xcfd0d6);
pub const DISC_HOLE: Color = rgb(0x1a1a1d);
pub const DISC_RIM: Color = rgba(0xffffff4d);

pub const ARCHIVE: (Color, Color) = (rgb(0x7d7a8f), rgb(0x57546a));
pub const ZIPPER: (Color, Color) = (rgb(0xd4d4d8), rgb(0x3a3848));
pub const ZIPPER_PULL: Color = rgb(0xe4e4e8);
pub const ARCHIVE_HIGHLIGHT: Color = rgba(0xffffff40);

pub const DROP_SHADOW: Color = rgba(0x000000cc);
pub const BADGE_TEXT: Color = rgb(0xffffff);

pub fn badge_color(color: BadgeColor) -> Color {
    Color::from_rgba8(color.red, color.green, color.blue, u8::MAX)
}
