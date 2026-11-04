//! Runs one file operation as root for CrownOS Files. Started through
//! `pkexec`; reads a request on stdin, writes progress and the result on
//! stdout, and stops when stdin closes.

use std::{
    io::{self, Read},
    process::ExitCode,
    thread,
};

use anyhow::{Context, Result};
use crownos_files_devices::privileged::{Caller, PrivilegedRequest, frame::read_frame, serve};
use tokio_util::sync::CancellationToken;

const USAGE_ERROR: u8 = 2;

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("crownos-files-helper: {error:#}");
            ExitCode::from(USAGE_ERROR)
        }
    }
}

fn run() -> Result<bool> {
    let caller = Caller::from_env()?;
    let request: PrivilegedRequest = read_frame(&mut io::stdin().lock())
        .context("cannot read the request")?
        .context("no request on stdin")?;
    let cancel = CancellationToken::new();
    let hang_up = cancel.clone();
    thread::spawn(move || {
        let _ = io::stdin().read(&mut [0]);
        hang_up.cancel();
    });
    Ok(serve(request, caller, &cancel, &mut io::stdout().lock())?)
}
