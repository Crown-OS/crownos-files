//! File operations: a prioritised worker pool running copy, move, delete and
//! trash jobs, plus the instant operations (rename, create) and undo.

mod attributes;
pub mod conflict;
pub mod context;
mod copy;
pub mod copy_file;
pub mod create;
pub mod delete;
pub mod error;
pub mod event;
pub mod execute;
pub mod guard;
pub mod job;
mod r#move;
pub mod plan;
mod progress;
pub mod rename;
pub mod scheduler;
pub mod trash;
pub mod traverse;
pub mod undo;

pub use conflict::{Claim, ConflictPolicy, ConflictReply, unique_name};
pub use copy_file::copy_file;
pub use create::{create_file, create_folder};
pub use delete::remove_tree;
pub use error::{JobError, NameError};
pub use event::{JobEvent, Outcome};
pub use job::{Job, JobId, JobKind, Priority, Transfer};
pub use rename::{rename, validate_name};
pub use scheduler::Scheduler;
pub use trash::{home_trash, list_trash};
pub use undo::{Reversible, UndoStack};
