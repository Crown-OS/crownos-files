//! JPEG through libjpeg-turbo, scaled in the DCT domain: a 7680×4320 photo
//! decodes straight to 960×540 without the full-size buffer in between.

use std::path::Path;

use image::metadata::Orientation;
use turbojpeg::{Decompressor, Image, PixelFormat, ScalingFactor};

use crate::{
    thumbs::{
        decode::{orient::apply_orientation, read_error, resize::shrink_to},
        error::DecodeError,
        frame::Frame,
    },
    util::jpeg_exif::exif_chunk,
};

const JPEG_MAGIC: [u8; 3] = [0xFF, 0xD8, 0xFF];
const RGBA_CHANNELS: usize = 4;

pub fn decode_file(path: &Path, edge: u32) -> Result<Frame, DecodeError> {
    decode_bytes(&std::fs::read(path).map_err(read_error(path))?, edge)
}

pub fn is_jpeg(bytes: &[u8]) -> bool {
    bytes.starts_with(&JPEG_MAGIC)
}

pub fn decode_bytes(jpeg: &[u8], edge: u32) -> Result<Frame, DecodeError> {
    let mut decompressor = Decompressor::new()?;
    let header = decompressor.read_header(jpeg)?;
    let factor = smallest_covering_factor(header.width.max(header.height), edge as usize);
    decompressor.set_scaling_factor(factor)?;
    let scaled = header.scaled(factor);
    let pitch = scaled.width * RGBA_CHANNELS;
    let mut pixels = vec![0; pitch * scaled.height];
    decompressor.decompress(
        jpeg,
        Image {
            pixels: pixels.as_mut_slice(),
            width: scaled.width,
            pitch,
            height: scaled.height,
            format: PixelFormat::RGBA,
        },
    )?;
    let frame = Frame::new(
        u32::try_from(scaled.width).map_err(|_| DecodeError::Empty)?,
        u32::try_from(scaled.height).map_err(|_| DecodeError::Empty)?,
        pixels,
    )
    .ok_or(DecodeError::Empty)?;
    let orientation = exif_chunk(jpeg)
        .and_then(Orientation::from_exif_chunk)
        .unwrap_or(Orientation::NoTransforms);
    apply_orientation(shrink_to(frame, edge)?, orientation).ok_or(DecodeError::Empty)
}

/// The most aggressive DCT scaling that leaves `longest` at least `edge` long.
fn smallest_covering_factor(longest: usize, edge: usize) -> ScalingFactor {
    Decompressor::supported_scaling_factors()
        .into_iter()
        .filter(|factor| factor.num() <= factor.denom() && factor.scale(longest) >= edge)
        .min_by(|a, b| (a.num() * b.denom()).cmp(&(b.num() * a.denom())))
        .unwrap_or(ScalingFactor::ONE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_down_as_far_as_the_edge_allows() {
        let factor = smallest_covering_factor(7680, 480);
        assert_eq!((factor.num(), factor.denom()), (1, 8));
        let factor = smallest_covering_factor(400, 480);
        assert_eq!(factor.num(), factor.denom());
    }
}
