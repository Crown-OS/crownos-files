//! Canvas painters for the file-type glyphs of the mockup, one per file.

mod archive;
mod audio_wave;
mod choice;
mod code_page;
mod disc;
mod folder;
mod page;
mod paint;
pub mod palette;
mod photo;
mod picture;
mod sheet;
mod video;
mod view;

pub use choice::Glyph;
pub use folder::{MARK_FRAME, MARK_STROKE};
pub use page::{BADGE_BOTTOM, BADGE_PADDING, BADGE_RADIUS, BADGE_TEXT_SIZE};
pub use photo::PHOTO_SIZE;
pub use picture::render_drag_glyph;
pub use video::play_mark;
pub use view::{GlyphArt, glyph_view, paint_glyph};
