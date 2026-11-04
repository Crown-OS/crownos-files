//! MIME detection through the shared-mime-info database, in process.
//!
//! Every listed entry is classified by name ([`mime_of_name`]); the UI
//! sniffs content ([`sniff`]) only for visible entries the name says
//! nothing about.

mod category;
mod database;
mod error;
mod globs;
mod hierarchy;
mod magic;
mod table;
pub mod well_known;

use std::path::Path;

pub use database::MimeDatabase;
pub use error::MimeError;
pub use table::MimeTable;

use crate::model::{kind::FileKind, mime_id::MimeId};

/// The type a file name implies, from the system database.
pub fn mime_of_name(name: &str) -> MimeId {
    MimeDatabase::system().mime_of_name(name)
}

/// The type of `path` from its content, from the system database. Blocking.
pub fn sniff(path: &Path) -> Result<MimeId, MimeError> {
    MimeDatabase::system().sniff(path)
}

pub fn kind_of(mime: MimeId) -> FileKind {
    MimeDatabase::system().kind_of(mime)
}

pub fn refine_kind(kind: FileKind, mime: MimeId) -> FileKind {
    MimeDatabase::system().refine_kind(kind, mime)
}

pub fn is_previewable(mime: MimeId) -> bool {
    MimeDatabase::system().is_previewable(mime)
}
