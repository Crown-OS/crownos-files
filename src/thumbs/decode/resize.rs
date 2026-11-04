use fast_image_resize::{
    FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer,
    images::{Image, ImageRef},
};

use crate::thumbs::{error::DecodeError, frame::Frame};

/// `width × height` scaled to fit in an `edge` square, keeping the aspect
/// ratio and never enlarging.
pub fn fit_within(width: u32, height: u32, edge: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest <= edge {
        return (width, height);
    }
    let scale = f64::from(edge) / f64::from(longest);
    let scaled = |side: u32| ((f64::from(side) * scale).round() as u32).max(1);
    (scaled(width), scaled(height))
}

/// `frame` shrunk with SIMD convolution to fit in an `edge` square.
pub fn shrink_to(frame: Frame, edge: u32) -> Result<Frame, DecodeError> {
    let (width, height) = fit_within(frame.width, frame.height, edge);
    if (width, height) == (frame.width, frame.height) {
        return Ok(frame);
    }
    let source = ImageRef::new(frame.width, frame.height, &frame.rgba, PixelType::U8x4)
        .map_err(|_| DecodeError::Empty)?;
    let mut target = Image::new(width, height, PixelType::U8x4);
    let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Bilinear));
    Resizer::new().resize(&source, &mut target, &options)?;
    Frame::new(width, height, target.into_vec()).ok_or(DecodeError::Empty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fitting_keeps_the_aspect_ratio_and_never_enlarges() {
        assert_eq!(fit_within(4000, 3000, 256), (256, 192));
        assert_eq!(fit_within(3000, 4000, 256), (192, 256));
        assert_eq!(fit_within(100, 50, 256), (100, 50));
        assert_eq!(fit_within(10_000, 1, 128), (128, 1));
    }

    #[test]
    fn shrinking_averages_pixels() {
        let frame = Frame::new(4, 2, [[200, 100, 50, 255]; 8].concat()).unwrap();
        let small = shrink_to(frame, 2).unwrap();
        assert_eq!((small.width, small.height), (2, 1));
        assert_eq!(&small.rgba[..4], &[200, 100, 50, 255]);
    }
}
