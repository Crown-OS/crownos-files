//! Canvas painters for the file-type glyphs of the mockup, one per file.

mod archive;
mod audio_wave;
mod choice;
mod code_page;
mod disc;
mod folder;
mod page;
mod paint;
mod palette;
mod photo;
mod sheet;
mod video;
mod view;

pub use choice::Glyph;
pub use view::glyph_view;
