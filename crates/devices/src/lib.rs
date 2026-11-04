//! What CrownOS Files reaches outside the filesystem for: removable drives
//! through UDisks2, free space, privileged file operations through pkexec,
//! tags in extended attributes, and sending files to paired devices.

#[cfg(feature = "connect")]
pub mod connect;
pub mod privileged;
pub mod storage;
pub mod tags;
pub mod udisks;
mod util;

use crownos_files_core::{fs, jobs, model};
