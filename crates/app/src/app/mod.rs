//! The window: the root that starts the workers, the pure shell tests drive,
//! and the handlers that turn input into navigation, selection and edits.

pub mod actions;
pub mod config_sync;
pub mod editing;
#[cfg(test)]
mod fixtures;
pub mod keys;
pub mod listing;
pub mod navigation;
pub mod root;
pub mod selection;
pub mod shell;
pub mod tasks;
#[cfg(test)]
mod tests;
pub mod type_ahead;

pub use root::root;
pub use shell::shell;
