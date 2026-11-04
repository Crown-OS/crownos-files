//! The disk of the current folder for the storage card and the status bar,
//! and how the user folders fill it, measured now and then off the UI thread.

use std::time::{Duration, Instant};

use crownos_files_devices::storage::{SpaceUsage, StorageBreakdown, measure_breakdown};
use crownui::prelude::{Cx, Runtime, Signal, Store as Cell};
use tokio::{
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    task::spawn_blocking,
};
use tokio_util::sync::CancellationToken;

use crate::model::special_dir::{SpecialDir, SpecialDirs};

/// The breakdown walks every user folder, so it runs at most this often.
const REMEASURE_AFTER: Duration = Duration::from_secs(120);
const MEDIA: [SpecialDir; 3] = [SpecialDir::Pictures, SpecialDir::Videos, SpecialDir::Music];
const DOCUMENTS: [SpecialDir; 3] = [
    SpecialDir::Documents,
    SpecialDir::Downloads,
    SpecialDir::Desktop,
];

/// The disk the current folder is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiskSpace {
    pub usage: SpaceUsage,
    /// The home folder is on this disk too, so the breakdown describes it.
    pub holds_home: bool,
}

/// A run of the storage card's bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Share {
    /// Everything used, when the user folders are not on this disk.
    Used,
    Media,
    Documents,
    Other,
    Free,
}

#[derive(Debug, Clone, Copy)]
pub struct MeasureBreakdown;

#[derive(Clone, Copy)]
pub struct Storage {
    pub space: Signal<Option<DiskSpace>>,
    pub breakdown: Signal<Option<StorageBreakdown>>,
    pub sender: Cell<Option<UnboundedSender<MeasureBreakdown>>>,
}

impl Storage {
    pub fn new(cx: &mut Cx) -> Self {
        Self {
            space: cx.signal(None),
            breakdown: cx.signal(None),
            sender: cx.store(None),
        }
    }

    pub fn request_breakdown(self, runtime: &Runtime) {
        self.sender.with(runtime, |sender| {
            if let Some(sender) = sender {
                let _ = sender.send(MeasureBreakdown);
            }
        });
    }

    /// Adopts the space of the disk the current folder is on.
    pub fn show(self, runtime: &mut Runtime, space: DiskSpace) {
        if self.space.get(runtime) != Some(space) {
            self.space.set(runtime, Some(space));
        }
        if space.holds_home {
            self.request_breakdown(runtime);
        }
    }
}

/// Measures the user folders when asked, at most every [`REMEASURE_AFTER`],
/// reporting the running breakdown as the walk goes.
pub async fn run_breakdown(
    dirs: SpecialDirs,
    breakdowns: UnboundedSender<StorageBreakdown>,
    mut requests: UnboundedReceiver<MeasureBreakdown>,
) {
    let cancel = CancellationToken::new();
    let _stop_on_close = cancel.clone().drop_guard();
    let mut last_run: Option<Instant> = None;
    while requests.recv().await.is_some() {
        if last_run.is_some_and(|at| at.elapsed() < REMEASURE_AFTER) {
            continue;
        }
        last_run = Some(Instant::now());
        let (dirs, breakdowns, cancel) = (dirs.clone(), breakdowns.clone(), cancel.clone());
        let walk = spawn_blocking(move || {
            measure_breakdown(&dirs, &cancel, |breakdown| {
                let _ = breakdowns.send(breakdown.clone());
            });
        });
        if walk.await.is_err() {
            return;
        }
    }
}

pub fn finish_breakdown(cx: &mut Cx, storage: Storage, burst: Vec<StorageBreakdown>) {
    if let Some(latest) = burst.into_iter().last() {
        storage.breakdown.set(cx, Some(latest));
    }
}

/// The runs of the storage card's bar, in bytes: what the user folders hold
/// when they are on this disk, the rest of the used space, and the free space.
pub fn shares(space: DiskSpace, breakdown: Option<&StorageBreakdown>) -> Vec<(Share, u64)> {
    let usage = space.usage;
    let free = (Share::Free, usage.total.saturating_sub(usage.used()));
    let Some(breakdown) = breakdown.filter(|_| space.holds_home) else {
        return vec![(Share::Used, usage.used()), free];
    };
    let sum =
        |dirs: [SpecialDir; 3]| -> u64 { dirs.into_iter().map(|dir| breakdown.get(dir)).sum() };
    let media = sum(MEDIA).min(usage.used());
    let documents = sum(DOCUMENTS).min(usage.used() - media);
    vec![
        (Share::Media, media),
        (Share::Documents, documents),
        (Share::Other, usage.used() - media - documents),
        free,
    ]
}
