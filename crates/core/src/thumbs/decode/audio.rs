use std::path::Path;

use gstreamer::{self as gst, prelude::*};

use crate::{
    thumbs::{
        decode::{
            Decoded,
            media::{Pipeline, element, link_when_exposed, media_error, uri_decoder},
            raster,
        },
        error::DecodeError,
    },
    util::uri::glib_file_uri,
};

const RAW_AUDIO: &str = "audio/x-raw(ANY)";

/// The cover art a song embeds, read from its tags while prerolling, plus
/// the running time.
pub fn decode_file(path: &Path, edge: u32) -> Result<Decoded, DecodeError> {
    let pipeline = Pipeline::new()?;
    let decoder = uri_decoder(&glib_file_uri(path), RAW_AUDIO)?;
    let sink = element("fakesink")?;
    pipeline.add_many([&decoder, &sink]).map_err(media_error)?;
    link_when_exposed(&decoder, &sink);

    let mut cover: Option<gst::Sample> = None;
    pipeline.preroll(|message| {
        if let gst::MessageView::Tag(tag) = message.view()
            && cover.is_none()
        {
            let tags = tag.tags();
            cover = tags
                .get::<gst::tags::Image>()
                .or_else(|| tags.get::<gst::tags::PreviewImage>())
                .map(|value| value.get());
        }
    })?;
    let duration = pipeline.duration();
    let cover = cover.ok_or(DecodeError::Empty)?;
    let buffer = cover.buffer().ok_or(DecodeError::Empty)?;
    let bytes = buffer.map_readable().map_err(media_error)?;
    let frame = raster::decode_bytes(bytes.as_slice(), edge)?;
    Ok(Decoded { frame, duration })
}
