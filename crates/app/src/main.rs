use std::{ffi::OsString, os::unix::ffi::OsStrExt, path::PathBuf};

use anyhow::Result;
use crownos_files_core::util::uri::uri_to_path;

#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

/// The folder named on the command line, as a path or as the `file://` URI `%U` passes.
fn start_folder(argument: OsString) -> PathBuf {
    uri_to_path(argument.as_bytes()).unwrap_or_else(|| PathBuf::from(argument))
}

fn main() -> Result<()> {
    let start = std::env::args_os().nth(1).map(start_folder);
    crownui::run(crownos_files::app::root(start), crownos_files::app_config())?;
    Ok(())
}
