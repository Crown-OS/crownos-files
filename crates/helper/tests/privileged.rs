#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{path::Path, time::Duration};

use crownos_files_core::jobs::{ConflictPolicy, JobEvent, JobKind, Outcome};
use crownos_files_devices::privileged::{PrivilegeError, PrivilegedClient, PrivilegedRequest};
use tokio::{sync::mpsc, time::timeout};
use tokio_util::sync::CancellationToken;

const HELPER: &str = env!("CARGO_BIN_EXE_crownos-files-helper");
const PATIENCE: Duration = Duration::from_secs(20);

/// Stands in for pkexec exiting with `exit_code` before the helper runs.
fn fake_launcher(dir: &Path, exit_code: i32) -> PrivilegedClient {
    let script = dir.join(format!("pkexec-{exit_code}"));
    std::fs::write(&script, format!("exit {exit_code}\n")).unwrap();
    PrivilegedClient::new("sh", script)
}

#[tokio::test]
async fn the_helper_binary_runs_requests_without_pkexec() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::write(root.join("note"), b"hi").unwrap();
    std::fs::create_dir(root.join("into")).unwrap();
    let client = PrivilegedClient::direct(HELPER);
    let none = CancellationToken::new();

    let made = client
        .run(
            &PrivilegedRequest::CreateFolder {
                parent: root.clone(),
                name: "made".into(),
            },
            &none,
        )
        .await
        .unwrap();
    assert_eq!(made, [root.join("made")]);

    let refused = client
        .run(
            &PrivilegedRequest::Delete {
                paths: vec!["/".into()],
            },
            &none,
        )
        .await;
    assert!(
        matches!(refused, Err(PrivilegeError::Failed(failure)) if failure.message.contains("protected"))
    );

    let (events, mut received) = mpsc::channel(64);
    let job = client.submit(
        PrivilegedRequest::Move {
            sources: vec![root.join("note")],
            destination: root.join("into"),
            on_conflict: ConflictPolicy::Skip,
        },
        events,
    );
    let mut seen = Vec::new();
    loop {
        let event = timeout(PATIENCE, received.recv()).await.unwrap().unwrap();
        let last = matches!(event, JobEvent::Finished { .. });
        seen.push(event);
        if last {
            break;
        }
    }
    assert!(matches!(
        seen.first(),
        Some(JobEvent::Started { id, kind: JobKind::Move, total_items: 1, .. }) if *id == job.id
    ));
    assert!(matches!(
        seen.last(),
        Some(JobEvent::Finished {
            outcome: Outcome::Completed,
            ..
        })
    ));
    assert_eq!(std::fs::read(root.join("into/note")).unwrap(), b"hi");
}

#[tokio::test]
async fn failures_become_failed_job_events() {
    let temp = tempfile::tempdir().unwrap();
    let (events, mut received) = mpsc::channel(8);
    PrivilegedClient::direct(HELPER).submit(
        PrivilegedRequest::Delete {
            paths: vec![temp.path().join("missing")],
        },
        events,
    );
    let failed = timeout(PATIENCE, received.recv()).await.unwrap().unwrap();
    assert!(matches!(
        failed,
        JobEvent::Failed {
            needs_privilege: false,
            ..
        }
    ));
    let finished = timeout(PATIENCE, received.recv()).await.unwrap().unwrap();
    assert!(matches!(
        finished,
        JobEvent::Finished {
            outcome: Outcome::Failed,
            ..
        }
    ));
}

#[tokio::test]
async fn pkexec_exit_codes_map_to_cancelled_and_not_authorized() {
    let temp = tempfile::tempdir().unwrap();
    let request = PrivilegedRequest::Delete {
        paths: vec![temp.path().join("x")],
    };
    let none = CancellationToken::new();
    let dismissed = fake_launcher(temp.path(), 126).run(&request, &none).await;
    assert!(matches!(dismissed, Err(PrivilegeError::Cancelled)));
    let refused = fake_launcher(temp.path(), 127).run(&request, &none).await;
    assert!(matches!(refused, Err(PrivilegeError::NotAuthorized)));
    let broken = fake_launcher(temp.path(), 1).run(&request, &none).await;
    assert!(matches!(broken, Err(PrivilegeError::Exited(_))));
    let missing = PrivilegedClient::direct(temp.path().join("absent"))
        .run(&request, &none)
        .await;
    assert!(matches!(missing, Err(PrivilegeError::Spawn(_))));

    let (events, mut received) = mpsc::channel(8);
    fake_launcher(temp.path(), 126).submit(request, events);
    let finished = timeout(PATIENCE, received.recv()).await.unwrap().unwrap();
    assert!(matches!(
        finished,
        JobEvent::Finished {
            outcome: Outcome::Cancelled,
            ..
        }
    ));
}
