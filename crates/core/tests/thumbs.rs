#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{
    fs::{self, File, FileTimes},
    path::{Path, PathBuf},
    time::{Duration, UNIX_EPOCH},
};

use crownos_files_core::{
    model::timestamp::Timestamp,
    thumbs::{
        CachedDecoder, DecodeError, ThumbCache, ThumbError, ThumbKey, ThumbSize, ThumbnailSource,
        decode,
    },
};
use tokio_util::sync::CancellationToken;
use turbojpeg::{Compressor, Image, PixelFormat};

const RED: [u8; 3] = [255, 0, 0];
const BLUE: [u8; 3] = [0, 0, 255];
const EXIF_ROTATE_90: u16 = 6;

/// A `width × height` RGB image, the left half red and the right half blue.
fn halves(width: usize, height: usize) -> Vec<u8> {
    (0..width * height)
        .flat_map(|index| if index % width < width / 2 { RED } else { BLUE })
        .collect()
}

fn jpeg(width: usize, height: usize, orientation: Option<u16>) -> Vec<u8> {
    let pixels = halves(width, height);
    let mut compressor = Compressor::new().unwrap();
    compressor.set_quality(95).unwrap();
    let encoded = compressor
        .compress_to_vec(Image {
            pixels: pixels.as_slice(),
            width,
            pitch: width * 3,
            height,
            format: PixelFormat::RGB,
        })
        .unwrap();
    let Some(orientation) = orientation else {
        return encoded;
    };
    let mut tiff = b"MM\0*\0\0\0\x08\0\x01\x01\x12\0\x03\0\0\0\x01".to_vec();
    tiff.extend_from_slice(&orientation.to_be_bytes());
    tiff.extend_from_slice(&[0; 6]);
    let payload = [&b"Exif\0\0"[..], &tiff].concat();
    let length = u16::try_from(payload.len() + 2).unwrap().to_be_bytes();
    [
        &encoded[..2],
        &[0xFF, 0xE1],
        &length,
        &payload,
        &encoded[2..],
    ]
    .concat()
}

fn pixel(rgba: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * width + x) * 4) as usize;
    rgba[at..at + 4].try_into().unwrap()
}

fn is_reddish([red, green, blue, _]: [u8; 4]) -> bool {
    red > 200 && green < 60 && blue < 60
}

fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn jpeg_thumbnails_are_scaled_in_the_dct_and_turned_upright() {
    let dir = tempfile::tempdir().unwrap();
    let plain = write(dir.path(), "plain.jpg", &jpeg(640, 320, None));
    let thumb = decode(&plain, 128).unwrap().frame;
    assert_eq!((thumb.width, thumb.height), (128, 64));
    assert!(is_reddish(pixel(&thumb.rgba, thumb.width, 4, 32)));

    let rotated = write(
        dir.path(),
        "rotated.jpeg",
        &jpeg(640, 320, Some(EXIF_ROTATE_90)),
    );
    let thumb = decode(&rotated, 128).unwrap().frame;
    assert_eq!((thumb.width, thumb.height), (64, 128));
    assert!(is_reddish(pixel(&thumb.rgba, thumb.width, 32, 4)));
}

#[test]
fn png_thumbnails_keep_the_aspect_ratio_and_small_ones_are_not_enlarged() {
    let dir = tempfile::tempdir().unwrap();
    let large = dir.path().join("wide.png");
    let rgb = halves(300, 150);
    image::save_buffer(&large, &rgb, 300, 150, image::ExtendedColorType::Rgb8).unwrap();
    let thumb = decode(&large, 128).unwrap().frame;
    assert_eq!((thumb.width, thumb.height), (128, 64));
    assert_eq!(thumb.rgba.len(), 128 * 64 * 4);
    assert!(is_reddish(pixel(&thumb.rgba, thumb.width, 2, 30)));

    let small = dir.path().join("icon.png");
    image::save_buffer(
        &small,
        &halves(20, 10),
        20,
        10,
        image::ExtendedColorType::Rgb8,
    )
    .unwrap();
    let thumb = decode(&small, 128).unwrap().frame;
    assert_eq!((thumb.width, thumb.height), (20, 10));
}

