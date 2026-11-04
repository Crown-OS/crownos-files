use crate::model::{
    badge::{Badge, BadgeColor, badge_for, kind_color},
    entry::Entry,
    kind::FileKind,
    special_dir::{FolderGlyph, SpecialDir},
};

/// Which glyph stands for an entry until its thumbnail is decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    Folder(Option<FolderGlyph>),
    Photo,
    Video,
    Page(Option<Badge>),
    Vector(Option<Badge>),
    Code(Option<Badge>),
    Sheet(Option<Badge>, BadgeColor),
    Audio(Option<Badge>, BadgeColor),
    Disc,
    Archive,
}

impl Glyph {
    pub fn for_entry(entry: &Entry, special: Option<SpecialDir>) -> Self {
        let badge = entry.extension().and_then(badge_for);
        let accent = badge.map_or_else(|| kind_color(entry.kind), |badge| badge.color);
        match entry.kind {
            FileKind::Folder => Self::Folder(special.and_then(SpecialDir::folder_glyph)),
            FileKind::Image if badge.is_some_and(|badge| badge.label == "SVG") => {
                Self::Vector(badge)
            }
            FileKind::Image => Self::Photo,
            FileKind::Video => Self::Video,
            FileKind::Code => Self::Code(badge),
            FileKind::Spreadsheet => Self::Sheet(badge, accent),
            FileKind::Audio => Self::Audio(badge, accent),
            FileKind::DiskImage => Self::Disc,
            FileKind::Archive => Self::Archive,
            FileKind::Application
            | FileKind::Pdf
            | FileKind::Document
            | FileKind::Markdown
            | FileKind::Text
            | FileKind::Presentation
            | FileKind::Font
            | FileKind::Other => Self::Page(badge),
        }
    }

    /// Whether the file's own picture replaces this glyph once decoded.
    pub fn has_thumbnail(self) -> bool {
        matches!(self, Self::Photo | Self::Video)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{entry::EntryExtra, stat::Stat};

    fn glyph(name: &str) -> Glyph {
        let stat = Stat {
            mode: if name.ends_with('/') {
                0o040755
            } else {
                0o100644
            },
            ..Stat::default()
        };
        let entry = Entry::new(
            name.trim_end_matches('/').into(),
            stat,
            EntryExtra::default(),
        );
        Glyph::for_entry(&entry, None)
    }

    #[test]
    fn the_mockup_files_get_the_mockup_glyphs() {
        assert_eq!(glyph("Brand/"), Glyph::Folder(None));
        assert_eq!(glyph("aurora-wallpaper.avif"), Glyph::Photo);
        assert_eq!(glyph("launch-film.webm"), Glyph::Video);
        assert_eq!(glyph("crownos-2026.11-x86_64.iso"), Glyph::Disc);
        assert_eq!(glyph("launch-assets.tar.zst"), Glyph::Archive);
        assert!(matches!(glyph("palette.svg"), Glyph::Vector(Some(_))));
        assert!(matches!(glyph("compositor.rs"), Glyph::Code(Some(_))));
        assert!(matches!(glyph("benchmarks.csv"), Glyph::Sheet(Some(_), _)));
        assert!(matches!(glyph("interview.flac"), Glyph::Audio(Some(_), _)));
        assert!(matches!(glyph("keynote.pdf"), Glyph::Page(Some(badge)) if badge.label == "PDF"));
    }
}
