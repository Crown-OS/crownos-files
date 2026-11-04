use image::{DynamicImage, RgbaImage, metadata::Orientation};

use crate::thumbs::frame::Frame;

/// `frame` turned upright as its Exif orientation asks.
pub fn apply_orientation(frame: Frame, orientation: Orientation) -> Option<Frame> {
    if orientation == Orientation::NoTransforms {
        return Some(frame);
    }
    let mut image =
        DynamicImage::ImageRgba8(RgbaImage::from_raw(frame.width, frame.height, frame.rgba)?);
    image.apply_orientation(orientation);
    let upright = image.into_rgba8();
    Frame::new(upright.width(), upright.height(), upright.into_raw())
}
