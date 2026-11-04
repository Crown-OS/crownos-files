const MARKER: u8 = 0xFF;
const START_OF_IMAGE: u8 = 0xD8;
const START_OF_SCAN: u8 = 0xDA;
const APP1: u8 = 0xE1;
const EXIF_SIGNATURE: &[u8] = b"Exif\0\0";

/// The TIFF-structured Exif payload of a JPEG's APP1 segment, read from the
/// headers only.
pub fn exif_chunk(jpeg: &[u8]) -> Option<&[u8]> {
    let mut rest = jpeg.strip_prefix(&[MARKER, START_OF_IMAGE])?;
    loop {
        let [MARKER, marker, high, low, ..] = *rest else {
            return None;
        };
        if marker == START_OF_SCAN {
            return None;
        }
        let length = usize::from(u16::from_be_bytes([high, low]));
        let segment = rest.get(4..2 + length)?;
        if marker == APP1
            && let Some(exif) = segment.strip_prefix(EXIF_SIGNATURE)
        {
            return Some(exif);
        }
        rest = rest.get(2 + length..)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_exif_segment_after_other_headers() {
        let jpeg = [
            &[0xFF, 0xD8][..],
            &[0xFF, 0xE0, 0x00, 0x04, 1, 2],
            &[0xFF, 0xE1, 0x00, 0x0C],
            b"Exif\0\0MM\0*",
            &[0xFF, 0xDA],
        ]
        .concat();
        assert_eq!(exif_chunk(&jpeg), Some(&b"MM\0*"[..]));
    }

    #[test]
    fn stops_at_the_scan_and_rejects_truncation() {
        assert_eq!(exif_chunk(&[0xFF, 0xD8, 0xFF, 0xDA, 0, 2]), None);
        assert_eq!(exif_chunk(&[0xFF, 0xD8, 0xFF, 0xE1, 0x00, 0x40]), None);
        assert_eq!(exif_chunk(b"not a jpeg"), None);
    }
}
