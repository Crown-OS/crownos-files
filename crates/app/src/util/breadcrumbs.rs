use std::path::{Component, Path};

/// One folder on the way from the top of a path to the folder itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Breadcrumb<'a> {
    /// The folder's name; empty for home and the filesystem root, which show an icon.
    pub label: &'a str,
    pub path: &'a Path,
    pub is_home: bool,
}

/// The crumbs of `dir`, starting at `home` when `dir` is inside it and at `/` otherwise.
pub fn breadcrumbs<'a>(dir: &'a Path, home: Option<&Path>) -> Vec<Breadcrumb<'a>> {
    let top = home.filter(|home| dir.starts_with(home));
    let mut crumbs: Vec<_> = dir
        .ancestors()
        .take_while(|ancestor| top.is_none_or(|top| ancestor.starts_with(top)))
        .map(|path| Breadcrumb {
            label: match path.components().next_back() {
                Some(Component::Normal(name)) if Some(path) != top => name.to_str().unwrap_or(""),
                _ => "",
            },
            path,
            is_home: Some(path) == top,
        })
        .collect();
    crumbs.reverse();
    crumbs
}

/// "Documents › Launch": where a folder is, as the details panel says it.
pub fn describe_location(dir: &Path, home: Option<&Path>) -> String {
    let crumbs = breadcrumbs(dir, home);
    let named: Vec<&str> = crumbs
        .iter()
        .filter(|crumb| !crumb.label.is_empty())
        .map(|crumb| crumb.label)
        .collect();
    if named.is_empty() {
        return if crumbs.first().is_some_and(|crumb| crumb.is_home) {
            "Home".to_owned()
        } else {
            "Computer".to_owned()
        };
    }
    named.join(" › ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_in_home_starts_at_home() {
        let home = Path::new("/home/alex");
        let crumbs = breadcrumbs(Path::new("/home/alex/Documents/Launch"), Some(home));
        let labels: Vec<_> = crumbs.iter().map(|crumb| crumb.label).collect();
        assert_eq!(labels, ["", "Documents", "Launch"]);
        assert!(crumbs[0].is_home);
        assert_eq!(crumbs[1].path, Path::new("/home/alex/Documents"));
        assert_eq!(
            describe_location(Path::new("/home/alex/Documents/Launch"), Some(home)),
            "Documents › Launch"
        );
    }

    #[test]
    fn a_folder_outside_home_starts_at_the_root() {
        let crumbs = breadcrumbs(Path::new("/usr/share"), Some(Path::new("/home/alex")));
        let labels: Vec<_> = crumbs.iter().map(|crumb| crumb.label).collect();
        assert_eq!(labels, ["", "usr", "share"]);
        assert!(!crumbs[0].is_home);
        assert_eq!(describe_location(Path::new("/"), None), "Computer");
    }
}
