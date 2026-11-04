use std::path::Path;

use cairo::{Context, Format, ImageSurface};

use crate::{
    thumbs::{error::DecodeError, frame::Frame},
    util::{
        pixels::{packed_rows, premultiplied_bgra_to_rgba},
        uri::glib_file_uri,
    },
};

const WHITE: f64 = 1.0;
const RGBA_CHANNELS: usize = 4;

/// The first page on white paper, its longest side `edge` pixels.
pub fn decode_file(path: &Path, edge: u32) -> Result<Frame, DecodeError> {
    let document = poppler::Document::from_file(&glib_file_uri(path), None).map_err(pdf_error)?;
    let page = document.page(0).ok_or(DecodeError::Empty)?;
    let (points_wide, points_high) = page.size();
    let longest = points_wide.max(points_high);
    if longest <= 0.0 {
        return Err(DecodeError::Empty);
    }
    let scale = f64::from(edge) / longest;
    let side = |points: f64| ((points * scale).round() as i32).clamp(1, edge as i32);
    let (width, height) = (side(points_wide), side(points_high));
    let mut surface = ImageSurface::create(Format::ARgb32, width, height).map_err(pdf_error)?;
    {
        let context = Context::new(&surface).map_err(pdf_error)?;
        context.set_source_rgb(WHITE, WHITE, WHITE);
        context.paint().map_err(pdf_error)?;
        context.scale(scale, scale);
        page.render(&context);
    }
    surface.flush();
    let stride = usize::try_from(surface.stride()).map_err(|_| DecodeError::Empty)?;
    let (width, height) = (width as u32, height as u32);
    let row_bytes = width as usize * RGBA_CHANNELS;
    let data = surface.data().map_err(pdf_error)?;
    let mut rgba = packed_rows(&data, stride, row_bytes, height as usize);
    premultiplied_bgra_to_rgba(&mut rgba);
    Frame::new(width, height, rgba).ok_or(DecodeError::Empty)
}

fn pdf_error(error: impl std::fmt::Display) -> DecodeError {
    DecodeError::Pdf(error.to_string())
}
