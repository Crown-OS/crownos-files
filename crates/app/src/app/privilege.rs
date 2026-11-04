//! A job failed for lack of permission: the window offers to run it again
//! through the pkexec helper, which asks for the password first.

use std::path::PathBuf;

use crownui::prelude::{Cx, Runtime};

use crate::{
    app::file_jobs::submit_elevated,
    jobs::{ConflictPolicy, Job, JobKind, Transfer},
    state::Store,
};
use crownos_files_devices::privileged::PrivilegedRequest;

/// The callout's message and what "Authenticate" runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElevationPrompt {
    pub message: String,
    pub request: PrivilegedRequest,
    pub kind: JobKind,
    pub items: usize,
}

/// The helper's request doing what `job` did, for the jobs it can run.
pub fn privileged_request(job: &Job) -> Option<PrivilegedRequest> {
    match job {
        Job::Copy(transfers) => {
            let (sources, destination) = into_one_dir(transfers)?;
            Some(PrivilegedRequest::Copy {
                sources,
                destination,
                on_conflict: ConflictPolicy::KeepBoth,
            })
        }
        Job::Move(transfers) => {
            let (sources, destination) = into_one_dir(transfers)?;
            Some(PrivilegedRequest::Move {
                sources,
                destination,
                on_conflict: ConflictPolicy::KeepBoth,
            })
        }
        Job::Delete(paths) => Some(PrivilegedRequest::Delete {
            paths: paths.clone(),
        }),
        Job::Trash(_) | Job::Restore(_) | Job::DeleteFromTrash(_) | Job::EmptyTrash => None,
    }
}

/// The sources and the one folder they all go to, keeping their names.
fn into_one_dir(transfers: &[Transfer]) -> Option<(Vec<PathBuf>, PathBuf)> {
    let destination = transfers.first()?.to.parent()?.to_path_buf();
    let kept_name = |transfer: &Transfer| {
        transfer.to.parent() == Some(destination.as_path())
            && transfer.to.file_name() == transfer.from.file_name()
    };
    transfers.iter().all(kept_name).then(|| {
        let sources = transfers
            .iter()
            .map(|transfer| transfer.from.clone())
            .collect();
        (sources, destination)
    })
}

/// Shows the "Authenticate" callout for `job`, or just the error when the
/// helper cannot run it.
pub fn offer_privileges(cx: &mut Cx, store: Store, job: Job, error: String) {
    let Some(request) = privileged_request(&job) else {
        store.toasts.show(cx, error);
        return;
    };
    let items = match &job {
        Job::Copy(transfers) | Job::Move(transfers) => transfers.len(),
        Job::Delete(paths) => paths.len(),
        _ => 0,
    };
    let prompt = ElevationPrompt {
        message: error,
        request,
        kind: job.kind(),
        items,
    };
    store.elevation.set(cx, Some(prompt));
}

/// "Authenticate": runs the failed job again as root.
pub fn authenticate(runtime: &mut Runtime, store: Store) {
    if let Some(prompt) = store.elevation.get(runtime) {
        store.elevation.set(runtime, None);
        submit_elevated(runtime, store, prompt.request, prompt.kind, prompt.items);
    }
}

pub fn dismiss_privileges(runtime: &mut Runtime, store: Store) {
    if store.elevation.with(runtime, Option::is_some) {
        store.elevation.set(runtime, None);
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn copies_and_moves_into_one_folder_run_as_root_and_trash_does_not() {
        let job = Job::copy_into(
            [PathBuf::from("/home/u/a"), PathBuf::from("/home/u/b")],
            Path::new("/opt"),
        );
        assert_eq!(
            privileged_request(&job),
            Some(PrivilegedRequest::Copy {
                sources: vec!["/home/u/a".into(), "/home/u/b".into()],
                destination: "/opt".into(),
                on_conflict: ConflictPolicy::KeepBoth,
            })
        );
        let duplicate = Job::duplicate([PathBuf::from("/opt/x")]);
        assert!(matches!(
            privileged_request(&duplicate),
            Some(PrivilegedRequest::Copy { destination, .. }) if destination == Path::new("/opt")
        ));
        assert_eq!(privileged_request(&Job::Trash(vec!["/opt/x".into()])), None);
        assert_eq!(
            privileged_request(&Job::Delete(vec!["/opt/x".into()])),
            Some(PrivilegedRequest::Delete {
                paths: vec!["/opt/x".into()]
            })
        );
    }
}
