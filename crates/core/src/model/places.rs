use std::path::{Path, PathBuf};

use crate::{
    jobs::home_trash,
    model::special_dir::{SpecialDir, SpecialDirs},
    util::user_dirs::home_dir,
};

/// The folders the sidebar names: home, the XDG user directories and the trash.
#[derive(Debug, Clone, Default)]
pub struct Places {
    pub home: Option<PathBuf>,
    pub special: SpecialDirs,
    pub trash: Option<PathBuf>,
}

impl Places {
    pub fn load() -> Self {
        Self {
            home: home_dir(),
            special: SpecialDirs::load(),
            trash: home_trash().map(|trash| trash.join("files")),
        }
    }

    pub fn special(&self, path: &Path) -> Option<SpecialDir> {
        self.special.lookup(path)
    }

    pub fn path(&self, special: SpecialDir) -> Option<&Path> {
        self.special.path(special)
    }
}
