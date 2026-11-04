//! Which application opens a MIME type: the XDG MIME Applications spec
//! over `mimeapps.list` files and `mimeinfo.cache`.

use std::{
    fs::{self, DirBuilder},
    io::{ErrorKind, Write},
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
};

use crate::{
    mime::MimeDatabase,
    model::mime_id::MimeId,
    open::{
        associations::{Associations, with_default},
        desktop_entry::DesktopEntry,
        error::OpenError,
    },
    util::{atomic_write::write_atomically, locale::message_locales, xdg_dirs::XdgDirs},
};

const LIST_NAME: &str = "mimeapps.list";
const DESKTOP_LIST_NAME: &str = "crownos-mimeapps.list";
const CACHE_NAME: &str = "mimeinfo.cache";
const APPLICATIONS_DIR: &str = "applications";
/// `$XDG_CONFIG_HOME/mimeapps.list`, right after the desktop-specific one.
const USER_LIST: usize = 1;
const LIST_MODE: u32 = 0o644;
const CONFIG_DIR_MODE: u32 = 0o700;

/// Every association source, most important first:
/// `$XDG_CONFIG_HOME`, `$XDG_CONFIG_DIRS`, then each applications directory
/// with its lists before its cache.
#[derive(Debug)]
pub struct MimeApps {
    sources: Vec<Associations>,
    application_dirs: Vec<PathBuf>,
    user_list: PathBuf,
    locales: Vec<String>,
    db: &'static MimeDatabase,
}

impl MimeApps {
    /// Reads every source. Blocking: a few small files.
    pub fn load(xdg: &XdgDirs, db: &'static MimeDatabase) -> Self {
        let mut sources = Vec::new();
        for dir in xdg.config_paths() {
            sources.push(Associations::read(&dir.join(DESKTOP_LIST_NAME), db));
            sources.push(Associations::read(&dir.join(LIST_NAME), db));
        }
        let application_dirs: Vec<PathBuf> = xdg.data_paths(APPLICATIONS_DIR).collect();
        for dir in &application_dirs {
            sources.push(Associations::read(&dir.join(DESKTOP_LIST_NAME), db));
            sources.push(Associations::read(&dir.join(LIST_NAME), db));
            sources.push(Associations::read(&dir.join(CACHE_NAME), db));
        }
        Self {
            sources,
            application_dirs,
            user_list: xdg.config_home.join(LIST_NAME),
            locales: message_locales(),
            db,
        }
    }

    pub fn db(&self) -> &'static MimeDatabase {
        self.db
    }

    /// The application that opens `mime`: an explicit default of the type
    /// or else its first association, then the same for each parent type.
    pub fn default_app(&self, mime: MimeId) -> Option<DesktopEntry> {
        self.db.ancestors(mime).into_iter().find_map(|mime| {
            self.explicit_default(mime).or_else(|| {
                self.associated_ids(mime)
                    .into_iter()
                    .find_map(|id| self.find(id))
            })
        })
    }

    /// The "Open With" choices: the default first, then the defaults and
    /// associations of the type and each parent, menu-hidden ones left out.
    pub fn apps_for(&self, mime: MimeId) -> Vec<DesktopEntry> {
        let mut apps: Vec<DesktopEntry> = self.default_app(mime).into_iter().collect();
        for mime in self.db.ancestors(mime) {
            let defaults = self
                .sources
                .iter()
                .filter_map(|source| source.defaults.get(&mime))
                .flatten()
                .map(|id| &**id);
            for id in defaults.chain(self.associated_ids(mime)) {
                if apps.iter().any(|app| &*app.id == id) {
                    continue;
                }
                if let Some(app) = self.find(id).filter(|app| !app.no_display) {
                    apps.push(app);
                }
            }
        }
        apps
    }

    /// The installed entry with desktop file ID `id`. The first
    /// applications directory holding the ID decides, so a `Hidden` copy
    /// in the user's directory masks the system one.
    pub fn find(&self, id: &str) -> Option<DesktopEntry> {
        if id.contains('/') {
            return None;
        }
        let path = self
            .application_dirs
            .iter()
            .flat_map(|dir| id_paths(dir, id))
            .find(|path| path.is_file())?;
        DesktopEntry::load(&path, id, &self.locales).ok()
    }

    /// Makes `desktop_id` the default for `mime` in
    /// `$XDG_CONFIG_HOME/mimeapps.list`, rewritten atomically. Blocking.
    pub fn set_default(&mut self, mime: MimeId, desktop_id: &str) -> Result<(), OpenError> {
        let mime = self.db.unalias(mime);
        let name = mime.name().ok_or(OpenError::UnknownMime)?;
        let text = match fs::read_to_string(&self.user_list) {
            Ok(text) => text,
            Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
            Err(source) => return Err(OpenError::io(&self.user_list)(source)),
        };
        let updated = with_default(&text, name, desktop_id);
        if let Some(dir) = self.user_list.parent() {
            DirBuilder::new()
                .recursive(true)
                .mode(CONFIG_DIR_MODE)
                .create(dir)
                .map_err(OpenError::io(dir))?;
        }
        write_atomically(&self.user_list, LIST_MODE, |out| {
            out.write_all(updated.as_bytes())
        })
        .map_err(OpenError::io(&self.user_list))?;
        if let Some(user) = self.sources.get_mut(USER_LIST) {
            user.defaults.insert(mime, vec![desktop_id.into()]);
        }
        Ok(())
    }

    pub fn user_list(&self) -> &Path {
        &self.user_list
    }

    fn explicit_default(&self, mime: MimeId) -> Option<DesktopEntry> {
        self.sources
            .iter()
            .filter_map(|source| source.defaults.get(&mime))
            .flatten()
            .find_map(|id| self.find(id))
    }

    /// Added and cached associations in source order, each source's
    /// removals hiding what it and the sources after it add.
    fn associated_ids(&self, mime: MimeId) -> Vec<&str> {
        let mut removed: Vec<&str> = Vec::new();
        let mut ids: Vec<&str> = Vec::new();
        for source in &self.sources {
            if let Some(gone) = source.removed.get(&mime) {
                removed.extend(gone.iter().map(|id| &**id));
            }
            for id in source.added.get(&mime).into_iter().flatten() {
                if !removed.contains(&&**id) && !ids.contains(&&**id) {
                    ids.push(id);
                }
            }
        }
        ids
    }
}

/// Where desktop file ID `id` may live under `dir`: as is, or with one of
/// its dashes standing for a subdirectory (`kde4-kate.desktop` →
/// `kde4/kate.desktop`).
fn id_paths<'a>(dir: &'a Path, id: &'a str) -> impl Iterator<Item = PathBuf> + 'a {
    std::iter::once(dir.join(id)).chain(id.match_indices('-').map(move |(dash, _)| {
        let mut nested = PathBuf::from(&id[..dash]);
        nested.push(&id[dash + 1..]);
        dir.join(nested)
    }))
}
