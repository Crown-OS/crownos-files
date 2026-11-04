use std::{
    env,
    path::{Path, PathBuf},
};

use crate::util::user_dirs::home_dir;

const DEFAULT_DATA_DIRS: [&str; 2] = ["/usr/local/share", "/usr/share"];
const DEFAULT_CONFIG_DIRS: [&str; 1] = ["/etc/xdg"];

/// The XDG base directories, resolved once so lookups never touch the
/// environment and tests can point every one of them at a temporary tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XdgDirs {
    pub config_home: PathBuf,
    pub config_dirs: Vec<PathBuf>,
    pub data_home: PathBuf,
    pub data_dirs: Vec<PathBuf>,
    pub cache_home: PathBuf,
}

impl XdgDirs {
    pub fn from_env() -> Option<Self> {
        let home = home_dir()?;
        Some(Self {
            config_home: absolute_var("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config")),
            config_dirs: absolute_list("XDG_CONFIG_DIRS", &DEFAULT_CONFIG_DIRS),
            data_home: absolute_var("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share")),
            data_dirs: absolute_list("XDG_DATA_DIRS", &DEFAULT_DATA_DIRS),
            cache_home: absolute_var("XDG_CACHE_HOME").unwrap_or_else(|| home.join(".cache")),
        })
    }

    /// Every directory under which a tree named `relative` is looked up,
    /// most important first: the data home, then the system data dirs.
    pub fn data_paths<'a>(&'a self, relative: &'a str) -> impl Iterator<Item = PathBuf> + 'a {
        std::iter::once(self.data_home.as_path())
            .chain(self.data_dirs.iter().map(PathBuf::as_path))
            .map(move |dir| dir.join(relative))
    }

    /// The config home then the system config dirs, most important first.
    pub fn config_paths(&self) -> impl Iterator<Item = &Path> {
        std::iter::once(self.config_home.as_path())
            .chain(self.config_dirs.iter().map(PathBuf::as_path))
    }
}

fn absolute_var(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

fn absolute_list(name: &str, defaults: &[&str]) -> Vec<PathBuf> {
    let listed: Vec<PathBuf> = env::var_os(name)
        .map(|value| {
            env::split_paths(&value)
                .filter(|path| path.is_absolute())
                .collect()
        })
        .unwrap_or_default();
    if listed.is_empty() {
        defaults.iter().map(PathBuf::from).collect()
    } else {
        listed
    }
}
