use std::path::PathBuf;

use anyhow::Result;
use crownui::{
    AppConfig,
    kit::{INTER_FAMILY, inter_font},
    prelude::BLACK,
};

#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

const INITIAL_SIZE: (u32, u32) = (1100, 700);
const MIN_SIZE: (u32, u32) = (720, 480);

fn main() -> Result<()> {
    let start = std::env::args_os().nth(1).map(PathBuf::from);
    let config = AppConfig::new("Files")
        .app_id("crownos-files")
        .size(INITIAL_SIZE.0, INITIAL_SIZE.1)
        .min_size(MIN_SIZE.0, MIN_SIZE.1)
        .clear_color(BLACK)
        .font(inter_font().to_vec(), Some(INTER_FAMILY));
    crownui::run(crownos_files::app::root(start), config)?;
    Ok(())
}
