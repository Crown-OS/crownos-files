//! `$XDG_DATA_HOME/recently-used.xbel`, shared with every GTK app: read for
//! the Recents sidebar entry, appended to when Files opens something.

use std::{
    borrow::Cow,
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use quick_xml::{
    Reader,
    escape::escape,
    events::{BytesStart, Event},
};

use crate::{
    open::error::OpenError,
    util::{
        atomic_write::write_atomically,
        iso8601::{format_utc, parse_utc},
        uri::{glib_file_uri, uri_to_path},
        xdg_dirs::XdgDirs,
    },
};

const RECENT_FILE: &str = "recently-used.xbel";
const MAX_BOOKMARKS: usize = 500;
const FILE_MODE: u32 = 0o600;
const BOOKMARK: &[u8] = b"bookmark";
const MIME_TYPE: &[u8] = b"mime:mime-type";
const APPLICATION: &[u8] = b"bookmark:application";
const HEADER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<xbel version="1.0"
      xmlns:bookmark="http://www.freedesktop.org/standards/desktop-bookmarks"
      xmlns:mime="http://www.freedesktop.org/standards/shared-mime-info"
>
"#;
const FOOTER: &str = "</xbel>\n";

/// The application credited with opening a file.
#[derive(Debug, Clone, Copy)]
pub struct RecentApp<'a> {
    pub name: &'a str,
    pub exec: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Registration {
    name: String,
    exec: String,
    modified: String,
    count: u32,
}

/// One `<bookmark>`, with its original text so entries Files does not
/// touch are written back byte for byte.
#[derive(Debug)]
struct Bookmark<'a> {
    raw: &'a str,
    href: String,
    added: Option<String>,
    last_used: i64,
    mime: Option<String>,
    applications: Vec<Registration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentFiles {
    path: PathBuf,
}

impl RecentFiles {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn from_xdg(xdg: &XdgDirs) -> Self {
        Self::new(xdg.data_home.join(RECENT_FILE))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The recently used local files that still exist, newest first. Blocking.
    pub fn read(&self) -> Result<Vec<PathBuf>, OpenError> {
        let text = self.text()?;
        let mut bookmarks = self.parse(&text)?;
        bookmarks.sort_by_key(|bookmark| std::cmp::Reverse(bookmark.last_used));
        Ok(bookmarks
            .into_iter()
            .filter_map(|bookmark| uri_to_path(bookmark.href.as_bytes()))
            .filter(|path| path.exists())
            .collect())
    }

    /// Marks each `(file, mime type)` as just opened by `app`, keeping at
    /// most 500 bookmarks. Rewritten atomically. Blocking.
    pub fn record(&self, files: &[(&Path, &str)], app: RecentApp<'_>) -> Result<(), OpenError> {
        let text = self.text()?;
        let mut bookmarks = self.parse(&text)?;
        let now = now_utc();
        let touched: Vec<(String, &str)> = files
            .iter()
            .map(|(path, mime)| (glib_file_uri(path), *mime))
            .collect();
        let mut updated: Vec<String> = Vec::with_capacity(touched.len());
        for (href, mime) in &touched {
            let previous = bookmarks
                .iter()
                .position(|bookmark| &bookmark.href == href)
                .map(|index| bookmarks.remove(index));
            updated.push(render(href, mime, previous.as_ref(), app, &now));
        }
        bookmarks.sort_by_key(|bookmark| std::cmp::Reverse(bookmark.last_used));
        bookmarks.truncate(MAX_BOOKMARKS.saturating_sub(updated.len()));
        bookmarks.reverse();

        let mut xml =
            String::with_capacity(text.len() + updated.iter().map(String::len).sum::<usize>());
        xml.push_str(HEADER);
        for bookmark in &bookmarks {
            xml.push_str("  ");
            xml.push_str(bookmark.raw);
            xml.push('\n');
        }
        updated.iter().for_each(|bookmark| xml.push_str(bookmark));
        xml.push_str(FOOTER);
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(OpenError::io(dir))?;
        }
        write_atomically(&self.path, FILE_MODE, |out| out.write_all(xml.as_bytes()))
            .map_err(OpenError::io(&self.path))
    }

    fn text(&self) -> Result<String, OpenError> {
        match fs::read_to_string(&self.path) {
            Ok(text) => Ok(text),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(String::new()),
            Err(source) => Err(OpenError::io(&self.path)(source)),
        }
    }

    fn parse<'a>(&self, text: &'a str) -> Result<Vec<Bookmark<'a>>, OpenError> {
        parse_bookmarks(text).map_err(|source| OpenError::Xbel {
            path: self.path.clone(),
            source,
        })
    }
}

