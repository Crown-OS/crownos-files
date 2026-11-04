use std::{
    io::{BufRead, Cursor, Seek},
    path::Path,
};

use image::{DynamicImage, ImageDecoder, ImageReader, Limits, metadata::Orientation};

use crate::thumbs::{
    decode::{jpeg, orient::apply_orientation, read_error, resize::shrink_to},
    error::DecodeError,
    frame::Frame,
};

const MAX_ALLOC_BYTES: u64 = 512 * 1024 * 1024;
const MAX_DIMENSION: u32 = 10_000;

pub fn decode_file(path: &Path, edge: u32) -> Result<Frame, DecodeError> {
    let reader = ImageReader::open(path)
        .and_then(ImageReader::with_guessed_format)
        .map_err(read_error(path))?;
    decode_reader(reader, edge)
}

/// An encoded image held in memory, such as embedded cover art.
pub fn decode_bytes(bytes: &[u8], edge: u32) -> Result<Frame, DecodeError> {
    if jpeg::is_jpeg(bytes) {
        return jpeg::decode_bytes(bytes, edge);
    }
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|source| DecodeError::Image(source.into()))?;
    decode_reader(reader, edge)
}

fn decode_reader<R: BufRead + Seek>(
    mut reader: ImageReader<R>,
    edge: u32,
) -> Result<Frame, DecodeError> {
    reader.limits(limits());
    let mut decoder = reader.into_decoder()?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let rgba = DynamicImage::from_decoder(decoder)?.into_rgba8();
    let frame =
        Frame::new(rgba.width(), rgba.height(), rgba.into_raw()).ok_or(DecodeError::Empty)?;
    apply_orientation(shrink_to(frame, edge)?, orientation).ok_or(DecodeError::Empty)
}

fn limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_ALLOC_BYTES);
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits
}
