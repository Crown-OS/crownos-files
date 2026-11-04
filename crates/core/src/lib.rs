//! The engine of CrownOS Files: the directory model, the workers that keep it
//! in sync with the disk, file jobs, MIME types, thumbnails and openers.

pub mod error;
pub mod fs;
pub mod jobs;
pub mod mime;
pub mod model;
pub mod open;
pub mod preview;
pub mod thumbs;
pub mod util;
