const RGBA_CHANNELS: usize = 4;
const OPAQUE: u8 = u8::MAX;

/// The first `row_bytes` of each of `rows` rows spaced `stride` apart, packed tightly.
pub fn packed_rows(data: &[u8], stride: usize, row_bytes: usize, rows: usize) -> Vec<u8> {
    if stride == row_bytes {
        return data.get(..row_bytes * rows).unwrap_or(data).to_vec();
    }
    let mut packed = Vec::with_capacity(row_bytes * rows);
    data.chunks(stride)
        .take(rows)
        .for_each(|row| packed.extend_from_slice(row.get(..row_bytes).unwrap_or(row)));
    packed
}

/// Premultiplied native-endian ARGB32 (cairo's layout) to straight RGBA8, in place.
pub fn premultiplied_bgra_to_rgba(pixels: &mut [u8]) {
    for pixel in pixels.chunks_exact_mut(RGBA_CHANNELS) {
        pixel.swap(0, 2);
    }
    unpremultiply_rgba(pixels);
}

/// Premultiplied RGBA8 (tiny-skia's layout) to straight alpha, in place.
pub fn unpremultiply_rgba(pixels: &mut [u8]) {
    for pixel in pixels.chunks_exact_mut(RGBA_CHANNELS) {
        let alpha = u16::from(pixel[3]);
        if alpha != 0 && alpha != u16::from(OPAQUE) {
            for channel in &mut pixel[..3] {
                *channel = ((u16::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
    }
}

/// 8-bit gray, gray+alpha or RGB samples widened to RGBA8.
pub fn expand_to_rgba(samples: &[u8], channels: usize) -> Vec<u8> {
    match channels {
        1 => samples
            .iter()
            .flat_map(|&gray| [gray, gray, gray, OPAQUE])
            .collect(),
        2 => samples
            .chunks_exact(2)
            .flat_map(|pair| [pair[0], pair[0], pair[0], pair[1]])
            .collect(),
        3 => samples
            .chunks_exact(3)
            .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], OPAQUE])
            .collect(),
        _ => samples.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_drop_their_padding() {
        let data = [1, 2, 0, 3, 4, 0];
        assert_eq!(packed_rows(&data, 3, 2, 2), [1, 2, 3, 4]);
    }

    #[test]
    fn premultiplied_bgra_becomes_straight_rgba() {
        let mut pixels = [0, 0, 128, 128, 30, 20, 10, 255];
        premultiplied_bgra_to_rgba(&mut pixels);
        assert_eq!(pixels, [255, 0, 0, 128, 10, 20, 30, 255]);
    }

    #[test]
    fn gray_and_rgb_widen_to_rgba() {
        assert_eq!(expand_to_rgba(&[7], 1), [7, 7, 7, 255]);
        assert_eq!(expand_to_rgba(&[7, 9], 2), [7, 7, 7, 9]);
        assert_eq!(expand_to_rgba(&[1, 2, 3], 3), [1, 2, 3, 255]);
    }
}
