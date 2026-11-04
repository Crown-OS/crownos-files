//! The GStreamer plumbing video frames and cover art share. Decoders are
//! auto-plugged by rank, so VA-API ones are used whenever installed.

use std::{
    ops::Deref,
    time::{Duration, Instant},
};

use gstreamer::{self as gst, prelude::*};

use crate::thumbs::error::DecodeError;

const PREROLL_TIMEOUT: Duration = Duration::from_secs(5);

/// A pipeline that is shut down however decoding ends.
pub struct Pipeline(gst::Pipeline);

impl Pipeline {
    pub fn new() -> Result<Self, DecodeError> {
        gst::init().map_err(media_error)?;
        Ok(Self(gst::Pipeline::new()))
    }

    /// Pauses the pipeline and waits until it holds its first buffer,
    /// handing every bus message to `observe` meanwhile.
    pub fn preroll(&self, observe: impl FnMut(&gst::Message)) -> Result<(), DecodeError> {
        self.0.set_state(gst::State::Paused).map_err(media_error)?;
        self.await_async_done(observe)
    }

    /// Waits for the preroll that follows a state change or a flushing seek.
    pub fn await_async_done(
        &self,
        mut observe: impl FnMut(&gst::Message),
    ) -> Result<(), DecodeError> {
        let bus = self.0.bus().ok_or(DecodeError::Empty)?;
        let deadline = Instant::now() + PREROLL_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let message = bus
                .timed_pop(gst::ClockTime::from_nseconds(remaining.as_nanos() as u64))
                .ok_or_else(|| DecodeError::Media("timed out waiting for a frame".into()))?;
            observe(&message);
            match message.view() {
                gst::MessageView::AsyncDone(_) => return Ok(()),
                gst::MessageView::Eos(_) => return Err(DecodeError::Empty),
                gst::MessageView::Error(error) => return Err(media_error(error.error())),
                _ => {}
            }
        }
    }

    pub fn duration(&self) -> Option<Duration> {
        self.0
            .query_duration::<gst::ClockTime>()
            .map(|duration| Duration::from_nanos(duration.nseconds()))
            .filter(|duration| !duration.is_zero())
    }
}

impl Deref for Pipeline {
    type Target = gst::Pipeline;

    fn deref(&self) -> &gst::Pipeline {
        &self.0
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        self.0.set_state(gst::State::Null).ok();
    }
}

/// `uridecodebin` decoding only the streams `caps` describe, so a video
/// thumbnail never decodes its audio and the other way round.
pub fn uri_decoder(uri: &str, caps: &str) -> Result<gst::Element, DecodeError> {
    let caps: gst::Caps = caps.parse().map_err(media_error)?;
    gst::ElementFactory::make("uridecodebin")
        .property("uri", uri)
        .property("caps", caps)
        .property("expose-all-streams", false)
        .build()
        .map_err(media_error)
}

pub fn element(factory: &str) -> Result<gst::Element, DecodeError> {
    gst::ElementFactory::make(factory)
        .build()
        .map_err(media_error)
}

/// Links the first stream `decoder` exposes to `downstream`.
pub fn link_when_exposed(decoder: &gst::Element, downstream: &gst::Element) {
    let downstream = downstream.downgrade();
    decoder.connect_pad_added(move |_, pad| {
        let Some(sink) = downstream
            .upgrade()
            .and_then(|downstream| downstream.static_pad("sink"))
        else {
            return;
        };
        if !sink.is_linked() {
            pad.link(&sink).ok();
        }
    });
}

pub fn media_error(error: impl std::fmt::Display) -> DecodeError {
    DecodeError::Media(error.to_string())
}
