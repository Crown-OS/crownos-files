use std::borrow::Cow;

use crate::model::{entry::Entry, kind::FileKind};

/// What the Kind column and the details panel call an entry: the extension
/// names the format for media and archives ("AVIF image", "FLAC audio").
pub fn kind_label(entry: &Entry) -> Cow<'static, str> {
    let noun = match entry.kind {
        FileKind::Image => "image",
        FileKind::Video => "video",
        FileKind::Audio => "audio",
        FileKind::Archive => "archive",
        kind => return Cow::Borrowed(kind.label()),
    };
    match entry.extension() {
        Some(extension) => Cow::Owned(format!("{} {noun}", extension.to_ascii_uppercase())),
        None => Cow::Borrowed(entry.kind.label()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{entry::EntryExtra, stat::Stat};

    fn label(name: &str) -> String {
        kind_label(&Entry::new(
            name.into(),
            Stat::default(),
            EntryExtra::default(),
        ))
        .into_owned()
    }

    #[test]
    fn media_is_named_after_its_format() {
        assert_eq!(label("aurora-wallpaper.avif"), "AVIF image");
        assert_eq!(label("interview.flac"), "FLAC audio");
        assert_eq!(label("keynote.pdf"), "PDF Document");
        assert_eq!(label("notes"), "File");
    }
}
