//! The file area's geometry, shared by the lazy views, the rubber band and
//! the keyboard's page size.

use crownconfig::ViewMode;
use crownui::{ext::ItemLayout, prelude::Size};

pub const TILE: Size = Size::new(106.0, 132.0);
pub const TILE_GAP: (f32, f32) = (6.0, 2.0);
pub const GRID_PADDING: f32 = 12.0;
pub const THUMB_WELL: Size = Size::new(90.0, 76.0);
pub const TILE_GLYPH_SCALE: f32 = 1.18;
pub const ROW_HEIGHT: f32 = 36.0;
pub const LIST_PADDING: f32 = 6.0;
pub const LIST_ICON: f32 = 28.0;
pub const LIST_GLYPH_SCALE: f32 = 0.4;
/// The kit sidebar layout's menu width and window inset, for the Details
/// column that mirrors it on the right.
pub const SIDEBAR_WIDTH: f32 = 260.0;
pub const LAYOUT_PADDING: f32 = 6.0;
pub const SIDEBAR_PADDING: f32 = 12.0;
pub const STATUS_HEIGHT: f32 = 40.0;

pub fn layout_for(mode: ViewMode) -> ItemLayout {
    match mode {
        ViewMode::Grid => ItemLayout::cells(TILE, TILE_GAP).with_padding(GRID_PADDING),
        ViewMode::List => ItemLayout::rows(ROW_HEIGHT).with_padding(LIST_PADDING),
    }
}