#[test]
fn an_extensionless_image_is_recognised_by_content() {
    let dir = tempfile::tempdir().unwrap();
    let encoded = dir.path().join("encoded.png");
    image::save_buffer(
        &encoded,
        &halves(40, 20),
        40,
        20,
        image::ExtendedColorType::Rgb8,
    )
    .unwrap();
    let path = dir.path().join("no_extension");
    fs::rename(encoded, &path).unwrap();
    let thumb = decode(&path, 128).unwrap().frame;
    assert_eq!((thumb.width, thumb.height), (40, 20));
}

#[test]
fn svg_thumbnails_fill_the_edge() {
    let dir = tempfile::tempdir().unwrap();
    let svg = write(
        dir.path(),
        "tall.svg",
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="50" height="100" viewBox="0 0 50 100"><rect width="50" height="100" fill="#00ff00"/></svg>"##,
    );
    let thumb = decode(&svg, 256).unwrap().frame;
    assert_eq!((thumb.width, thumb.height), (128, 256));
    assert_eq!(pixel(&thumb.rgba, thumb.width, 64, 128), [0, 255, 0, 255]);
}

#[test]
fn unknown_and_broken_files_are_errors() {
    let dir = tempfile::tempdir().unwrap();
    let text = write(dir.path(), "notes.txt", b"hello");
    assert!(matches!(decode(&text, 128), Err(DecodeError::Unsupported)));
    let broken = write(dir.path(), "broken.png", b"\x89PNG\r\n\x1a\nnot really");
    assert!(decode(&broken, 128).is_err());
}

#[cfg(feature = "thumbs-pdf")]
#[test]
fn pdf_thumbnails_render_the_first_page_on_white() {
    let content = b"1 0 0 rg 0 0 100 100 re f\n";
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R >>".to_owned(),
        format!(
            "<< /Length {} >>\nstream\n{}endstream",
            content.len(),
            String::from_utf8_lossy(content)
        ),
    ];
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "page.pdf", &pdf);
    let thumb = decode(&path, 128).unwrap().frame;
    assert_eq!((thumb.width, thumb.height), (128, 64));
    assert!(is_reddish(pixel(&thumb.rgba, thumb.width, 10, 32)));
    assert_eq!(
        pixel(&thumb.rgba, thumb.width, 120, 32),
        [255, 255, 255, 255]
    );
}

#[cfg(feature = "thumbs-video")]
mod media {
    use gstreamer::{self as gst, prelude::*};

    use super::*;

    /// Runs a `gst-launch` style pipeline to its end; `false` when an element is missing.
    fn render(description: &str) -> bool {
        if gst::init().is_err() {
            return false;
        }
        let Ok(pipeline) = gst::parse::launch(description) else {
            return false;
        };
        if pipeline.set_state(gst::State::Playing).is_err() {
            return false;
        }
        let bus = pipeline.bus().unwrap();
        let finished = bus
            .timed_pop_filtered(
                gst::ClockTime::from_seconds(30),
                &[gst::MessageType::Eos, gst::MessageType::Error],
            )
            .is_some_and(|message| matches!(message.view(), gst::MessageView::Eos(_)));
        pipeline.set_state(gst::State::Null).unwrap();
        finished
    }

    #[test]
    fn thumbnail_uris_are_spelled_like_glib_spells_them() {
        let path = Path::new("/tmp/a b(1)!$&'*+,;=:@[]{}#?%~^`\u{e9}\u{4e2d}.png");
        assert_eq!(
            crownos_files_core::util::uri::glib_file_uri(path),
            gst::glib::filename_to_uri(path, None).unwrap().as_str()
        );
    }

