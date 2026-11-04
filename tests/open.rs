#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{
    fs,
    path::{Path, PathBuf},
};

use crownos_files::{
    mime::MimeDatabase,
    model::{kind::FileKind, mime_id::MimeId},
    open::{MimeApps, OpenRoute, route},
    util::xdg_dirs::XdgDirs,
};

struct Tree {
    _root: tempfile::TempDir,
    xdg: XdgDirs,
    db: &'static MimeDatabase,
}

fn write(path: PathBuf, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn app(dir: &Path, relative: &str, name: &str, extra: &str) {
    write(
        dir.join(relative),
        &format!("[Desktop Entry]\nType=Application\nName={name}\nExec=sh %F\n{extra}"),
    );
}

/// A private XDG tree: user config and data, one system data dir.
fn tree() -> Tree {
    let root = tempfile::tempdir().unwrap();
    let base = root.path();
    let xdg = XdgDirs {
        config_home: base.join("config"),
        config_dirs: vec![base.join("etc")],
        data_home: base.join("data"),
        data_dirs: vec![base.join("sys")],
        cache_home: base.join("cache"),
    };
    let mime = base.join("mime");
    write(
        mime.join("globs2"),
        "50:text/rust:*.rs\n50:image/png:*.png\n50:text/plain:*.txt\n",
    );
    write(mime.join("aliases"), "text/x-rust text/rust\n");
    write(mime.join("subclasses"), "text/rust text/plain\n");
    let db: &'static MimeDatabase = Box::leak(Box::new(MimeDatabase::load(&[mime])));

    write(
        xdg.config_home.join("mimeapps.list"),
        "[Default Applications]\ntext/plain=missing.desktop;editor.desktop;\n\n[Removed Associations]\ntext/x-rust=removed.desktop;\n",
    );
    write(
        xdg.config_home.join("crownos-mimeapps.list"),
        "[Default Applications]\nimage/png=viewer.desktop;\n",
    );
    let user_apps = xdg.data_home.join("applications");
    let system_apps = base.join("sys/applications");
    app(&user_apps, "editor.desktop", "Editor", "");
    app(&user_apps, "viewer.desktop", "Viewer", "");
    app(&user_apps, "rustide.desktop", "Rust IDE", "");
    app(&user_apps, "kde4/kate.desktop", "Kate", "");
    app(&user_apps, "hidden.desktop", "Masked", "Hidden=true\n");
    app(&user_apps, "quiet.desktop", "Quiet", "NoDisplay=true\n");
    app(&system_apps, "hidden.desktop", "Unmasked", "");
    app(&system_apps, "removed.desktop", "Removed", "");
    write(
        user_apps.join("mimeinfo.cache"),
        "[MIME Cache]\ntext/rust=removed.desktop;rustide.desktop;\ntext/plain=quiet.desktop;kde4-kate.desktop;hidden.desktop;\n",
    );
    write(
        system_apps.join("mimeinfo.cache"),
        "[MIME Cache]\ntext/plain=removed.desktop;\n",
    );
    Tree {
        _root: root,
        xdg,
        db,
    }
}

fn ids(apps: &[crownos_files::open::DesktopEntry]) -> Vec<&str> {
    apps.iter().map(|app| &*app.id).collect()
}

#[test]
fn defaults_associations_and_removals_resolve_per_the_spec() {
    let tree = tree();
    let apps = MimeApps::load(&tree.xdg, tree.db);
    let rust = MimeId::intern("text/x-rust");
    let plain = MimeId::intern("text/plain");

    let default = |mime| apps.default_app(mime).map(|app| app.id.to_string());
    assert_eq!(default(plain).as_deref(), Some("editor.desktop"));
    assert_eq!(
        default(MimeId::intern("image/png")).as_deref(),
        Some("viewer.desktop")
    );
    assert_eq!(default(rust).as_deref(), Some("rustide.desktop"));
    assert_eq!(default(MimeId::intern("application/x-nothing")), None);

    assert_eq!(
        ids(&apps.apps_for(rust)),
        [
            "rustide.desktop",
            "editor.desktop",
            "kde4-kate.desktop",
            "removed.desktop"
        ]
    );
    assert_eq!(
        ids(&apps.apps_for(plain)),
        ["editor.desktop", "kde4-kate.desktop", "removed.desktop"]
    );
    assert!(apps.find("hidden.desktop").is_none());
    assert_eq!(
        apps.find("quiet.desktop").map(|app| app.no_display),
        Some(true)
    );
}

#[test]
fn setting_a_default_rewrites_the_user_list() {
    let tree = tree();
    let mut apps = MimeApps::load(&tree.xdg, tree.db);
    let rust = MimeId::intern("text/x-rust");
    apps.set_default(rust, "kde4-kate.desktop").unwrap();
    assert_eq!(apps.default_app(rust).unwrap().name, "Kate");

    let written = fs::read_to_string(apps.user_list()).unwrap();
    assert!(
        written
            .contains("text/plain=missing.desktop;editor.desktop;\ntext/rust=kde4-kate.desktop;\n")
    );
    assert!(written.contains("[Removed Associations]\ntext/x-rust=removed.desktop;"));
    let reloaded = MimeApps::load(&tree.xdg, tree.db);
    assert_eq!(
        reloaded.default_app(rust).unwrap().id.as_ref(),
        "kde4-kate.desktop"
    );
}

#[test]
fn opening_routes_folders_previews_apps_and_the_chooser() {
    let tree = tree();
    let apps = MimeApps::load(&tree.xdg, tree.db);
    assert_eq!(
        route(
            Path::new("/x/dir"),
            FileKind::Folder,
            MimeId::UNKNOWN,
            &apps
        ),
        OpenRoute::Navigate
    );
    assert_eq!(
        route(
            Path::new("/x/main.rs"),
            FileKind::Code,
            MimeId::UNKNOWN,
            &apps
        ),
        OpenRoute::Preview
    );
    match route(
        Path::new("/x/lib.rs.orig"),
        FileKind::Other,
        MimeId::intern("text/rust"),
        &apps,
    ) {
        OpenRoute::App(app) => assert_eq!(&*app.id, "rustide.desktop"),
        other => panic!("routed to {other:?}"),
    }
    let unknown = MimeId::intern("application/x-unknown-thing");
    assert_eq!(
        route(Path::new("/x/blob.bin"), FileKind::Other, unknown, &apps),
        OpenRoute::ChooseApp(unknown)
    );
}
