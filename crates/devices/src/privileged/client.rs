use std::{
    ffi::OsString,
    io::ErrorKind,
    path::PathBuf,
    process::{ExitStatus, Stdio},
};

use tokio::{
    io::AsyncWriteExt,
    process::{Child, ChildStdout, Command},
    sync::mpsc::Sender,
};
use tokio_util::{sync::CancellationToken, task::AbortOnDropHandle};

use crate::{
    jobs::{JobEvent, JobId, Outcome},
    privileged::{
        error::PrivilegeError,
        frame::{
            HelperFailure, HelperFrame, HelperOutcome, HelperReport, read_frame_async, write_frame,
        },
        helper::HELPER_PATH,
        request::PrivilegedRequest,
    },
};

const LAUNCHER: &str = "pkexec";

/// Runs privileged requests through `pkexec crownos-files-helper`.
#[derive(Debug, Clone)]
pub struct PrivilegedClient {
    launcher: Option<OsString>,
    helper: PathBuf,
}

impl Default for PrivilegedClient {
    fn default() -> Self {
        Self::new(LAUNCHER, HELPER_PATH)
    }
}

/// A privileged request running as a job. It reports `JobEvent`s under `id`
/// and stops when `cancel` is cancelled.
#[derive(Debug, Clone)]
pub struct PrivilegedJob {
    pub id: JobId,
    pub cancel: CancellationToken,
}

impl PrivilegedClient {
    /// Runs `launcher helper`, as the default does with pkexec.
    pub fn new(launcher: impl Into<OsString>, helper: impl Into<PathBuf>) -> Self {
        Self {
            launcher: Some(launcher.into()),
            helper: helper.into(),
        }
    }

    /// Runs `helper` itself, without pkexec, with the caller's own rights.
    pub fn direct(helper: impl Into<PathBuf>) -> Self {
        Self {
            launcher: None,
            helper: helper.into(),
        }
    }

    /// Starts the helper on `request`; polkit asks for the password first.
    /// Cancelling `cancel` closes the helper's input, which stops it.
    pub async fn start(
        &self,
        request: &PrivilegedRequest,
        cancel: &CancellationToken,
    ) -> Result<HelperSession, PrivilegeError> {
        let mut command = match &self.launcher {
            Some(launcher) => {
                let mut command = Command::new(launcher);
                command.arg(&self.helper);
                command
            }
            None => Command::new(&self.helper),
        };
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(PrivilegeError::Spawn)?;
        let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            return Err(PrivilegeError::Spawn(ErrorKind::BrokenPipe.into()));
        };
        let mut frame = Vec::new();
        write_frame(&mut frame, request).map_err(PrivilegeError::Pipe)?;
        match stdin.write_all(&frame).await {
            Err(error) if error.kind() != ErrorKind::BrokenPipe => {
                return Err(PrivilegeError::Pipe(error));
            }
            _ => {}
        }
        let cancel = cancel.clone();
        let hangup = tokio::spawn(async move {
            cancel.cancelled().await;
            drop(stdin);
        });
        Ok(HelperSession {
            child,
            stdout,
            outcome: None,
            _hangup: AbortOnDropHandle::new(hangup),
        })
    }

    /// Runs `request` to the end and returns what it created or renamed to.
    pub async fn run(
        &self,
        request: &PrivilegedRequest,
        cancel: &CancellationToken,
    ) -> Result<Vec<PathBuf>, PrivilegeError> {
        let mut session = self.start(request, cancel).await?;
        while session.next_report().await?.is_some() {}
        session.finish().await
    }

    /// Runs `request` in the background like a scheduled job: `Started` and
    /// `Progress` for copies, moves and deletes, then `Failed` if it failed,
    /// and always `Finished`.
    pub fn submit(&self, request: PrivilegedRequest, events: Sender<JobEvent>) -> PrivilegedJob {
        let job = PrivilegedJob {
            id: JobId::next(),
            cancel: CancellationToken::new(),
        };
        let (client, id, cancel) = (self.clone(), job.id, job.cancel.clone());
        tokio::spawn(async move {
            let outcome = match client.relay(&request, id, &cancel, &events).await {
                Ok(_) => Outcome::Completed,
                Err(PrivilegeError::Cancelled) => Outcome::Cancelled,
                Err(error) => {
                    let error = error.into_job_error(request.primary_path());
                    let failed = JobEvent::Failed {
                        id,
                        error,
                        needs_privilege: false,
                    };
                    events.send(failed).await.ok();
                    Outcome::Failed
                }
            };
            let finished = JobEvent::Finished {
                id,
                outcome,
                undo: None,
            };
            events.send(finished).await.ok();
        });
        job
    }

    async fn relay(
        &self,
        request: &PrivilegedRequest,
        id: JobId,
        cancel: &CancellationToken,
        events: &Sender<JobEvent>,
    ) -> Result<Vec<PathBuf>, PrivilegeError> {
        let mut session = self.start(request, cancel).await?;
        while let Some(report) = session.next_report().await? {
            match report {
                HelperReport::Started {
                    total_bytes,
                    total_items,
                } => {
                    if let Some(kind) = request.job_kind() {
                        let started = JobEvent::Started {
                            id,
                            kind,
                            total_bytes,
                            total_items,
                        };
                        events.send(started).await.ok();
                    }
                }
                HelperReport::Progress {
                    done_bytes,
                    done_items,
                    current,
                } => {
                    let progress = JobEvent::Progress {
                        id,
                        done_bytes,
                        done_items,
                        current,
                    };
                    events.try_send(progress).ok();
                }
            }
        }
        session.finish().await
    }
}

/// A running helper.
#[derive(Debug)]
pub struct HelperSession {
    child: Child,
    stdout: ChildStdout,
    outcome: Option<Result<HelperOutcome, HelperFailure>>,
    _hangup: AbortOnDropHandle<()>,
}

impl HelperSession {
    /// The next progress report; `None` once the helper is done.
    pub async fn next_report(&mut self) -> Result<Option<HelperReport>, PrivilegeError> {
        if self.outcome.is_some() {
            return Ok(None);
        }
        let frame = read_frame_async(&mut self.stdout)
            .await
            .map_err(PrivilegeError::Pipe)?;
        Ok(match frame {
            Some(HelperFrame::Report(report)) => Some(report),
            Some(HelperFrame::Finished(outcome)) => {
                self.outcome = Some(outcome);
                None
            }
            None => None,
        })
    }

    /// Waits for the helper to exit; the paths it created or renamed to.
    pub async fn finish(mut self) -> Result<Vec<PathBuf>, PrivilegeError> {
        while self.next_report().await?.is_some() {}
        let status = self.child.wait().await.map_err(PrivilegeError::Pipe)?;
        settle(self.outcome, status)
    }
}

fn settle(
    outcome: Option<Result<HelperOutcome, HelperFailure>>,
    status: ExitStatus,
) -> Result<Vec<PathBuf>, PrivilegeError> {
    match outcome {
        Some(Ok(HelperOutcome::Completed { paths })) => Ok(paths),
        Some(Ok(HelperOutcome::Cancelled)) => Err(PrivilegeError::Cancelled),
        Some(Err(failure)) => Err(PrivilegeError::Failed(failure)),
        None => Err(PrivilegeError::from_exit_code(status.code())),
    }
}
