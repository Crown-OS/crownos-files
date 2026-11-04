use std::{
    env,
    path::{Path, PathBuf},
};

const HOME_VARIABLE: &str = "$HOME";

/// The `XDG_<KEY>_DIR` assignments in a `user-dirs.dirs` file, as
/// `(KEY, path)`, with `$HOME` expanded against `home`.
pub fn parse_user_dirs<'a>(
    text: &'a str,
    home: &'a Path,
) -> impl Iterator<Item = (&'a str, PathBuf)> + 'a {
    text.lines().filter_map(move |line| {
        let (key, value) = line.trim().split_once('=')?;
        let key = key.strip_prefix("XDG_")?.strip_suffix("_DIR")?;
        let value = value.trim().strip_prefix('"')?.strip_suffix('"')?;
        let path = match value.strip_prefix(HOME_VARIABLE) {
            Some(relative) => home.join(relative.trim_start_matches('/')),
            None if value.starts_with('/') => PathBuf::from(value),
            None => return None,
        };
        Some((key, path))
    })
}

pub fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

pub fn user_dirs_file(home: &Path) -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"))
        .join("user-dirs.dirs")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_home_and_skips_comments_and_relative_values() {
        let text = "# generated\nXDG_DOWNLOAD_DIR=\"$HOME/Downloads\"\nXDG_MUSIC_DIR=\"/srv/music\"\nXDG_BOGUS_DIR=\"relative\"\nXDG_DESKTOP_DIR=\"$HOME/\"\n";
        let home = Path::new("/home/u");
        let dirs: Vec<_> = parse_user_dirs(text, home).collect();
        assert_eq!(
            dirs,
            [
                ("DOWNLOAD", PathBuf::from("/home/u/Downloads")),
                ("MUSIC", PathBuf::from("/srv/music")),
                ("DESKTOP", PathBuf::from("/home/u/")),
            ]
        );
    }
}
