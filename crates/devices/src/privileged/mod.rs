//! File operations on other users' files, run as root by
//! `crownos-files-helper` through pkexec.

pub mod client;
pub mod error;
pub mod frame;
pub mod helper;
pub mod request;
pub mod validate;

pub use client::{HelperSession, PrivilegedClient, PrivilegedJob};
pub use error::{PrivilegeError, ValidationError};
pub use frame::{HelperFailure, HelperFrame, HelperOutcome, HelperReport};
pub use helper::{Caller, HELPER_PATH, serve};
pub use request::PrivilegedRequest;
pub use validate::{ValidRequest, validate};
