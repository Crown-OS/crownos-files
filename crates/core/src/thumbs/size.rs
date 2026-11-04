/// The freedesktop thumbnail size buckets, each a directory of the cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThumbSize {
    Normal,
    Large,
    XLarge,
    XXLarge,
}

impl ThumbSize {
    pub const ALL: [Self; 4] = [Self::Normal, Self::Large, Self::XLarge, Self::XXLarge];

    /// The longest edge of a thumbnail in this bucket, in pixels.
    pub const fn edge(self) -> u32 {
        match self {
            Self::Normal => 128,
            Self::Large => 256,
            Self::XLarge => 512,
            Self::XXLarge => 1024,
        }
    }

    pub const fn dir_name(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Large => "large",
            Self::XLarge => "x-large",
            Self::XXLarge => "xx-large",
        }
    }

    /// The smallest bucket whose thumbnails are at least `pixels` long, for
    /// a tile of that size at the display's scale.
    pub fn covering(pixels: u32) -> Self {
        Self::ALL
            .into_iter()
            .find(|size| size.edge() >= pixels)
            .unwrap_or(Self::XXLarge)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covering_picks_the_smallest_big_enough_bucket() {
        assert_eq!(ThumbSize::covering(64), ThumbSize::Normal);
        assert_eq!(ThumbSize::covering(129), ThumbSize::Large);
        assert_eq!(ThumbSize::covering(512), ThumbSize::XLarge);
        assert_eq!(ThumbSize::covering(4000), ThumbSize::XXLarge);
    }
}
