use std::{ffi::OsString, os::unix::ffi::OsStrExt, path::PathBuf};

use anyhow::Result;
use crownos_files_core::util::uri::uri_to_path;
use crownui::{
    AppConfig,
    kit::{INTER_FAMILY, inter_font},
    prelude::BLACK,
};

#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

const INITIAL_SIZE: (u32, u32) = (1100, 700);
const MIN_SIZE: (u32, u32) = (720, 480);

/// The folder named on the command line, as a path or as the `file://` URI `%U` passes.
fn start_folder(argument: OsString) -> PathBuf {
    uri_to_path(argument.as_bytes()).unwrap_or_else(|| PathBuf::from(argument))
}

fn main() -> Result<()> {
    let start = std::env::args_os().nth(1).map(start_folder);
    let config = AppConfig::new("Files")
        .app_id("crownos-files")
        .size(INITIAL_SIZE.0, INITIAL_SIZE.1)
        .min_size(MIN_SIZE.0, MIN_SIZE.1)
        .clear_color(BLACK)
        .font(inter_font().to_vec(), Some(INTER_FAMILY));
    crownui::run(crownos_files::app::root(start), config)?;
    Ok(())
}
