use std::{io, path::PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("cannot read {path}: {source}", path = path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot watch for changes: {0}")]
    Watch(#[from] notify::Error),
}
