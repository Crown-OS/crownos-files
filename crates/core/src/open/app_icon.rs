//! Where the icon theme keeps an application's icon: the user's GTK icon
//! theme, the themes it inherits from, then hicolor and the pixmaps folder.
//!
//! Only application icons are looked up, so instead of reading each theme's
//! directory table this probes the places themes put them, in both layouts in
//! use: `48x48/apps` (hicolor) and `apps/48` (breeze).

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::util::xdg_dirs::XdgDirs;

const SIZES: [&str; 10] = [
    "scalable", "256x256", "128x128", "96x96", "64x64", "48x48", "256", "128", "64", "48",
];
const CONTEXT: &str = "apps";
const EXTENSIONS: [&str; 2] = ["svg", "png"];
const FALLBACK_THEME: &str = "hicolor";
const PIXMAPS: &str = "/usr/share/pixmaps";
const GTK_SETTINGS: [&str; 2] = ["gtk-4.0", "gtk-3.0"];

/// The installed theme folders, in lookup order.
#[derive(Debug, Clone, Default)]
pub struct IconThemes {
    roots: Vec<PathBuf>,
}

impl IconThemes {
    /// Resolves the user's theme and what it inherits. Blocking: a few small files.
    pub fn load(xdg: &XdgDirs) -> Self {
        let dirs: Vec<PathBuf> = xdg.data_paths("icons").collect();
        let mut themes: Vec<String> = Vec::new();
        let mut pending = vec![gtk_icon_theme(xdg).unwrap_or_else(|| FALLBACK_THEME.to_owned())];
        while let Some(theme) = pending.pop() {
            if !themes.contains(&theme) {
                pending.extend(inherited(&dirs, &theme).into_iter().rev());
                themes.push(theme);
            }
        }
        if !themes.iter().any(|theme| theme == FALLBACK_THEME) {
            themes.push(FALLBACK_THEME.to_owned());
        }
        let roots = themes
            .iter()
            .flat_map(|theme| dirs.iter().map(move |dir| dir.join(theme)))
            .filter(|root| root.is_dir())
            .collect();
        Self { roots }
    }

    /// The file of the application icon `name`, an icon name or an absolute path.
    pub fn find(&self, name: &str) -> Option<PathBuf> {
        let path = Path::new(name);
        if path.is_absolute() {
            return path.is_file().then(|| path.to_path_buf());
        }
        self.roots
            .iter()
            .find_map(|root| {
                SIZES
                    .iter()
                    .flat_map(|size| [root.join(size).join(CONTEXT), root.join(CONTEXT).join(size)])
                    .find_map(|dir| file_in(&dir, name))
            })
            .or_else(|| file_in(Path::new(PIXMAPS), name))
    }
}

fn file_in(dir: &Path, name: &str) -> Option<PathBuf> {
    EXTENSIONS
        .iter()
        .map(|extension| dir.join(format!("{name}.{extension}")))
        .find(|path| path.is_file())
}

fn inherited(dirs: &[PathBuf], theme: &str) -> Vec<String> {
    dirs.iter()
        .find_map(|dir| fs::read_to_string(dir.join(theme).join("index.theme")).ok())
        .and_then(|index| {
            index
                .lines()
                .find_map(|line| line.trim().strip_prefix("Inherits="))
                .map(|parents| {
                    parents
                        .split(',')
                        .map(str::trim)
                        .filter(|parent| !parent.is_empty())
                        .map(str::to_owned)
                        .collect()
                })
        })
        .unwrap_or_default()
}

fn gtk_icon_theme(xdg: &XdgDirs) -> Option<String> {
    GTK_SETTINGS.iter().find_map(|gtk| {
        fs::read_to_string(xdg.config_home.join(gtk).join("settings.ini"))
            .ok()?
            .lines()
            .find_map(|line| line.trim().strip_prefix("gtk-icon-theme-name="))
            .map(|name| name.trim().trim_matches('"').to_owned())
            .filter(|name| !name.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xdg(root: &Path) -> XdgDirs {
        XdgDirs {
            config_home: root.join("config"),
            config_dirs: Vec::new(),
            data_home: root.join("data"),
            data_dirs: Vec::new(),
            cache_home: root.join("cache"),
        }
    }

    #[test]
    fn the_users_theme_wins_over_what_it_inherits() {
        let root = tempfile::tempdir().unwrap();
        let icons = root.path().join("data/icons");
        for dir in ["Crown/scalable/apps", "hicolor/48x48/apps"] {
            fs::create_dir_all(icons.join(dir)).unwrap();
        }
        fs::create_dir_all(root.path().join("config/gtk-4.0")).unwrap();
        fs::write(
            root.path().join("config/gtk-4.0/settings.ini"),
            "[Settings]\ngtk-icon-theme-name=Crown\n",
        )
        .unwrap();
        fs::write(icons.join("Crown/index.theme"), "Inherits=hicolor\n").unwrap();
        fs::write(icons.join("Crown/scalable/apps/editor.svg"), "<svg/>").unwrap();
        fs::write(icons.join("hicolor/48x48/apps/editor.png"), "").unwrap();
        fs::write(icons.join("hicolor/48x48/apps/viewer.png"), "").unwrap();

        let themes = IconThemes::load(&xdg(root.path()));
        assert_eq!(
            themes.find("editor"),
            Some(icons.join("Crown/scalable/apps/editor.svg"))
        );
        assert_eq!(
            themes.find("viewer"),
            Some(icons.join("hicolor/48x48/apps/viewer.png"))
        );
        assert_eq!(themes.find("missing-app-icon"), None);
    }
}