    #[test]
    fn video_thumbnails_are_a_scaled_frame_with_the_duration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clip.webm");
        let made = render(&format!(
            "videotestsrc num-buffers=60 pattern=red ! video/x-raw,width=640,height=360,framerate=30/1 ! videoconvert ! vp8enc deadline=1 ! webmmux ! filesink location={}",
            path.display()
        ));
        if !made {
            eprintln!("skipped: GStreamer cannot encode VP8/WebM here");
            return;
        }
        let decoded = decode(&path, 128).unwrap();
        assert_eq!((decoded.frame.width, decoded.frame.height), (128, 72));
        assert!(is_reddish(pixel(&decoded.frame.rgba, 128, 64, 36)));
        let duration = decoded.duration.unwrap();
        assert!(duration > Duration::from_millis(1500) && duration < Duration::from_millis(2500));
    }

    #[test]
    fn audio_thumbnails_are_the_embedded_cover() {
        let dir = tempfile::tempdir().unwrap();
        let raw = dir.path().join("tone.raw.mp3");
        if !render(&format!(
            "audiotestsrc num-buffers=40 ! audioconvert ! lamemp3enc ! filesink location={}",
            raw.display()
        )) {
            eprintln!("skipped: GStreamer cannot encode MP3 here");
            return;
        }
        let cover = dir.path().join("cover.png");
        image::save_buffer(
            &cover,
            &halves(60, 60),
            60,
            60,
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();
        let cover = fs::read(cover).unwrap();
        let mut frame = b"\0image/png\0\x03\0".to_vec();
        frame.extend_from_slice(&cover);
        let mut tag = b"APIC".to_vec();
        tag.extend_from_slice(&u32::try_from(frame.len()).unwrap().to_be_bytes());
        tag.extend_from_slice(&[0, 0]);
        tag.extend_from_slice(&frame);
        let size = u32::try_from(tag.len()).unwrap();
        let syncsafe = [
            (size >> 21) & 0x7F,
            (size >> 14) & 0x7F,
            (size >> 7) & 0x7F,
            size & 0x7F,
        ]
        .map(|byte| byte as u8);
        let song = [
            &b"ID3\x03\0\0"[..],
            &syncsafe,
            &tag,
            &fs::read(&raw).unwrap(),
        ]
        .concat();
        let path = write(dir.path(), "song.mp3", &song);
        let decoded = decode(&path, 128).unwrap();
        assert_eq!((decoded.frame.width, decoded.frame.height), (60, 60));
        assert!(is_reddish(pixel(&decoded.frame.rgba, 60, 5, 30)));
    }
}

fn key(path: &Path, size: ThumbSize) -> ThumbKey {
    ThumbKey {
        path: path.into(),
        modified: Timestamp::UNKNOWN,
        size,
    }
}

#[test]
fn the_cache_serves_valid_thumbnails_without_reading_the_original() {
    let dir = tempfile::tempdir().unwrap();
    let cache = ThumbCache::new(dir.path().join("thumbnails"));
    let source = CachedDecoder::new(cache.clone());
    let photo = write(dir.path(), "photo.jpg", &jpeg(400, 200, None));
    let request = key(&photo, ThumbSize::Normal);
    let never = CancellationToken::new();

    let fresh = source.produce(&request, &never).unwrap();
    assert_eq!((fresh.frame.width, fresh.frame.height), (128, 64));

    let modified = fs::metadata(&photo).unwrap().modified().unwrap();
    let length = fs::metadata(&photo).unwrap().len();
    fs::write(&photo, vec![0u8; usize::try_from(length).unwrap()]).unwrap();
    File::options()
        .write(true)
        .open(&photo)
        .unwrap()
        .set_times(FileTimes::new().set_modified(modified))
        .unwrap();
    let cached = source.produce(&request, &never).unwrap();
    assert_eq!(cached.frame, fresh.frame);

    File::options()
        .write(true)
        .open(&photo)
        .unwrap()
        .set_times(FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(1)))
        .unwrap();
    assert!(source.produce(&request, &never).is_err());
}

#[test]
fn a_file_that_fails_is_not_decoded_again_until_it_changes() {
    let dir = tempfile::tempdir().unwrap();
    let cache = ThumbCache::new(dir.path().join("thumbnails"));
    let source = CachedDecoder::new(cache);
    let broken = write(dir.path(), "broken.png", b"\x89PNG\r\n\x1a\ngarbage");
    let request = key(&broken, ThumbSize::Large);
    let never = CancellationToken::new();
    assert!(matches!(
        source.produce(&request, &never),
        Err(ThumbError::Decode(_))
    ));
    assert!(matches!(
        source.produce(&request, &never),
        Err(ThumbError::PreviouslyFailed)
    ));
    let mut garbage = b"\x89PNG\r\n\x1a\ngarbage".to_vec();
    garbage.push(b'!');
    fs::write(&broken, garbage).unwrap();
    assert!(matches!(
        source.produce(&request, &never),
        Err(ThumbError::Decode(_))
    ));
}
