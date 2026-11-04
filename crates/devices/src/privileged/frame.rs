//! The helper's wire format: postcard values, each behind a little-endian
//! `u32` length.

use std::{
    io::{self, ErrorKind, Read, Write},
    path::PathBuf,
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::{jobs::JobError, privileged::error::ValidationError, util::path_bytes};

/// Larger frames are refused: no request or reply needs this much.
pub const MAX_FRAME_LEN: usize = 64 << 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HelperReport {
    Started {
        total_bytes: u64,
        total_items: u64,
    },
    Progress {
        done_bytes: u64,
        done_items: u64,
        #[serde(with = "path_bytes")]
        current: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HelperOutcome {
    /// Done; `paths` are what was created or renamed to.
    Completed {
        #[serde(with = "path_bytes::list")]
        paths: Vec<PathBuf>,
    },
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{message}")]
pub struct HelperFailure {
    #[serde(with = "path_bytes")]
    pub path: PathBuf,
    pub errno: Option<i32>,
    pub message: String,
}

impl HelperFailure {
    pub fn into_job_error(self) -> JobError {
        let source = match self.errno {
            Some(errno) => io::Error::from_raw_os_error(errno),
            None => io::Error::other(self.message),
        };
        JobError::Io {
            path: self.path,
            source,
        }
    }
}

impl From<JobError> for HelperFailure {
    fn from(error: JobError) -> Self {
        let (path, errno) = match &error {
            JobError::Io { path, source } => (path.clone(), source.raw_os_error()),
            _ => (PathBuf::new(), None),
        };
        Self {
            path,
            errno,
            message: error.to_string(),
        }
    }
}

impl From<ValidationError> for HelperFailure {
    fn from(error: ValidationError) -> Self {
        Self {
            path: PathBuf::new(),
            errno: None,
            message: error.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HelperFrame {
    Report(HelperReport),
    /// Always the last frame.
    Finished(Result<HelperOutcome, HelperFailure>),
}

pub fn write_frame<T: Serialize>(output: &mut impl Write, value: &T) -> io::Result<()> {
    let payload = postcard::to_stdvec(value).map_err(invalid)?;
    let len = u32::try_from(payload.len()).map_err(invalid)?;
    output.write_all(&len.to_le_bytes())?;
    output.write_all(&payload)?;
    output.flush()
}

/// The next frame, or `None` at the end of the stream.
pub fn read_frame<T: DeserializeOwned>(input: &mut impl Read) -> io::Result<Option<T>> {
    let mut len = [0; 4];
    match input.read_exact(&mut len) {
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(None),
        other => other?,
    }
    let mut payload = vec![0; checked_len(len)?];
    input.read_exact(&mut payload)?;
    postcard::from_bytes(&payload).map(Some).map_err(invalid)
}

pub async fn read_frame_async<T: DeserializeOwned>(
    input: &mut (impl AsyncRead + Unpin),
) -> io::Result<Option<T>> {
    let mut len = [0; 4];
    match input.read_exact(&mut len).await {
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(None),
        other => other?,
    };
    let mut payload = vec![0; checked_len(len)?];
    input.read_exact(&mut payload).await?;
    postcard::from_bytes(&payload).map(Some).map_err(invalid)
}

fn checked_len(header: [u8; 4]) -> io::Result<usize> {
    usize::try_from(u32::from_le_bytes(header))
        .ok()
        .filter(|&len| len <= MAX_FRAME_LEN)
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "frame too large"))
}

fn invalid(error: impl std::error::Error + Send + Sync + 'static) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, error)
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    use super::*;
    use crate::{jobs::ConflictPolicy, privileged::request::PrivilegedRequest};

    #[test]
    fn requests_and_frames_round_trip_with_non_utf8_paths() {
        let odd = PathBuf::from(OsString::from_vec(b"/tmp/caf\xe9".to_vec()));
        let request = PrivilegedRequest::Copy {
            sources: vec![odd.clone(), "/etc/hosts".into()],
            destination: "/root".into(),
            on_conflict: ConflictPolicy::KeepBoth,
        };
        let frames = [
            HelperFrame::Report(HelperReport::Progress {
                done_bytes: 7,
                done_items: 1,
                current: odd.clone(),
            }),
            HelperFrame::Finished(Err(HelperFailure {
                path: odd,
                errno: Some(13),
                message: "denied".into(),
            })),
            HelperFrame::Finished(Ok(HelperOutcome::Completed {
                paths: vec!["/root/hosts".into()],
            })),
        ];
        let mut wire = Vec::new();
        write_frame(&mut wire, &request).unwrap();
        frames
            .iter()
            .for_each(|frame| write_frame(&mut wire, frame).unwrap());
        let mut input = wire.as_slice();
        assert_eq!(read_frame(&mut input).unwrap(), Some(request));
        for frame in &frames {
            assert_eq!(
                read_frame::<HelperFrame>(&mut input).unwrap().as_ref(),
                Some(frame)
            );
        }
        assert_eq!(read_frame::<HelperFrame>(&mut input).unwrap(), None);
    }

    #[test]
    fn oversized_and_truncated_frames_are_errors() {
        let huge = u32::MAX.to_le_bytes();
        assert!(read_frame::<HelperFrame>(&mut huge.as_slice()).is_err());
        let mut truncated = [9, 0, 0, 0, 1].as_slice();
        assert!(read_frame::<HelperFrame>(&mut truncated).is_err());
    }

    #[test]
    fn failures_become_job_errors_that_keep_their_errno() {
        let failure = HelperFailure {
            path: "/root/x".into(),
            errno: Some(13),
            message: "denied".into(),
        };
        assert!(failure.into_job_error().needs_privilege());
    }
}
