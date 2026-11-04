//! Opening files: routing, XDG MIME associations, desktop entries,
//! detached launching and the recently-used list.

mod associations;
pub mod desktop_entry;
pub mod error;
pub mod exec;
pub mod launch;
pub mod mimeapps;
pub mod recent;
pub mod routing;

pub use desktop_entry::DesktopEntry;
pub use error::OpenError;
pub use launch::launch;
pub use mimeapps::MimeApps;
pub use recent::{RecentApp, RecentFiles};
pub use routing::{OpenRoute, is_previewable, preview_list, route};
