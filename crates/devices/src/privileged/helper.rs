use std::{
    env,
    fs::{self, Permissions},
    io::{self, Write},
    os::unix::fs::{MetadataExt, PermissionsExt, lchown},
    path::{Path, PathBuf},
    thread,
};

use rustix::process::geteuid;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{
    jobs::{
        Claim, ConflictPolicy, Job, JobError, JobEvent, JobId, Outcome, Reversible,
        context::JobContext, error::AtPath, execute::execute, rename::rename_without_replacing,
        traverse::walk, unique_name,
    },
    privileged::{
        frame::{HelperFailure, HelperFrame, HelperOutcome, HelperReport, write_frame},
        request::PrivilegedRequest,
        validate::{ValidRequest, validate},
    },
};

pub const HELPER_PATH: &str = "/usr/lib/crownos-files/crownos-files-helper";

const CALLER_VARIABLE: &str = "PKEXEC_UID";
const EVENT_QUEUE: usize = 64;

#[derive(Debug, thiserror::Error)]
pub enum CallerError {
    #[error("running as root outside pkexec: {CALLER_VARIABLE} is not set")]
    NotPkexec,
    #[error("{CALLER_VARIABLE} is not a user id")]
    InvalidUid,
}

/// Who the helper works for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Caller {
    /// Running as root for the user pkexec authenticated.
    User { uid: u32 },
    /// Not root: the helper can do nothing its caller could not.
    Unprivileged,
}

impl Caller {
    /// Refuses to run as root unless pkexec says on whose behalf.
    pub fn from_env() -> Result<Self, CallerError> {
        if !geteuid().is_root() {
            return Ok(Self::Unprivileged);
        }
        let uid = env::var(CALLER_VARIABLE).map_err(|_| CallerError::NotPkexec)?;
        uid.parse()
            .map(|uid| Self::User { uid })
            .map_err(|_| CallerError::InvalidUid)
    }

    /// The owner to give what is created in `dir`: the caller and the
    /// folder's group, when the folder is the caller's own.
    pub fn adopter(self, dir: &Path) -> Option<(u32, u32)> {
        let Self::User { uid } = self else {
            return None;
        };
        let folder = fs::metadata(dir).ok()?;
        (folder.uid() == uid).then(|| (uid, folder.gid()))
    }
}

/// Validates and runs `request`, writing its reports and then its result
/// to `output`. Returns whether it completed.
pub fn serve(
    request: PrivilegedRequest,
    caller: Caller,
    cancel: &CancellationToken,
    output: &mut impl Write,
) -> io::Result<bool> {
    let result = validate(request)
        .map_err(HelperFailure::from)
        .and_then(|valid| perform(valid, caller, cancel, output));
    let completed = matches!(result, Ok(HelperOutcome::Completed { .. }));
    write_frame(output, &HelperFrame::Finished(result))?;
    Ok(completed)
}

fn perform(
    valid: ValidRequest,
    caller: Caller,
    cancel: &CancellationToken,
    output: &mut impl Write,
) -> Result<HelperOutcome, HelperFailure> {
    let paths = match valid.into_inner() {
        PrivilegedRequest::Copy {
            sources,
            destination,
            on_conflict,
        } => {
            let job = Job::copy_into(sources, &destination);
            return run_job(job, on_conflict, caller, cancel, output);
        }
        PrivilegedRequest::Move {
            sources,
            destination,
            on_conflict,
        } => {
            let job = Job::move_into(sources, &destination);
            return run_job(job, on_conflict, caller, cancel, output);
        }
        PrivilegedRequest::Delete { paths } => {
            return run_job(
                Job::Delete(paths),
                ConflictPolicy::Skip,
                caller,
                cancel,
                output,
            );
        }
        PrivilegedRequest::CreateFolder { parent, name } => {
            let created = unique_name(&parent, &name, Claim::Dir).at(&parent)?;
            adopt(caller, &created, cancel)?;
            vec![created]
        }
        PrivilegedRequest::Rename { path, new_name } => {
            vec![rename_without_replacing(
                &path,
                path.with_file_name(new_name),
            )?]
        }
        PrivilegedRequest::Chmod { paths, mode } => {
            for path in &paths {
                fs::set_permissions(path, Permissions::from_mode(mode)).at(path)?;
            }
            paths
        }
    };
    Ok(HelperOutcome::Completed { paths })
}

/// Runs a copy, move or delete job on this thread's behalf, relaying its
/// progress as frames.
fn run_job(
    job: Job,
    policy: ConflictPolicy,
    caller: Caller,
    cancel: &CancellationToken,
    output: &mut impl Write,
) -> Result<HelperOutcome, HelperFailure> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .at(Path::new(""))?;
    let (events, mut received) = mpsc::channel(EVENT_QUEUE);
    let ctx = JobContext::new(
        JobId::next(),
        job.kind(),
        cancel.clone(),
        events,
        runtime.handle().clone(),
    )
    .with_policy(policy);
    let mut failure = None;
    let mut finished = None;
    thread::scope(|scope| {
        scope.spawn(|| execute(job, ctx));
        while let Some(event) = received.blocking_recv() {
            let report = match event {
                JobEvent::Started {
                    total_bytes,
                    total_items,
                    ..
                } => HelperReport::Started {
                    total_bytes,
                    total_items,
                },
                JobEvent::Progress {
                    done_bytes,
                    done_items,
                    current,
                    ..
                } => HelperReport::Progress {
                    done_bytes,
                    done_items,
                    current,
                },
                JobEvent::Failed { error, .. } => {
                    failure = Some(HelperFailure::from(error));
                    continue;
                }
                JobEvent::Finished { outcome, undo, .. } => {
                    finished = Some((outcome, undo));
                    continue;
                }
                JobEvent::Conflict { .. } => continue,
            };
            if write_frame(output, &HelperFrame::Report(report)).is_err() {
                cancel.cancel();
            }
        }
    });
    match finished {
        Some((Outcome::Completed, undo)) => {
            let paths = placed(undo);
            paths
                .iter()
                .try_for_each(|path| adopt(caller, path, cancel))?;
            Ok(HelperOutcome::Completed { paths })
        }
        Some((Outcome::Cancelled, _)) => Ok(HelperOutcome::Cancelled),
        Some((Outcome::Failed, _)) | None => {
            Err(failure.unwrap_or_else(|| JobError::Cancelled.into()))
        }
    }
}