fn parse_bookmarks(text: &str) -> Result<Vec<Bookmark<'_>>, quick_xml::Error> {
    let mut reader = Reader::from_str(text);
    let mut bookmarks = Vec::new();
    let mut open: Option<(usize, Bookmark<'_>)> = None;
    loop {
        let start = usize::try_from(reader.buffer_position()).unwrap_or(text.len());
        let event = reader.read_event()?;
        let end = usize::try_from(reader.buffer_position()).unwrap_or(text.len());
        match event {
            Event::Start(element) if element.name().as_ref() == BOOKMARK => {
                open = Some((start, bookmark_of(&element)?));
            }
            Event::Empty(element) if element.name().as_ref() == BOOKMARK => {
                let mut bookmark = bookmark_of(&element)?;
                bookmark.raw = &text[start..end];
                bookmarks.push(bookmark);
            }
            Event::Empty(element) => {
                if let Some((_, bookmark)) = open.as_mut() {
                    match element.name().as_ref() {
                        MIME_TYPE => {
                            bookmark.mime = attribute(&element, "type")?.map(Cow::into_owned)
                        }
                        APPLICATION => bookmark.applications.push(registration_of(&element)?),
                        _ => {}
                    }
                }
            }
            Event::End(element) if element.name().as_ref() == BOOKMARK => {
                if let Some((from, mut bookmark)) = open.take() {
                    bookmark.raw = &text[from..end];
                    bookmarks.push(bookmark);
                }
            }
            Event::Eof => return Ok(bookmarks),
            _ => {}
        }
    }
}

fn bookmark_of<'a>(element: &BytesStart<'_>) -> Result<Bookmark<'a>, quick_xml::Error> {
    let stamp = |name| -> Result<i64, quick_xml::Error> {
        Ok(attribute(element, name)?
            .and_then(|value| parse_utc(&value))
            .unwrap_or(i64::MIN))
    };
    Ok(Bookmark {
        raw: "",
        href: attribute(element, "href")?
            .map(Cow::into_owned)
            .unwrap_or_default(),
        added: attribute(element, "added")?.map(Cow::into_owned),
        last_used: stamp("modified")?
            .max(stamp("visited")?)
            .max(stamp("added")?),
        mime: None,
        applications: Vec::new(),
    })
}

fn registration_of(element: &BytesStart<'_>) -> Result<Registration, quick_xml::Error> {
    Ok(Registration {
        name: attribute(element, "name")?
            .map(Cow::into_owned)
            .unwrap_or_default(),
        exec: attribute(element, "exec")?
            .map(Cow::into_owned)
            .unwrap_or_default(),
        modified: attribute(element, "modified")?
            .map(Cow::into_owned)
            .unwrap_or_default(),
        count: attribute(element, "count")?
            .and_then(|count| count.parse().ok())
            .unwrap_or(1),
    })
}

fn attribute<'a>(
    element: &'a BytesStart<'_>,
    name: &str,
) -> Result<Option<Cow<'a, str>>, quick_xml::Error> {
    match element.try_get_attribute(name)? {
        Some(attribute) => Ok(Some(attribute.unescape_value()?)),
        None => Ok(None),
    }
}

fn render(
    href: &str,
    mime: &str,
    previous: Option<&Bookmark<'_>>,
    app: RecentApp<'_>,
    now: &str,
) -> String {
    let exec = format!("'{}'", app.exec);
    let mut applications: Vec<Registration> = previous
        .map(|bookmark| bookmark.applications.clone())
        .unwrap_or_default();
    match applications
        .iter_mut()
        .find(|registered| registered.name == app.name)
    {
        Some(registered) => {
            registered.count = registered.count.saturating_add(1);
            registered.modified = now.to_owned();
            registered.exec = exec;
        }
        None => applications.push(Registration {
            name: app.name.to_owned(),
            exec,
            modified: now.to_owned(),
            count: 1,
        }),
    }
    let added = previous
        .and_then(|bookmark| bookmark.added.as_deref())
        .unwrap_or(now);
    let mime = previous
        .and_then(|bookmark| bookmark.mime.as_deref())
        .unwrap_or(mime);
    let mut xml = format!(
        "  <bookmark href=\"{}\" added=\"{}\" modified=\"{now}\" visited=\"{now}\">\n    <info>\n      <metadata owner=\"http://freedesktop.org\">\n        <mime:mime-type type=\"{}\"/>\n        <bookmark:applications>\n",
        escape(href),
        escape(added),
        escape(mime),
    );
    for registered in &applications {
        xml.push_str(&format!(
            "          <bookmark:application name=\"{}\" exec=\"{}\" modified=\"{}\" count=\"{}\"/>\n",
            escape(registered.name.as_str()),
            escape(registered.exec.as_str()),
            escape(registered.modified.as_str()),
            registered.count
        ));
    }
    xml.push_str(
        "        </bookmark:applications>\n      </metadata>\n    </info>\n  </bookmark>\n",
    );
    xml
}

