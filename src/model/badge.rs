use crate::model::kind::FileKind;

/// A content color: file-type badges and glyph accents are the only literal
/// colors in the UI, everything else comes from the theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BadgeColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl BadgeColor {
    pub const fn from_hex(rgb: u32) -> Self {
        let [_, red, green, blue] = rgb.to_be_bytes();
        Self { red, green, blue }
    }
}

/// The short label painted on a document glyph, e.g. `PDF`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Badge {
    pub label: &'static str,
    pub color: BadgeColor,
}

const fn badge(label: &'static str, rgb: u32) -> Badge {
    Badge {
        label,
        color: BadgeColor::from_hex(rgb),
    }
}

const RED: u32 = 0xe5484d;
const GRAPHITE: u32 = 0x6e6e78;
const RUST: u32 = 0xce5a2b;
const VIOLET: u32 = 0x8f6dfb;
const GREEN: u32 = 0x2f9e59;
const AMBER: u32 = 0xd97706;
const BLUE: u32 = 0x3b82f6;
const ORANGE: u32 = 0xea580c;
const YELLOW: u32 = 0xeab308;
const CYAN: u32 = 0x0891b2;
const PINK: u32 = 0xdb2777;
const BROWN: u32 = 0x8b6f47;
const SLATE: u32 = 0x475569;

const BADGES: &[(&str, Badge)] = &[
    ("pdf", badge("PDF", RED)),
    ("md", badge("MD", GRAPHITE)),
    ("markdown", badge("MD", GRAPHITE)),
    ("txt", badge("TXT", GRAPHITE)),
    ("log", badge("LOG", GRAPHITE)),
    ("rs", badge("RS", RUST)),
    ("svg", badge("SVG", VIOLET)),
    ("csv", badge("CSV", GREEN)),
    ("tsv", badge("TSV", GREEN)),
    ("xls", badge("XLS", GREEN)),
    ("xlsx", badge("XLS", GREEN)),
    ("ods", badge("ODS", GREEN)),
    ("flac", badge("FLAC", AMBER)),
    ("mp3", badge("MP3", AMBER)),
    ("wav", badge("WAV", AMBER)),
    ("ogg", badge("OGG", AMBER)),
    ("opus", badge("OPUS", AMBER)),
    ("m4a", badge("M4A", AMBER)),
    ("odt", badge("ODT", BLUE)),
    ("doc", badge("DOC", BLUE)),
    ("docx", badge("DOC", BLUE)),
    ("rtf", badge("RTF", BLUE)),
    ("epub", badge("EPUB", BLUE)),
    ("ppt", badge("PPT", ORANGE)),
    ("pptx", badge("PPT", ORANGE)),
    ("odp", badge("ODP", ORANGE)),
    ("html", badge("HTML", ORANGE)),
    ("js", badge("JS", YELLOW)),
    ("ts", badge("TS", BLUE)),
    ("py", badge("PY", BLUE)),
    ("go", badge("GO", CYAN)),
    ("c", badge("C", SLATE)),
    ("h", badge("H", SLATE)),
    ("cpp", badge("C++", SLATE)),
    ("json", badge("JSON", GRAPHITE)),
    ("toml", badge("TOML", RUST)),
    ("yaml", badge("YAML", GRAPHITE)),
    ("yml", badge("YAML", GRAPHITE)),
    ("css", badge("CSS", PINK)),
    ("sh", badge("SH", GREEN)),
    ("zip", badge("ZIP", BROWN)),
    ("tar", badge("TAR", BROWN)),
    ("gz", badge("GZ", BROWN)),
    ("xz", badge("XZ", BROWN)),
    ("zst", badge("ZST", BROWN)),
    ("7z", badge("7Z", BROWN)),
    ("iso", badge("ISO", SLATE)),
    ("img", badge("IMG", SLATE)),
    ("ttf", badge("TTF", GRAPHITE)),
    ("otf", badge("OTF", GRAPHITE)),
];

/// The badge for a file extension, if it has one.
pub fn badge_for(extension: &str) -> Option<Badge> {
    BADGES
        .iter()
        .find(|(known, _)| known.eq_ignore_ascii_case(extension))
        .map(|&(_, badge)| badge)
}

/// The accent a kind's glyph is painted with when its extension has no badge.
pub fn kind_color(kind: FileKind) -> BadgeColor {
    BadgeColor::from_hex(match kind {
        FileKind::Pdf => RED,
        FileKind::Spreadsheet => GREEN,
        FileKind::Audio => AMBER,
        FileKind::Document => BLUE,
        FileKind::Presentation => ORANGE,
        FileKind::Image => VIOLET,
        FileKind::Video => PINK,
        FileKind::Code => RUST,
        FileKind::Archive => BROWN,
        FileKind::DiskImage | FileKind::Application => SLATE,
        FileKind::Folder
        | FileKind::Markdown
        | FileKind::Text
        | FileKind::Font
        | FileKind::Other => GRAPHITE,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mockup_badges_keep_their_colors() {
        let expected = [
            ("pdf", 0xe5484d),
            ("MD", 0x6e6e78),
            ("rs", 0xce5a2b),
            ("svg", 0x8f6dfb),
            ("csv", 0x2f9e59),
            ("flac", 0xd97706),
            ("odt", 0x3b82f6),
        ];
        for (extension, rgb) in expected {
            let badge = badge_for(extension).expect("mockup extension has a badge");
            assert_eq!(badge.color, BadgeColor::from_hex(rgb), "{extension}");
            assert_eq!(badge.label, extension.to_ascii_uppercase());
        }
    }

    #[test]
    fn unknown_extensions_have_no_badge() {
        assert_eq!(badge_for("qqq"), None);
    }

    #[test]
    fn hex_splits_into_channels() {
        let color = BadgeColor::from_hex(0x123456);
        assert_eq!((color.red, color.green, color.blue), (0x12, 0x34, 0x56));
    }
}
