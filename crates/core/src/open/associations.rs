//! One `mimeapps.list` or `mimeinfo.cache` file.

use std::{fs, path::Path};

use crate::{mime::MimeDatabase, model::mime_id::MimeId, util::hash::FastHashMap};

const DEFAULT_GROUP: &str = "Default Applications";
const ADDED_GROUP: &str = "Added Associations";
const REMOVED_GROUP: &str = "Removed Associations";
const CACHE_GROUP: &str = "MIME Cache";

pub type DesktopIds = Vec<Box<str>>;

/// Desktop file IDs per (unaliased) MIME type, in the order listed.
#[derive(Debug, Default)]
pub struct Associations {
    pub defaults: FastHashMap<MimeId, DesktopIds>,
    /// `[Added Associations]`, or a `mimeinfo.cache`'s `[MIME Cache]`.
    pub added: FastHashMap<MimeId, DesktopIds>,
    pub removed: FastHashMap<MimeId, DesktopIds>,
}

impl Associations {
    /// The file at `path`; a missing or unreadable one has no associations.
    pub fn read(path: &Path, db: &MimeDatabase) -> Self {
        fs::read_to_string(path)
            .map(|text| Self::parse(&text, db))
            .unwrap_or_default()
    }

    pub fn parse(text: &str, db: &MimeDatabase) -> Self {
        let mut associations = Self::default();
        for (group, key, value) in ini_entries(text) {
            let target = match group {
                DEFAULT_GROUP => &mut associations.defaults,
                ADDED_GROUP | CACHE_GROUP => &mut associations.added,
                REMOVED_GROUP => &mut associations.removed,
                _ => continue,
            };
            let mime = db.unalias(MimeId::intern(key));
            if mime == MimeId::UNKNOWN {
                continue;
            }
            let ids = target.entry(mime).or_default();
            for id in value.split(';').map(str::trim).filter(|id| !id.is_empty()) {
                if !ids.iter().any(|listed| &**listed == id) {
                    ids.push(id.into());
                }
            }
        }
        associations
    }
}

/// `(group, key, value)` for every assignment, comments and blanks skipped.
fn ini_entries(text: &str) -> impl Iterator<Item = (&str, &str, &str)> {
    let mut group = "";
    text.lines().filter_map(move |line| {
        let line = line.trim();
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            group = name;
            return None;
        }
        if line.starts_with('#') {
            return None;
        }
        let (key, value) = line.split_once('=')?;
        Some((group, key.trim(), value.trim()))
    })
}

/// `text` with `mime` defaulting to `desktop_id`: the existing key is
/// replaced, everything else is kept as written.
pub fn with_default(text: &str, mime: &str, desktop_id: &str) -> String {
    let assignment = format!("{mime}={desktop_id};\n");
    let mut out = String::with_capacity(text.len() + assignment.len() + DEFAULT_GROUP.len() + 3);
    let mut in_defaults = false;
    let mut written = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if in_defaults && !written {
                let separated = out.ends_with("\n\n");
                if separated {
                    out.pop();
                }
                out.push_str(&assignment);
                if separated {
                    out.push('\n');
                }
                written = true;
            }
            in_defaults = trimmed == format!("[{DEFAULT_GROUP}]");
        } else if in_defaults
            && trimmed
                .split_once('=')
                .is_some_and(|(key, _)| key.trim() == mime)
        {
            if !written {
                out.push_str(&assignment);
                written = true;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if !written {
        if !in_defaults {
            if !out.is_empty() && !out.ends_with("\n\n") {
                out.push('\n');
            }
            out.push_str(&format!("[{DEFAULT_GROUP}]\n"));
        }
        out.push_str(&assignment);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_default_goes_into_the_existing_group() {
        let text = "[Added Associations]\ntext/plain=a.desktop;\n\n[Default Applications]\nimage/png=old.desktop;\ntext/plain=b.desktop;\n";
        assert_eq!(
            with_default(text, "image/png", "new.desktop"),
            "[Added Associations]\ntext/plain=a.desktop;\n\n[Default Applications]\nimage/png=new.desktop;\ntext/plain=b.desktop;\n"
        );
        assert_eq!(
            with_default(text, "video/mp4", "mpv.desktop"),
            format!("{text}video/mp4=mpv.desktop;\n")
        );
    }

    #[test]
    fn an_addition_stays_inside_its_group() {
        assert_eq!(
            with_default(
                "[Default Applications]\na/b=x;\n\n[Added Associations]\n",
                "c/d",
                "y"
            ),
            "[Default Applications]\na/b=x;\nc/d=y;\n\n[Added Associations]\n"
        );
    }

    #[test]
    fn the_group_is_created_when_missing() {
        assert_eq!(
            with_default("", "text/plain", "e.desktop"),
            "[Default Applications]\ntext/plain=e.desktop;\n"
        );
        assert_eq!(
            with_default("[Added Associations]\nx/y=a;\n", "text/plain", "e.desktop"),
            "[Added Associations]\nx/y=a;\n\n[Default Applications]\ntext/plain=e.desktop;\n"
        );
    }
}