fn now_utc() -> String {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format_utc(
        i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX),
        elapsed.subsec_micros(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const GTK_WRITTEN: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<xbel version="1.0"
      xmlns:bookmark="http://www.freedesktop.org/standards/desktop-bookmarks"
      xmlns:mime="http://www.freedesktop.org/standards/shared-mime-info"
>
  <bookmark href="file:///OLD" added="2020-01-01T00:00:00.000000Z" modified="2020-01-01T00:00:00.000000Z" visited="2020-01-01T00:00:00.000000Z">
    <info>
      <metadata owner="http://freedesktop.org">
        <mime:mime-type type="text/plain"/>
        <bookmark:groups>
          <bookmark:group>gedit</bookmark:group>
        </bookmark:groups>
        <bookmark:applications>
          <bookmark:application name="gedit" exec="&apos;gedit %u&apos;" modified="2020-01-01T00:00:00Z" count="3"/>
        </bookmark:applications>
      </metadata>
    </info>
  </bookmark>
  <bookmark href="file:///NEWER" added="2021-01-01T00:00:00Z" modified="2021-01-01T00:00:00Z" visited="2021-01-01T00:00:00Z"/>
</xbel>
"#;

    #[test]
    fn reading_lists_existing_files_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let older = dir.path().join("older.txt");
        let newer = dir.path().join("new er.txt");
        fs::write(&older, "").unwrap();
        fs::write(&newer, "").unwrap();
        let text = GTK_WRITTEN
            .replace("file:///OLD", &glib_file_uri(&older))
            .replace("file:///NEWER", &glib_file_uri(&newer));
        let recent = RecentFiles::new(dir.path().join(RECENT_FILE));
        fs::write(recent.path(), text).unwrap();
        assert_eq!(recent.read().unwrap(), [newer.clone(), older.clone()]);
        fs::remove_file(&newer).unwrap();
        assert_eq!(recent.read().unwrap(), [older]);
    }

    #[test]
    fn recording_keeps_other_entries_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let recent = RecentFiles::new(dir.path().join("share").join(RECENT_FILE));
        fs::create_dir_all(recent.path().parent().unwrap()).unwrap();
        fs::write(recent.path(), GTK_WRITTEN).unwrap();
        let opened = dir.path().join("photo & me.png");
        fs::write(&opened, "").unwrap();
        let viewer = RecentApp {
            name: "Viewer",
            exec: "viewer %u",
        };

        recent.record(&[(&opened, "image/png")], viewer).unwrap();
        recent.record(&[(&opened, "image/png")], viewer).unwrap();

        let written = fs::read_to_string(recent.path()).unwrap();
        assert!(written.contains("<bookmark:group>gedit</bookmark:group>"));
        assert!(written.contains("exec=\"&apos;gedit %u&apos;\""));
        assert!(written.contains("name=\"Viewer\" exec=\"&apos;viewer %u&apos;\""));
        assert!(written.contains("count=\"2\""));
        assert_eq!(written.matches(&*escape(glib_file_uri(&opened))).count(), 1);
        assert_eq!(recent.read().unwrap().first(), Some(&opened));
        assert_eq!(parse_bookmarks(&written).unwrap().len(), 3);
    }

    #[test]
    fn a_missing_file_has_no_recents_and_is_created_on_record() {
        let dir = tempfile::tempdir().unwrap();
        let recent = RecentFiles::new(dir.path().join(RECENT_FILE));
        assert!(recent.read().unwrap().is_empty());
        let file = dir.path().join("a.txt");
        fs::write(&file, "").unwrap();
        recent
            .record(
                &[(&file, "text/plain")],
                RecentApp {
                    name: "E",
                    exec: "e %f",
                },
            )
            .unwrap();
        assert_eq!(recent.read().unwrap(), [file]);
    }

    #[test]
    fn the_list_is_capped() {
        let dir = tempfile::tempdir().unwrap();
        let recent = RecentFiles::new(dir.path().join(RECENT_FILE));
        let bookmarks: String = (0..MAX_BOOKMARKS + 20)
            .map(|index| {
                format!(
                    "  <bookmark href=\"file:///f{index}\" modified=\"{}\"/>\n",
                    format_utc(i64::try_from(index).unwrap(), 0)
                )
            })
            .collect();
        fs::write(recent.path(), format!("{HEADER}{bookmarks}{FOOTER}")).unwrap();
        let file = dir.path().join("a.txt");
        fs::write(&file, "").unwrap();
        recent
            .record(
                &[(&file, "text/plain")],
                RecentApp {
                    name: "E",
                    exec: "e %f",
                },
            )
            .unwrap();
        let written = fs::read_to_string(recent.path()).unwrap();
        let kept = parse_bookmarks(&written).unwrap();
        assert_eq!(kept.len(), MAX_BOOKMARKS);
        assert!(!written.contains("file:///f0\""));
        assert!(written.contains(&format!("file:///f{}\"", MAX_BOOKMARKS + 19)));
    }
}
