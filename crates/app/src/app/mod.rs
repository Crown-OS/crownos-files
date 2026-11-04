//! The window: the root that starts the workers, the pure shell tests drive,
//! and the handlers that turn input into navigation, selection and edits.

pub mod actions;
pub mod clipboard;
pub mod config_sync;
pub mod conflicts;
pub mod deleting;
pub mod drag_drop;
pub mod editing;
pub mod file_jobs;
#[cfg(test)]
mod fixtures;
pub mod keys;
pub mod listing;
pub mod navigation;
pub mod open_plan;
pub mod opener;
pub mod opening;
pub mod privilege;
pub mod quick_look;
pub mod root;
pub mod selection;
pub mod shell;
pub mod spring_load;
pub mod tasks;
#[cfg(test)]
mod tests;
pub mod thumbnails;
pub mod type_ahead;
pub mod virtual_folder;

pub use root::root;
pub use shell::shell;
