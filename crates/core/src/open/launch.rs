use std::{ffi::OsString, path::Path};

use crate::{
    mime::{MimeDatabase, well_known},
    open::{
        desktop_entry::DesktopEntry,
        error::OpenError,
        recent::{RecentApp, RecentFiles},
    },
    util::{process::spawn_detached, which::find_program},
};

const TERMINAL_LAUNCHER: &str = "xdg-terminal-exec";

/// Starts `entry` on `files`, detached from the file manager, then records
/// the files in `recent` when given. Blocking: run it off the UI thread.
pub fn launch(
    entry: &DesktopEntry,
    files: &[&Path],
    recent: Option<&RecentFiles>,
) -> Result<(), OpenError> {
    let commands = entry.command_lines(files);
    if commands.is_empty() {
        return Err(OpenError::NothingToRun(entry.id.to_string()));
    }
    for argv in commands {
        run(entry, argv)?;
    }
    if let Some(recent) = recent {
        let db = MimeDatabase::system();
        let typed: Vec<(&Path, &str)> = files
            .iter()
            .map(|file| {
                let name = file
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default();
                let mime = db
                    .mime_of_name(name)
                    .name()
                    .unwrap_or(well_known::OCTET_STREAM);
                (*file, mime)
            })
            .collect();
        let app = RecentApp {
            name: &entry.name,
            exec: &entry.exec,
        };
        recent.record(&typed, app).ok();
    }
    Ok(())
}

fn run(entry: &DesktopEntry, mut argv: Vec<OsString>) -> Result<(), OpenError> {
    let program = argv
        .first()
        .ok_or_else(|| OpenError::NothingToRun(entry.id.to_string()))?;
    if find_program(program).is_none() {
        return Err(OpenError::NotInstalled(
            program.to_string_lossy().into_owned(),
        ));
    }
    if entry.terminal {
        argv.insert(0, OsString::from(TERMINAL_LAUNCHER));
    }
    spawn_detached(&argv, entry.working_dir.as_deref()).map_err(|source| OpenError::Spawn {
        program: entry.id.to_string(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{Duration, Instant},
    };

    use super::*;

    #[test]
    fn the_launched_program_runs_with_the_files_and_is_recorded() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("ran");
        let file = dir.path().join("note.txt");
        fs::write(&file, "").unwrap();
        let desktop = dir.path().join("touch.desktop");
        fs::write(
            &desktop,
            "[Desktop Entry]\nType=Application\nName=Toucher\nExec=sh -c \"cp \\\\\"\\$1\\\\\" ran\" sh %f\n",
        )
        .unwrap();
        let mut entry = DesktopEntry::load(&desktop, "touch.desktop", &[]).unwrap();
        entry.working_dir = Some(dir.path().to_path_buf());
        let recent = RecentFiles::new(dir.path().join("recently-used.xbel"));

        launch(&entry, &[&file], Some(&recent)).unwrap();

        let deadline = Instant::now() + Duration::from_secs(10);
        while !marker.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(marker.exists(), "the program did not run");
        assert_eq!(recent.read().unwrap(), [file]);
    }

    #[test]
    fn a_missing_program_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let desktop = dir.path().join("x.desktop");
        fs::write(
            &desktop,
            "[Desktop Entry]\nName=X\nExec=no-such-program-anywhere %f\n",
        )
        .unwrap();
        let entry = DesktopEntry::load(&desktop, "x.desktop", &[]).unwrap();
        assert!(matches!(
            launch(&entry, &[], None),
            Err(OpenError::NotInstalled(_))
        ));
    }
}
