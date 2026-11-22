//! The CrownUI window of CrownOS Files.

pub mod app;
pub mod state;
pub mod util;
pub mod views;

use crownos_files_core::{fs, jobs, mime, model, open, preview, thumbs};
use crownui::{
    AppConfig,
    kit::{INTER_FAMILY, inter_font},
    prelude::BLACK,
};

const INITIAL_SIZE: (u32, u32) = (1100, 700);
const MIN_SIZE: (u32, u32) = (720, 480);

/// The window as the binary opens it.
pub fn app_config() -> AppConfig {
    AppConfig::new("Files")
        .app_id("crownos-files")
        .size(INITIAL_SIZE.0, INITIAL_SIZE.1)
        .min_size(MIN_SIZE.0, MIN_SIZE.1)
        .clear_color(BLACK)
        .font(inter_font().to_vec(), Some(INTER_FAMILY))
}
