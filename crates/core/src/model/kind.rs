use crate::util::extension::extension;

/// What a file is, from its extension. Declaration order is the order
/// "sort by kind" groups entries in.
///
/// Extensions rather than content sniffing: classification runs for every
/// listed entry, and MIME refinement later only touches visible ones.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum FileKind {
    Folder,
    Application,
    Image,
    Video,
    Audio,
    Pdf,
    Document,
    Markdown,
    Text,
    Spreadsheet,
    Presentation,
    Code,
    Archive,
    DiskImage,
    Font,
    #[default]
    Other,
}

const LONGEST_KNOWN_EXTENSION: usize = 10;

impl FileKind {
    pub fn classify(name: &str, is_dir: bool) -> Self {
        if is_dir {
            return Self::Folder;
        }
        extension(name).map_or(Self::Other, Self::from_extension)
    }

    pub fn from_extension(extension: &str) -> Self {
        let mut lowered = [0u8; LONGEST_KNOWN_EXTENSION];
        let Some(slot) = lowered.get_mut(..extension.len()) else {
            return Self::Other;
        };
        slot.copy_from_slice(extension.as_bytes());
        slot.make_ascii_lowercase();
        match &*slot {
            b"desktop" | b"appimage" | b"flatpakref" => Self::Application,
            b"png" | b"jpg" | b"jpeg" | b"gif" | b"webp" | b"bmp" | b"tif" | b"tiff" | b"svg"
            | b"svgz" | b"heic" | b"heif" | b"avif" | b"ico" | b"jxl" | b"raw" | b"cr2"
            | b"nef" | b"dng" => Self::Image,
            b"mp4" | b"mkv" | b"mov" | b"avi" | b"webm" | b"m4v" | b"wmv" | b"flv" | b"mpg"
            | b"mpeg" | b"ogv" | b"3gp" => Self::Video,
            b"mp3" | b"flac" | b"wav" | b"ogg" | b"oga" | b"opus" | b"m4a" | b"aac" | b"wma"
            | b"aiff" | b"alac" => Self::Audio,
            b"pdf" => Self::Pdf,
            b"doc" | b"docx" | b"odt" | b"rtf" | b"pages" | b"epub" | b"tex" => Self::Document,
            b"md" | b"markdown" => Self::Markdown,
            b"txt" | b"log" | b"nfo" => Self::Text,
            b"csv" | b"tsv" | b"xls" | b"xlsx" | b"ods" | b"numbers" => Self::Spreadsheet,
            b"ppt" | b"pptx" | b"odp" | b"key" => Self::Presentation,
            b"rs" | b"c" | b"h" | b"cc" | b"cpp" | b"hpp" | b"py" | b"js" | b"mjs" | b"ts"
            | b"tsx" | b"jsx" | b"go" | b"java" | b"kt" | b"rb" | b"sh" | b"fish" | b"zsh"
            | b"bash" | b"toml" | b"json" | b"yaml" | b"yml" | b"ron" | b"xml" | b"html"
            | b"css" | b"scss" | b"sql" | b"lua" | b"swift" | b"zig" | b"nix" | b"wgsl"
            | b"glsl" => Self::Code,
            b"zip" | b"tar" | b"gz" | b"tgz" | b"bz2" | b"xz" | b"zst" | b"7z" | b"rar"
            | b"deb" | b"rpm" | b"pkg" | b"apk" | b"jar" => Self::Archive,
            b"iso" | b"img" | b"dmg" | b"qcow2" | b"vdi" | b"vmdk" => Self::DiskImage,
            b"ttf" | b"otf" | b"woff" | b"woff2" => Self::Font,
            _ => Self::Other,
        }
    }

    /// The "Kind" column and details-panel text.
    pub fn label(self) -> &'static str {
        match self {
            Self::Folder => "Folder",
            Self::Application => "Application",
            Self::Image => "Image",
            Self::Video => "Video",
            Self::Audio => "Audio",
            Self::Pdf => "PDF Document",
            Self::Document => "Document",
            Self::Markdown => "Markdown",
            Self::Text => "Plain Text",
            Self::Spreadsheet => "Spreadsheet",
            Self::Presentation => "Presentation",
            Self::Code => "Source Code",
            Self::Archive => "Archive",
            Self::DiskImage => "Disk Image",
            Self::Font => "Font",
            Self::Other => "File",
        }
    }

    /// The generic freedesktop `mimetypes` icon every theme ships.
    pub fn generic_icon(self) -> &'static str {
        match self {
            Self::Folder => "folder",
            Self::Application => "application-x-executable",
            Self::Image => "image-x-generic",
            Self::Video => "video-x-generic",
            Self::Audio => "audio-x-generic",
            Self::Pdf => "application-pdf",
            Self::Spreadsheet => "x-office-spreadsheet",
            Self::Presentation => "x-office-presentation",
            Self::Document => "x-office-document",
            Self::Code => "text-x-script",
            Self::Archive => "package-x-generic",
            Self::DiskImage => "media-optical",
            Self::Font => "font-x-generic",
            Self::Markdown | Self::Text | Self::Other => "text-x-generic",
        }
    }

    /// Whether crownos-preview can show it.
    pub fn is_previewable(self) -> bool {
        matches!(
            self,
            Self::Image
                | Self::Video
                | Self::Audio
                | Self::Pdf
                | Self::Markdown
                | Self::Text
                | Self::Code
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_directory_is_a_folder_whatever_it_is_called() {
        assert_eq!(FileKind::classify("photos.png", true), FileKind::Folder);
    }

    #[test]
    fn extensions_classify_case_insensitively_on_the_outer_suffix() {
        assert_eq!(FileKind::classify("a.PNG", false), FileKind::Image);
        assert_eq!(FileKind::classify("Report.Pdf", false), FileKind::Pdf);
        assert_eq!(FileKind::classify("src.tar.gz", false), FileKind::Archive);
        assert_eq!(FileKind::classify("main.rs", false), FileKind::Code);
    }

    #[test]
    fn hidden_names_and_unknown_suffixes_are_other() {
        assert_eq!(FileKind::classify(".bashrc", false), FileKind::Other);
        assert_eq!(FileKind::classify("LICENSE", false), FileKind::Other);
        assert_eq!(FileKind::classify("a.qqq", false), FileKind::Other);
        assert_eq!(
            FileKind::classify("a.waytoolongextension", false),
            FileKind::Other
        );
        assert_eq!(FileKind::classify("trailing.", false), FileKind::Other);
    }

    #[test]
    fn kinds_sort_folders_first() {
        assert!(FileKind::Folder < FileKind::Image);
        assert!(FileKind::Code < FileKind::Other);
    }
}
