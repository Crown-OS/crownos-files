use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

use crate::{
    open::{
        error::OpenError,
        exec::{ExecContext, expand_exec, unescape_value},
    },
    util::which::find_program,
};

const DESKTOP_GROUP: &str = "Desktop Entry";
const APPLICATION_TYPE: &str = "Application";
const TRUE: &str = "true";

/// An installed application, as the "Open With" list shows and launches it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopEntry {
    /// The desktop file ID, e.g. `org.gnome.TextEditor.desktop`.
    pub id: Box<str>,
    pub path: PathBuf,
    pub name: String,
    pub icon: Option<String>,
    pub exec: String,
    pub working_dir: Option<PathBuf>,
    pub terminal: bool,
    /// Hidden from menus and choosers, still a valid handler.
    pub no_display: bool,
}

impl DesktopEntry {
    /// Reads the application entry at `path`, its `Name` localized for the
    /// first of `locales` it has. A `Hidden` entry, a non-application or
    /// one whose `TryExec` program is missing is an error.
    pub fn load(path: &Path, id: &str, locales: &[String]) -> Result<Self, OpenError> {
        let parsed = freedesktop_entry_parser::parse_entry(path).map_err(OpenError::io(path))?;
        let section = parsed.section(DESKTOP_GROUP);
        let invalid = |reason| OpenError::InvalidEntry {
            path: path.to_path_buf(),
            reason,
        };
        if section.attr("Hidden") == Some(TRUE) {
            return Err(OpenError::Hidden(path.to_path_buf()));
        }
        if section
            .attr("Type")
            .is_some_and(|kind| kind != APPLICATION_TYPE)
        {
            return Err(invalid("not an application"));
        }
        let exec = section.attr("Exec").ok_or_else(|| invalid("no Exec key"))?;
        if let Some(try_exec) = section.attr("TryExec")
            && find_program(OsStr::new(&unescape_value(try_exec))).is_none()
        {
            return Err(OpenError::NotInstalled(id.to_owned()));
        }
        let name = locales
            .iter()
            .find_map(|locale| section.attr_with_param("Name", locale))
            .or_else(|| section.attr("Name"))
            .ok_or_else(|| invalid("no Name key"))?;
        Ok(Self {
            id: id.into(),
            path: path.to_path_buf(),
            name: unescape_value(name),
            icon: section.attr("Icon").map(unescape_value),
            exec: exec.to_owned(),
            working_dir: section
                .attr("Path")
                .map(|dir| PathBuf::from(unescape_value(dir))),
            terminal: section.attr("Terminal") == Some(TRUE),
            no_display: section.attr("NoDisplay") == Some(TRUE),
        })
    }

    /// The command lines that open `files` with this application.
    pub fn command_lines(&self, files: &[&Path]) -> Vec<Vec<OsString>> {
        let context = ExecContext {
            name: &self.name,
            icon: self.icon.as_deref(),
            desktop_file: &self.path,
        };
        expand_exec(&self.exec, files, &context)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, format!("[Desktop Entry]\nType=Application\n{body}")).unwrap();
        path
    }

    #[test]
    fn reads_localized_names_and_flags() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(
            dir.path(),
            "editor.desktop",
            "Name=Editor\nName[de]=Bearbeiter\nExec=editor %F\nIcon=editor\nTerminal=true\nNoDisplay=true\n",
        );
        let entry =
            DesktopEntry::load(&path, "editor.desktop", &["de_DE".into(), "de".into()]).unwrap();
        assert_eq!(entry.name, "Bearbeiter");
        assert_eq!(entry.icon.as_deref(), Some("editor"));
        assert!(entry.terminal && entry.no_display);
        assert_eq!(
            entry.command_lines(&[Path::new("/a")]),
            [vec![OsString::from("editor"), OsString::from("/a")]]
        );
    }

    #[test]
    fn hidden_entries_and_missing_try_exec_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let hidden = write(dir.path(), "h.desktop", "Name=H\nExec=h\nHidden=true\n");
        assert!(matches!(
            DesktopEntry::load(&hidden, "h.desktop", &[]),
            Err(OpenError::Hidden(_))
        ));
        let missing = write(
            dir.path(),
            "m.desktop",
            "Name=M\nExec=m\nTryExec=no-such-program-anywhere\n",
        );
        assert!(matches!(
            DesktopEntry::load(&missing, "m.desktop", &[]),
            Err(OpenError::NotInstalled(_))
        ));
        let present = write(dir.path(), "p.desktop", "Name=P\nExec=sh\nTryExec=sh\n");
        assert!(DesktopEntry::load(&present, "p.desktop", &[]).is_ok());
    }
}