/// Where a finished job put things.
fn placed(undo: Option<Reversible>) -> Vec<PathBuf> {
    match undo {
        Some(Reversible::Copy(paths)) => paths,
        Some(Reversible::Move(transfers)) => transfers.into_iter().map(|moved| moved.to).collect(),
        _ => Vec::new(),
    }
}

/// Gives `path` and everything below it to the caller when it landed in
/// one of the caller's folders.
fn adopt(caller: Caller, path: &Path, cancel: &CancellationToken) -> Result<(), JobError> {
    let Some((uid, gid)) = path.parent().and_then(|dir| caller.adopter(dir)) else {
        return Ok(());
    };
    let mut first_error = None;
    walk(path, cancel, |node| {
        let entry = node.under(path);
        if let Err(error) = lchown(&entry, Some(uid), Some(gid)) {
            first_error.get_or_insert(JobError::Io {
                path: entry.into_owned(),
                source: error,
            });
        }
    })?;
    first_error.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use rustix::process::{getgid, getuid};

    use super::*;
    use crate::privileged::frame::read_frame;

    fn run(request: PrivilegedRequest) -> (bool, Vec<HelperFrame>) {
        let mut output = Vec::new();
        let completed = serve(
            request,
            Caller::Unprivileged,
            &CancellationToken::new(),
            &mut output,
        )
        .unwrap();
        let mut input = output.as_slice();
        let frames = std::iter::from_fn(|| read_frame(&mut input).unwrap()).collect();
        (completed, frames)
    }

    fn completed_paths(frames: &[HelperFrame]) -> &[PathBuf] {
        match frames.last() {
            Some(HelperFrame::Finished(Ok(HelperOutcome::Completed { paths }))) => paths,
            other => panic!("not completed: {other:?}"),
        }
    }

    #[test]
    fn copies_deletes_creates_renames_and_chmods_in_a_tempdir() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("src/inner")).unwrap();
        std::fs::write(root.join("src/inner/file"), b"hello").unwrap();
        std::fs::create_dir(root.join("dst")).unwrap();

        let (completed, frames) = run(PrivilegedRequest::Copy {
            sources: vec![root.join("src")],
            destination: root.join("dst"),
            on_conflict: ConflictPolicy::KeepBoth,
        });
        assert!(completed);
        assert!(matches!(
            frames[0],
            HelperFrame::Report(HelperReport::Started {
                total_bytes: 5,
                total_items: 3
            })
        ));
        assert_eq!(completed_paths(&frames), [root.join("dst/src")]);
        assert_eq!(
            std::fs::read(root.join("dst/src/inner/file")).unwrap(),
            b"hello"
        );

        let (_, frames) = run(PrivilegedRequest::CreateFolder {
            parent: root.join("dst"),
            name: "src".into(),
        });
        assert_eq!(completed_paths(&frames), [root.join("dst/src (2)")]);

        let (_, frames) = run(PrivilegedRequest::Rename {
            path: root.join("dst/src (2)"),
            new_name: "renamed".into(),
        });
        assert_eq!(completed_paths(&frames), [root.join("dst/renamed")]);

        let file = root.join("dst/src/inner/file");
        run(PrivilegedRequest::Chmod {
            paths: vec![file.clone()],
            mode: 0o640,
        });
        assert_eq!(std::fs::metadata(&file).unwrap().mode() & 0o7777, 0o640);

        let (completed, _) = run(PrivilegedRequest::Delete {
            paths: vec![root.join("src")],
        });
        assert!(completed && !root.join("src").exists());
    }

    #[test]
    fn refusals_and_failures_end_with_a_failure_frame() {
        let (completed, frames) = run(PrivilegedRequest::Delete {
            paths: vec!["/proc/self".into()],
        });
        assert!(!completed);
        assert!(
            matches!(&frames[..], [HelperFrame::Finished(Err(failure))] if failure.message.contains("protected"))
        );

        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        std::fs::write(root.join("a"), b"").unwrap();
        std::fs::write(root.join("b"), b"").unwrap();
        let (completed, frames) = run(PrivilegedRequest::Rename {
            path: root.join("a"),
            new_name: "b".into(),
        });
        assert!(!completed);
        assert!(matches!(frames.last(), Some(HelperFrame::Finished(Err(_)))));
    }

    #[test]
    fn only_a_pkexec_caller_adopts_what_lands_in_their_folders() {
        let temp = tempfile::tempdir().unwrap();
        let me = getuid().as_raw();
        assert_eq!(Caller::Unprivileged.adopter(temp.path()), None);
        assert_eq!(
            Caller::User { uid: me }.adopter(temp.path()),
            Some((me, getgid().as_raw()))
        );
        assert_eq!(Caller::User { uid: me + 1 }.adopter(temp.path()), None);
        if !geteuid().is_root() {
            assert_eq!(Caller::from_env().unwrap(), Caller::Unprivileged);
        }
    }
}
