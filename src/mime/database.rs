use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    os::unix::fs::{FileTypeExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::{LazyLock, OnceLock},
};

use crate::{
    mime::{
        error::MimeError, globs::Globs, hierarchy::Hierarchy, magic::Magic, table::MimeTable,
        well_known,
    },
    model::mime_id::MimeId,
    util::xdg_dirs::XdgDirs,
};

const MIME_SUBDIR: &str = "mime";
const MAX_SNIFF_BYTES: usize = 64 * 1024;
const TEXT_PROBE_BYTES: usize = 4096;

static SYSTEM: LazyLock<MimeDatabase> = LazyLock::new(|| {
    let dirs: Vec<PathBuf> = XdgDirs::from_env()
        .map(|xdg| xdg.data_paths(MIME_SUBDIR).collect())
        .unwrap_or_else(|| vec![PathBuf::from("/usr/share/mime")]);
    MimeDatabase::load(&dirs)
});

/// shared-mime-info in process: name globs and the type hierarchy are
/// loaded up front, content rules only the first time something is sniffed.
#[derive(Debug)]
pub struct MimeDatabase {
    globs: Globs,
    hierarchy: Hierarchy,
    magic: OnceLock<Magic>,
    dirs: Box<[PathBuf]>,
}

impl MimeDatabase {
    /// The database of `$XDG_DATA_HOME/mime` and `$XDG_DATA_DIRS/*/mime`.
    pub fn system() -> &'static Self {
        &SYSTEM
    }

    /// Loads the `mime` directories in `dirs`, most important first.
    pub fn load(dirs: &[PathBuf]) -> Self {
        let mut globs = Globs::default();
        let mut hierarchy = Hierarchy::default();
        for dir in dirs {
            if let Ok(text) = fs::read_to_string(dir.join("globs2")) {
                globs.add(&text);
            }
            if let Ok(text) = fs::read_to_string(dir.join("aliases")) {
                hierarchy.add_aliases(&text);
            }
            if let Ok(text) = fs::read_to_string(dir.join("subclasses")) {
                hierarchy.add_subclasses(&text);
            }
        }
        Self {
            globs,
            hierarchy,
            magic: OnceLock::new(),
            dirs: dirs.into(),
        }
    }

    /// The type a name implies, or [`MimeId::UNKNOWN`] when no glob matches.
    pub fn mime_of_name(&self, name: &str) -> MimeId {
        self.globs
            .lookup(name)
            .map_or(MimeId::UNKNOWN, |mime| self.unalias(mime))
    }

    /// The type of `path` from its content. Opens without blocking, so a
    /// FIFO is reported rather than waited on.
    pub fn sniff(&self, path: &Path) -> Result<MimeId, MimeError> {
        let read_error = |source| MimeError::Read {
            path: path.to_path_buf(),
            source,
        };
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
            .map_err(read_error)?;
        let metadata = file.metadata().map_err(read_error)?;
        let file_type = metadata.file_type();
        let special = if file_type.is_dir() {
            Some(well_known::DIRECTORY)
        } else if file_type.is_fifo() {
            Some(well_known::FIFO)
        } else if file_type.is_socket() {
            Some(well_known::SOCKET)
        } else if file_type.is_char_device() {
            Some(well_known::CHAR_DEVICE)
        } else if file_type.is_block_device() {
            Some(well_known::BLOCK_DEVICE)
        } else {
            None
        };
        if let Some(special) = special {
            return Ok(MimeTable::intern(special));
        }
        let head =
            read_head(file, self.magic().extent().min(MAX_SNIFF_BYTES)).map_err(read_error)?;
        Ok(self.sniff_data(&head))
    }

    /// The type of a file starting with `data`: the content rules first,
    /// then plain text or binary by inspection.
    pub fn sniff_data(&self, data: &[u8]) -> MimeId {
        if data.is_empty() {
            return MimeTable::intern(well_known::ZERO_SIZE);
        }
        if let Some(mime) = self.magic().lookup(data) {
            return self.unalias(mime);
        }
        MimeTable::intern(if looks_like_text(data) {
            well_known::TEXT_PLAIN
        } else {
            well_known::OCTET_STREAM
        })
    }

    pub fn unalias(&self, mime: MimeId) -> MimeId {
        self.hierarchy.unalias(mime)
    }

    /// `mime` unaliased, then every type it is a subclass of, nearest first.
    pub fn ancestors(&self, mime: MimeId) -> Vec<MimeId> {
        self.hierarchy.ancestors(mime)
    }

    pub fn is_a(&self, mime: MimeId, base: MimeId) -> bool {
        self.hierarchy.is_a(mime, base)
    }

    fn magic(&self) -> &Magic {
        self.magic.get_or_init(|| {
            let mut magic = Magic::default();
            for dir in &self.dirs {
                if let Ok(bytes) = fs::read(dir.join("magic")) {
                    magic.add(&bytes);
                }
            }
            magic
        })
    }
}

fn read_head(file: File, limit: usize) -> std::io::Result<Vec<u8>> {
    let mut head = Vec::with_capacity(limit);
    file.take(limit as u64).read_to_end(&mut head)?;
    Ok(head)
}

fn looks_like_text(data: &[u8]) -> bool {
    let probe = &data[..data.len().min(TEXT_PROBE_BYTES)];
    let utf8 = match std::str::from_utf8(probe) {
        Ok(_) => true,
        Err(error) => error.error_len().is_none(),
    };
    utf8 && !probe
        .iter()
        .any(|&byte| byte < 0x20 && !matches!(byte, b'\t' | b'\n' | b'\r' | 0x0C | 0x1B))
}
