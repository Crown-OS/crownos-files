//! The storage card and the status bar: the disk of the current folder, and
//! the user folders' share of it when home is on that disk.

use std::{path::Path, sync::Arc};

use crownos_files_devices::storage::{SpaceUsage, StorageBreakdown};
use crowntest::text;

use super::window::{Window, drain, window};
use crate::app::{
    fixtures::LAUNCH,
    storage::{DiskSpace, Share, finish_breakdown, shares},
    tasks::{TaskDone, finish_tasks},
};

const GB: u64 = 1_000_000_000;

fn space(holds_home: bool) -> DiskSpace {
    DiskSpace {
        usage: SpaceUsage {
            total: 1_000 * GB,
            free: 412 * GB,
            available: 400 * GB,
        },
        holds_home,
    }
}

/// Documents, Downloads, Pictures, Music, Videos, Desktop.
fn breakdown(sizes: [u64; 6]) -> StorageBreakdown {
    StorageBreakdown {
        sizes: sizes.map(|size| size * GB),
    }
}

fn measured(window: &mut Window, space: DiskSpace) {
    let dir: Arc<Path> = window.dir(LAUNCH);
    window.answer(|cx, store| finish_tasks(cx, store, vec![TaskDone::Measured(dir, space)]));
}

#[test]
fn the_bar_splits_used_space_into_media_documents_and_the_rest() {
    let folders = breakdown([100, 50, 200, 40, 100, 10]);
    assert_eq!(
        shares(space(true), Some(&folders)),
        [
            (Share::Media, 340 * GB),
            (Share::Documents, 160 * GB),
            (Share::Other, 88 * GB),
            (Share::Free, 412 * GB),
        ]
    );
    assert_eq!(
        shares(space(false), Some(&folders)),
        [(Share::Used, 588 * GB), (Share::Free, 412 * GB)],
        "another disk is not broken down by the home folders"
    );
    assert_eq!(
        shares(space(true), None),
        [(Share::Used, 588 * GB), (Share::Free, 412 * GB)]
    );
    let overfull = breakdown([900, 0, 0, 0, 900, 0]);
    let split = shares(space(true), Some(&overfull));
    assert_eq!(
        split.iter().map(|(_, bytes)| bytes).sum::<u64>(),
        1_000 * GB
    );
    assert_eq!(split[0], (Share::Media, 588 * GB));
}

#[test]
fn the_card_shows_the_disk_of_the_folder() {
    let mut window = window();
    assert!(window.shows(text("588 GB of 1 TB used")));
    measured(&mut window, space(false));
    assert!(window.shows(text("588 GB of 1 TB used")));
    assert!(
        drain(&mut window.workers.breakdown).is_empty(),
        "the breakdown is only for the disk home is on"
    );
    measured(&mut window, space(true));
    assert_eq!(drain(&mut window.workers.breakdown).len(), 1);
    let storage = window.store.storage;
    let folders = breakdown([1, 2, 3, 4, 5, 6]);
    window.answer(|cx, _| {
        finish_breakdown(
            cx,
            storage,
            vec![StorageBreakdown::default(), folders.clone()],
        )
    });
    let shown = window.answer(|cx, store| store.storage.breakdown.get(cx));
    assert_eq!(shown, Some(folders));
}
