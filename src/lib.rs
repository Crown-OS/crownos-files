//! The filesystem engine of CrownOS Files: the directory model the UI renders
//! and the blocking-thread workers that keep it in sync with the disk.

pub mod error;
pub mod fs;
pub mod model;
pub mod util;
