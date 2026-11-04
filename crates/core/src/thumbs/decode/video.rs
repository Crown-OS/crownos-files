use std::path::Path;

use gstreamer::{self as gst, prelude::*};
use gstreamer_app::AppSink;
use gstreamer_video::{VideoCapsBuilder, VideoFormat, VideoFrameRef, VideoInfo, prelude::*};

use crate::{
    thumbs::{
        decode::{
            Decoded,
            media::{Pipeline, element, link_when_exposed, media_error, uri_decoder},
            resize::shrink_to,
        },
        error::DecodeError,
        frame::Frame,
    },
    util::{pixels::packed_rows, uri::glib_file_uri},
};

/// Any memory, so a hardware decoder is final and negotiates system
/// memory with the scaler.
const RAW_VIDEO: &str = "video/x-raw(ANY)";
/// How far in the frame is taken from, past black intros and title cards.
const SEEK_PERCENT: u128 = 10;
const RGBA_CHANNELS: usize = 4;

/// One frame at 10% of the running time, scaled by GStreamer on the way
/// out of the decoder, plus the running time.
pub fn decode_file(path: &Path, edge: u32) -> Result<Decoded, DecodeError> {
    let pipeline = Pipeline::new()?;
    let decoder = uri_decoder(&glib_file_uri(path), RAW_VIDEO)?;
    let convert = element("videoconvertscale")?;
    let largest = i32::try_from(edge).unwrap_or(i32::MAX);
    let sink = AppSink::builder()
        .caps(
            &VideoCapsBuilder::new()
                .format(VideoFormat::Rgba)
                .width_range(1..=largest)
                .height_range(1..=largest)
                .pixel_aspect_ratio(gst::Fraction::new(1, 1))
                .build(),
        )
        .max_buffers(1)
        .drop(true)
        .sync(false)
        .build();
    pipeline
        .add_many([&decoder, &convert, sink.upcast_ref()])
        .map_err(media_error)?;
    convert.link(&sink).map_err(media_error)?;
    link_when_exposed(&decoder, &convert);

    pipeline.preroll(|_| {})?;
    let duration = pipeline.duration();
    if let Some(duration) = duration {
        let target =
            gst::ClockTime::from_nseconds((duration.as_nanos() * SEEK_PERCENT / 100) as u64);
        if pipeline
            .seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT, target)
            .is_ok()
        {
            pipeline.await_async_done(|_| {})?;
        }
    }
    let sample = sink
        .try_pull_preroll(gst::ClockTime::ZERO)
        .ok_or(DecodeError::Empty)?;
    let frame = shrink_to(frame_of(&sample)?, edge)?;
    Ok(Decoded { frame, duration })
}

fn frame_of(sample: &gst::Sample) -> Result<Frame, DecodeError> {
    let caps = sample.caps().ok_or(DecodeError::Empty)?;
    let info = VideoInfo::from_caps(caps).map_err(media_error)?;
    let buffer = sample.buffer().ok_or(DecodeError::Empty)?;
    let frame = VideoFrameRef::from_buffer_ref_readable(buffer, &info).map_err(media_error)?;
    let stride = usize::try_from(frame.plane_stride()[0]).map_err(|_| DecodeError::Empty)?;
    let (width, height) = (frame.width(), frame.height());
    let rgba = packed_rows(
        frame.plane_data(0).map_err(media_error)?,
        stride,
        width as usize * RGBA_CHANNELS,
        height as usize,
    );
    Frame::new(width, height, rgba).ok_or(DecodeError::Empty)
}
