//! Thumbnail PNGs: 8-bit RGBA with the spec's `tEXt` keys.

use std::{
    fs::File,
    io::{BufReader, Write},
    path::Path,
};

use png::{BitDepth, ColorType, Compression, Decoder, Encoder, Reader, Transformations};

use crate::{
    thumbs::{error::CacheError, frame::Frame},
    util::pixels::expand_to_rgba,
};

pub fn write_png(
    writer: impl Write,
    frame: &Frame,
    text: &[(&str, &str)],
) -> Result<(), CacheError> {
    let mut encoder = Encoder::new(writer, frame.width, frame.height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    encoder.set_compression(Compression::Fast);
    for (keyword, value) in text {
        encoder.add_text_chunk((*keyword).to_owned(), (*value).to_owned())?;
    }
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&frame.rgba)?;
    writer.finish()?;
    Ok(())
}

/// A thumbnail opened far enough to read the text written before its
/// pixels, so a stale one is rejected without being decoded.
pub struct PngThumb {
    reader: Reader<BufReader<File>>,
}

impl PngThumb {
    pub fn open(path: &Path) -> Result<Self, CacheError> {
        let file = File::open(path).map_err(|source| CacheError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let mut decoder = Decoder::new(BufReader::new(file));
        decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);
        Ok(Self {
            reader: decoder.read_info()?,
        })
    }

    pub fn text(&self, keyword: &str) -> Option<&str> {
        self.reader
            .info()
            .uncompressed_latin1_text
            .iter()
            .find(|chunk| chunk.keyword == keyword)
            .map(|chunk| chunk.text.as_str())
    }

    pub fn into_frame(mut self) -> Result<Frame, CacheError> {
        let size = self
            .reader
            .output_buffer_size()
            .ok_or(CacheError::Malformed)?;
        let mut samples = vec![0; size];
        let output = self.reader.next_frame(&mut samples)?;
        samples.truncate(output.buffer_size());
        let channels = output.color_type.samples();
        let rgba = expand_to_rgba(&samples, channels);
        Frame::new(output.width, output.height, rgba).ok_or(CacheError::Malformed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_chunks_and_pixels_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("thumb.png");
        let frame = Frame::new(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 128]).unwrap();
        let file = File::create(&path).unwrap();
        write_png(
            &file,
            &frame,
            &[("Thumb::URI", "file:///a%20b.png"), ("Thumb::MTime", "42")],
        )
        .unwrap();
        let thumb = PngThumb::open(&path).unwrap();
        assert_eq!(thumb.text("Thumb::URI"), Some("file:///a%20b.png"));
        assert_eq!(thumb.text("Thumb::MTime"), Some("42"));
        assert_eq!(thumb.text("Thumb::Size"), None);
        assert_eq!(thumb.into_frame().unwrap(), frame);
    }
}
