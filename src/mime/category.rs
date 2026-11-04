use crate::{
    mime::{database::MimeDatabase, table::MimeTable, well_known},
    model::{kind::FileKind, mime_id::MimeId},
};

impl MimeDatabase {
    /// The kind a MIME type belongs to, walking aliases and subclasses so
    /// `text/x-rust` counts as code and an unlisted `text/*` as text.
    pub fn kind_of(&self, mime: MimeId) -> FileKind {
        let Some(name) = MimeTable::name(self.unalias(mime)) else {
            return FileKind::Other;
        };
        if let Some(kind) = kind_of_name(name) {
            return kind;
        }
        let text_plain = MimeTable::intern(well_known::TEXT_PLAIN);
        if self.is_a(mime, text_plain) {
            FileKind::Code
        } else {
            FileKind::Other
        }
    }

    /// `kind` from the extension, or the MIME type's kind when the
    /// extension said nothing.
    pub fn refine_kind(&self, kind: FileKind, mime: MimeId) -> FileKind {
        match kind {
            FileKind::Other => self.kind_of(mime),
            known => known,
        }
    }

    pub fn is_previewable(&self, mime: MimeId) -> bool {
        self.kind_of(mime).is_previewable()
    }
}

fn kind_of_name(name: &str) -> Option<FileKind> {
    let kind = match name {
        well_known::DIRECTORY => FileKind::Folder,
        well_known::TEXT_PLAIN => FileKind::Text,
        "application/pdf" => FileKind::Pdf,
        "text/markdown" => FileKind::Markdown,
        "application/x-desktop"
        | "application/x-executable"
        | "application/x-pie-executable"
        | "application/vnd.appimage"
        | "application/vnd.flatpak.ref" => FileKind::Application,
        "application/msword"
        | "application/rtf"
        | "application/epub+zip"
        | "application/vnd.oasis.opendocument.text"
        | "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        | "text/x-tex" => FileKind::Document,
        "text/csv"
        | "text/tab-separated-values"
        | "application/vnd.ms-excel"
        | "application/vnd.oasis.opendocument.spreadsheet"
        | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => {
            FileKind::Spreadsheet
        }
        "application/vnd.ms-powerpoint"
        | "application/vnd.oasis.opendocument.presentation"
        | "application/vnd.openxmlformats-officedocument.presentationml.presentation" => {
            FileKind::Presentation
        }
        "application/json"
        | "application/xml"
        | "application/toml"
        | "application/yaml"
        | "application/javascript"
        | "application/x-shellscript"
        | "application/sql" => FileKind::Code,
        "application/zip"
        | "application/gzip"
        | "application/x-tar"
        | "application/x-compressed-tar"
        | "application/x-bzip2"
        | "application/x-bzip"
        | "application/x-xz"
        | "application/x-xz-compressed-tar"
        | "application/zstd"
        | "application/x-zstd-compressed-tar"
        | "application/x-7z-compressed"
        | "application/vnd.rar"
        | "application/vnd.debian.binary-package"
        | "application/x-rpm"
        | "application/java-archive"
        | "application/vnd.android.package-archive" => FileKind::Archive,
        "application/x-cd-image"
        | "application/x-raw-disk-image"
        | "application/x-qemu-disk"
        | "application/x-apple-diskimage"
        | "application/x-virtualbox-vdi"
        | "application/x-virtualbox-vmdk" => FileKind::DiskImage,
        _ => return kind_of_media_type(name),
    };
    Some(kind)
}

fn kind_of_media_type(name: &str) -> Option<FileKind> {
    let (media_type, subtype) = name.split_once('/')?;
    match media_type {
        "image" => Some(FileKind::Image),
        "video" => Some(FileKind::Video),
        "audio" => Some(FileKind::Audio),
        "font" => Some(FileKind::Font),
        "application" if subtype.starts_with("x-font-") => Some(FileKind::Font),
        _ => None,
    }
}
