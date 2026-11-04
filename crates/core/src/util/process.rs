use std::{
    ffi::OsStr,
    io,
    path::Path,
    process::{Command, Stdio},
};

const SESSION_LAUNCHER: &str = "setsid";
const FORK_FLAG: &str = "--fork";

/// Runs `argv` in a session of its own through `setsid --fork`, which
/// returns at once: the program outlives the file manager, is not its
/// child, and never lingers as its zombie. Blocking for the fork only.
pub fn spawn_detached<S: AsRef<OsStr>>(argv: &[S], working_dir: Option<&Path>) -> io::Result<()> {
    let mut command = Command::new(SESSION_LAUNCHER);
    command
        .arg(FORK_FLAG)
        .args(argv)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(dir) = working_dir.filter(|dir| dir.is_dir()) {
        command.current_dir(dir);
    }
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{SESSION_LAUNCHER} exited with {status}"
        )))
    }
}
