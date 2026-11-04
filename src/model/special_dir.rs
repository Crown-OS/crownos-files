use std::path::{Path, PathBuf};

use crate::util::user_dirs::{home_dir, parse_user_dirs, user_dirs_file};

/// The XDG user directories the sidebar lists and the folder glyph marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecialDir {
    Home,
    Desktop,
    Documents,
    Downloads,
    Music,
    Pictures,
    Public,
    Templates,
    Videos,
}

/// The Lucide glyph drawn over a folder icon to mark a special directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FolderGlyph {
    Image,
    Music,
    Video,
    Download,
    Share,
}

impl SpecialDir {
    fn from_xdg_key(key: &str) -> Option<Self> {
        Some(match key {
            "DESKTOP" => Self::Desktop,
            "DOCUMENTS" => Self::Documents,
            "DOWNLOAD" => Self::Downloads,
            "MUSIC" => Self::Music,
            "PICTURES" => Self::Pictures,
            "PUBLICSHARE" => Self::Public,
            "TEMPLATES" => Self::Templates,
            "VIDEOS" => Self::Videos,
            _ => return None,
        })
    }

    fn default_name(self) -> &'static str {
        match self {
            Self::Home => "",
            Self::Desktop => "Desktop",
            Self::Documents => "Documents",
            Self::Downloads => "Downloads",
            Self::Music => "Music",
            Self::Pictures => "Pictures",
            Self::Public => "Public",
            Self::Templates => "Templates",
            Self::Videos => "Videos",
        }
    }

    pub fn folder_glyph(self) -> Option<FolderGlyph> {
        match self {
            Self::Pictures => Some(FolderGlyph::Image),
            Self::Music => Some(FolderGlyph::Music),
            Self::Videos => Some(FolderGlyph::Video),
            Self::Downloads => Some(FolderGlyph::Download),
            Self::Public => Some(FolderGlyph::Share),
            Self::Home | Self::Desktop | Self::Documents | Self::Templates => None,
        }
    }
}

const DEFAULTS: [SpecialDir; 8] = [
    SpecialDir::Desktop,
    SpecialDir::Documents,
    SpecialDir::Downloads,
    SpecialDir::Music,
    SpecialDir::Pictures,
    SpecialDir::Public,
    SpecialDir::Templates,
    SpecialDir::Videos,
];

/// Where each special directory lives for this user.
#[derive(Debug, Clone, Default)]
pub struct SpecialDirs {
    dirs: Vec<(PathBuf, SpecialDir)>,
}

impl SpecialDirs {
    /// Reads `user-dirs.dirs`, falling back to `~/Desktop`, `~/Documents`, ...
    pub fn load() -> Self {
        let Some(home) = home_dir() else {
            return Self::default();
        };
        let text = std::fs::read_to_string(user_dirs_file(&home)).unwrap_or_default();
        Self::from_user_dirs(&text, &home)
    }

    pub fn from_user_dirs(text: &str, home: &Path) -> Self {
        let mut dirs: Vec<_> = DEFAULTS
            .iter()
            .map(|&dir| (home.join(dir.default_name()), dir))
            .collect();
        for (key, path) in parse_user_dirs(text, home) {
            let Some(special) = SpecialDir::from_xdg_key(key) else {
                continue;
            };
            dirs.retain(|&(_, known)| known != special);
            if path != home {
                dirs.push((path, special));
            }
        }
        dirs.push((home.to_path_buf(), SpecialDir::Home));
        Self { dirs }
    }

    pub fn lookup(&self, path: &Path) -> Option<SpecialDir> {
        self.dirs
            .iter()
            .find(|(known, _)| known == path)
            .map(|&(_, special)| special)
    }

    pub fn path(&self, special: SpecialDir) -> Option<&Path> {
        self.dirs
            .iter()
            .find(|&&(_, known)| known == special)
            .map(|(path, _)| path.as_path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_dirs_override_defaults_and_mark_glyphs() {
        let home = Path::new("/home/u");
        let dirs = SpecialDirs::from_user_dirs(
            "XDG_PICTURES_DIR=\"$HOME/Photos\"\nXDG_PUBLICSHARE_DIR=\"$HOME/\"\n",
            home,
        );
        let photos = dirs.lookup(Path::new("/home/u/Photos"));
        assert_eq!(photos, Some(SpecialDir::Pictures));
        assert_eq!(
            photos.and_then(SpecialDir::folder_glyph),
            Some(FolderGlyph::Image)
        );
        assert_eq!(dirs.lookup(Path::new("/home/u/Pictures")), None);
        assert_eq!(dirs.path(SpecialDir::Public), None);
        assert_eq!(dirs.lookup(home), Some(SpecialDir::Home));
        assert_eq!(
            dirs.lookup(Path::new("/home/u/Downloads"))
                .and_then(SpecialDir::folder_glyph),
            Some(FolderGlyph::Download)
        );
    }
}
