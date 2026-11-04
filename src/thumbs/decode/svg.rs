use std::path::Path;

use resvg::{tiny_skia, usvg};

use crate::{
    thumbs::{decode::read_error, error::DecodeError, frame::Frame},
    util::pixels::unpremultiply_rgba,
};

/// SVG and SVGZ, rendered so the longest side is exactly `edge`.
pub fn decode_file(path: &Path, edge: u32) -> Result<Frame, DecodeError> {
    let data = std::fs::read(path).map_err(read_error(path))?;
    let options = usvg::Options {
        resources_dir: path.parent().map(Path::to_path_buf),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_data(&data, &options)?;
    let natural = tree.size();
    let scale = edge as f32 / natural.width().max(natural.height());
    let side = |length: f32| ((length * scale).round() as u32).clamp(1, edge);
    let (width, height) = (side(natural.width()), side(natural.height()));
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or(DecodeError::Empty)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let mut rgba = pixmap.take();
    unpremultiply_rgba(&mut rgba);
    Frame::new(width, height, rgba).ok_or(DecodeError::Empty)
}
