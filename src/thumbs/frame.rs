const RGBA_CHANNELS: usize = 4;

/// Straight-alpha RGBA8 pixels, rows packed tightly, ready for
/// `cx.register_image`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Frame {
    /// `None` unless `rgba` holds exactly `width × height` pixels.
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Option<Self> {
        let expected = usize::try_from(width)
            .ok()?
            .checked_mul(usize::try_from(height).ok()?)?
            .checked_mul(RGBA_CHANNELS)?;
        (width > 0 && height > 0 && rgba.len() == expected).then_some(Self {
            width,
            height,
            rgba,
        })
    }

    pub fn longest_edge(&self) -> u32 {
        self.width.max(self.height)
    }
}
