use crownui::{
    ext::{DrawList, Path},
    kit::icons,
    prelude::{CornerRadii, Point, Rect, Size},
};

use super::{
    paint::{fill_path, rect, vertical},
    palette::{FOLDER_BACK, FOLDER_FRONT, FOLDER_RIM, FOLDER_SHEET},
};
use crate::model::special_dir::FolderGlyph;

pub const FOLDER_SIZE: Size = Size::new(66.0, 54.0);
pub const MARK_STROKE: f32 = 2.0;
const MARK_SIDE: f32 = 18.0;
const MARK_TOP: f32 = 25.0;

/// Where a special folder's Lucide mark sits on the front.
pub const MARK_FRAME: Rect = Rect {
    origin: Point::new((FOLDER_SIZE.width - MARK_SIDE) / 2.0, MARK_TOP),
    size: Size::new(MARK_SIDE, MARK_SIDE),
};

pub fn lucide(mark: FolderGlyph) -> &'static str {
    match mark {
        FolderGlyph::Image => icons::IMAGE,
        FolderGlyph::Music => icons::MUSIC,
        FolderGlyph::Video => icons::VIDEO,
        FolderGlyph::Download => icons::DOWNLOAD,
        FolderGlyph::Share => icons::SHARE,
    }
}

/// A folder: a dark back with its tab, a white sheet and a lighter front.
pub fn paint_folder(list: &mut DrawList) {
    let back = Path::new()
        .move_to(Point::new(4.0, 8.5))
        .quad_to(Point::new(4.0, 4.5), Point::new(8.0, 4.5))
        .line_to(Point::new(23.0, 4.5))
        .quad_to(Point::new(24.8, 4.5), Point::new(25.9, 5.7))
        .line_to(Point::new(29.3, 9.3))
        .quad_to(Point::new(30.4, 10.5), Point::new(32.2, 10.5))
        .line_to(Point::new(58.0, 10.5))
        .quad_to(Point::new(62.0, 10.5), Point::new(62.0, 14.5))
        .line_to(Point::new(62.0, 46.0))
        .quad_to(Point::new(62.0, 50.0), Point::new(58.0, 50.0))
        .line_to(Point::new(8.0, 50.0))
        .quad_to(Point::new(4.0, 50.0), Point::new(4.0, 46.0))
        .close();
    fill_path(list, back, vertical(4.5, 50.0, FOLDER_BACK));
    list.fill(
        rect(8.0, 13.0, 50.0, 22.0),
        CornerRadii::uniform(2.0),
        FOLDER_SHEET,
    );
    list.fill(
        rect(4.0, 17.0, 58.0, 33.0),
        CornerRadii::uniform(4.0),
        vertical(17.0, 50.0, FOLDER_FRONT),
    );
    list.fill(rect(8.0, 17.0, 50.0, 1.2), CornerRadii::ZERO, FOLDER_RIM);
}
