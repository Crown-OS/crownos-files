use std::{io, path::PathBuf, time::Duration};

use capture::Carrier;
use crownos_files::app::{
    drive_worker::{DriveAction, DriveEvent},
    drives::finish_drives,
    file_jobs::{JobUpdate, Purpose, done_message, finish_jobs, submit},
};
use crownos_files_core::jobs::{Job, JobError, JobEvent, JobId, JobKind, Outcome};
use crownos_files_devices::udisks::DeviceError;
use crownui::kit::Toast;

use crate::{
    Stage,
    fixture::{Fixture, LAUNCH},
    sidebar::{STICK, plug_devices},
    window::{Window, volume},
};

const NOTICES: &str = "Notices & progress";
const GB: u64 = 1_000_000_000;
/// Long enough that the toast is still up when the frame settles and is captured.
const TOAST_HOLD: Duration = Duration::from_secs(3_600);
const COPIED: [&str; 3] = ["Renders", "Soundtrack", "launch-film.webm"];
const TRASHED: [&str; 4] = [
    "benchmarks.csv",
    "compositor.rs",
    "palette.svg",
    "release-notes.md",
];

pub fn capture(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    copy_progress(stage, carrier)?;
    toast(stage, carrier)?;
    permission(stage, carrier)?;
    drive_busy(stage, carrier)
}

/// A copy into Downloads well under way and a trash just started, as the job scheduler reports.
pub fn start_jobs(window: &mut Window, fixture: &Fixture) {
    let launch = fixture.path(LAUNCH);
    let copied: Vec<PathBuf> = COPIED.iter().map(|name| launch.join(name)).collect();
    let trashed: Vec<PathBuf> = TRASHED.iter().map(|name| launch.join(name)).collect();
    let downloads = fixture.path("Downloads");
    let (copy, trash) = (JobId::next(), JobId::next());
    let current = launch.join("Renders").join("hero-shot-04.exr");
    window.act(|cx, store| {
        submit(
            cx,
            store,
            Job::copy_into(copied, &downloads),
            Purpose::Plain,
        );
        submit(cx, store, Job::Trash(trashed.clone()), Purpose::Plain);
        let events = [
            JobEvent::Started {
                id: copy,
                kind: JobKind::Copy,
                total_bytes: 5_984 * GB / 1_000,
                total_items: 3,
            },
            JobEvent::Progress {
                id: copy,
                done_bytes: 3_700 * GB / 1_000,
                done_items: 1,
                current,
            },
            JobEvent::Started {
                id: trash,
                kind: JobKind::Trash,
                total_bytes: 0,
                total_items: 4,
            },
            JobEvent::Progress {
                id: trash,
                done_bytes: 0,
                done_items: 1,
                current: trashed[0].clone(),
            },
        ];
        let updates = [JobUpdate::Submitted(copy), JobUpdate::Submitted(trash)]
            .into_iter()
            .chain(events.into_iter().map(JobUpdate::Event))
            .collect();
        finish_jobs(cx, store, updates);
    });
}

fn copy_progress(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    start_jobs(&mut window, &stage.fixture);
    window.capture(
        carrier,
        NOTICES,
        "Jobs running",
        "A progress ring in the toolbar while files are copied and trashed",
    )
}

fn toast(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    let message = done_message(JobKind::Trash, 1);
    window.act(|cx, store| {
        store
            .toasts
            .show_with(cx, Toast::new(message).duration(TOAST_HOLD));
    });
    window.capture(
        carrier,
        NOTICES,
        "Toast",
        "What a finished job says, at the bottom of the window",
    )
}

fn permission(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    let photo = stage.fixture.path("Downloads").join("photo.jpg");
    let destination = PathBuf::from("/opt");
    let refused = destination.join("photo.jpg");
    let id = JobId::next();
    window.act(|cx, store| {
        submit(
            cx,
            store,
            Job::copy_into([photo], &destination),
            Purpose::Plain,
        );
        let updates = vec![
            JobUpdate::Submitted(id),
            JobUpdate::Event(JobEvent::Failed {
                id,
                error: JobError::Io {
                    path: refused,
                    source: io::Error::from_raw_os_error(rustix::io::Errno::ACCESS.raw_os_error()),
                },
                needs_privilege: true,
            }),
            JobUpdate::Event(JobEvent::Finished {
                id,
                outcome: Outcome::Failed,
                undo: None,
            }),
        ];
        finish_jobs(cx, store, updates);
    });
    window.capture(
        carrier,
        NOTICES,
        "Permission needed",
        "A copy was refused: the callout offers to run it again as root",
    )
}

fn drive_busy(stage: &Stage, carrier: &mut Carrier) -> anyhow::Result<()> {
    let mut window = stage.window(LAUNCH)?;
    plug_devices(&mut window);
    let busy = DriveEvent::Finished {
        volume: volume("sdb1", STICK, true).object_path,
        name: STICK.to_owned(),
        action: DriveAction::Eject,
        result: Err(DeviceError::Busy),
    };
    window.act(|cx, store| finish_drives(cx, store, vec![busy]));
    window.capture(
        carrier,
        NOTICES,
        "Drive in use",
        "Ejecting failed because files on the drive are still open",
    )
}
