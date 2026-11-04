//! The landing page's Files window as data: Documents › Launch with its 16
//! files, a phone and a drive, and a disk 58% full.

use std::{fs, path::Path, sync::Arc};

use crownconfig::{Files, TagColor};
use crownui::prelude::Cx;
use tempfile::TempDir;

use crate::{
    app::listing::publish,
    model::{
        device_entry::{DeviceEntry, DeviceKind},
        dir_stamp::DirStamp,
        disk_usage::DiskUsage,
        entry::{Entry, EntryExtra},
        files_op::FilesOp,
        places::Places,
        special_dir::SpecialDirs,
        stat::{Stat, StatFlags},
        ticket::Ticket,
        timestamp::Timestamp,
    },
    state::Store,
    util::{friendly_date::Today, local_time::LocalDateTime},
};

/// Nov 4, 2026, 12:00 UTC.
const NOON_UTC: i64 = 1_793_793_600;
const NOV_4: u8 = 4;
const DAY: i64 = 86_400;
const HOUR: i64 = 3_600;
const MINUTE: i64 = 60;
const KB: u64 = 1_000;
const MB: u64 = 1_000_000;
const GB: u64 = 1_000_000_000;
const DIRECTORY: u16 = 0o040755;
const FILE: u16 = 0o100644;

pub const LAUNCH: &str = "Documents/Launch";

/// `(name, bytes, created, modified)`, times as offsets from [`local_noon`]; folders end in `/`.
const MOCKUP: [(&str, u64, i64, i64); 17] = [
    ("Brand/", 186 * MB, -51 * DAY, -DAY + 6 * HOUR + 20 * MINUTE),
    ("Renders/", 4_800 * MB, -45 * DAY, -4 * HOUR),
    ("Press Kit/", 312 * MB, -33 * DAY, -5 * DAY),
    ("Soundtrack/", 1_100 * MB, -29 * DAY, -3 * DAY),
    (
        "aurora-wallpaper.avif",
        3_800 * KB,
        -7 * DAY,
        -3 * HOUR + 14 * MINUTE,
    ),
    ("crownbook-hero.png", 6_100 * KB, -14 * DAY, -6 * DAY),
    (
        "launch-film.webm",
        84_200 * KB,
        -3 * DAY,
        -DAY + 10 * HOUR + 47 * MINUTE,
    ),
    ("crownos-2026.11-x86_64.iso", 2_400 * MB, -DAY, -DAY),
    ("keynote.pdf", 18_400 * KB, -5 * DAY, -4 * HOUR),
    ("release-notes.md", 12 * KB, -4 * DAY, -3 * HOUR),
    ("compositor.rs", 48 * KB, -84 * DAY, -3 * HOUR),
    ("palette.svg", 4 * KB, -47 * DAY, -23 * DAY),
    ("benchmarks.csv", 220 * KB, -8 * DAY, -7 * DAY),
    ("interview.flac", 42_600 * KB, -11 * DAY, -11 * DAY),
    ("launch-assets.tar.zst", 640 * MB, -2 * DAY, -2 * DAY),
    ("launch-plan.odt", 86 * KB, -35 * DAY, -2 * DAY),
    (".git/", 0, -84 * DAY, -HOUR),
];

/// A home with the folders the sidebar lists, as real directories.
pub fn mockup_home() -> TempDir {
    let home = tempfile::tempdir().unwrap();
    for dir in [
        LAUNCH,
        "Documents/Launch/Renders",
        "Documents/Launch/Soundtrack",
        "Documents/Launch/Press Kit",
        "Desktop",
        "Downloads",
        "Pictures",
        "Videos",
        "Music",
        "Trash/files",
    ] {
        fs::create_dir_all(home.path().join(dir)).unwrap();
    }
    home
}

/// The places of `home`, with the mockup's marked folders as user directories.
pub fn mockup_places(home: &Path) -> Places {
    let user_dirs = format!(
        "XDG_PICTURES_DIR=\"$HOME/{LAUNCH}/Renders\"\n\
         XDG_MUSIC_DIR=\"$HOME/{LAUNCH}/Soundtrack\"\n\
         XDG_PUBLICSHARE_DIR=\"$HOME/{LAUNCH}/Press Kit\"\n"
    );
    Places {
        home: Some(home.to_path_buf()),
        special: SpecialDirs::from_user_dirs(&user_dirs, home),
        trash: Some(home.join("Trash/files")),
    }
}

pub fn mockup_store(cx: &mut Cx, home: &Path) -> Store {
    let dir: Arc<Path> = home.join(LAUNCH).into();
    let store = Store::new(
        cx,
        dir.clone(),
        &Files::default(),
        mockup_places(home),
        Today::at(local_noon()),
    );
    let ticket = Ticket::next();
    let stamp = DirStamp {
        modified: Timestamp::UNKNOWN,
        changed: Timestamp::UNKNOWN,
        len: 0,
        inode: 1,
        device: 1,
    };
    store.folder.update(cx, |folder| {
        folder.apply(FilesOp::Part {
            dir: dir.clone(),
            ticket,
            entries: MOCKUP.iter().map(|&row| entry(row)).collect(),
        });
        folder.apply(FilesOp::Done { dir, ticket, stamp });
    });
    store.storage.set(
        cx,
        Some(DiskUsage {
            total: 1_000 * GB,
            available: 412 * GB,
        }),
    );
    store.devices.set(
        cx,
        vec![
            DeviceEntry {
                name: "CrownBook".to_owned(),
                kind: DeviceKind::Drive,
                mount: Some(home.into()),
                online: true,
            },
            DeviceEntry {
                name: "Pixel 9 Pro".to_owned(),
                kind: DeviceKind::Phone,
                mount: None,
                online: true,
            },
        ],
    );
    store.tags.set(
        cx,
        vec![
            TagColor {
                name: "Brand".to_owned(),
                rgb: 0x8f6dfb,
            },
            TagColor {
                name: "Final".to_owned(),
                rgb: 0x4ade80,
            },
        ],
    );
    publish(cx, store);
    store
}

/// Nov 4, 2026, 12:00 on this machine's clock, so the mockup's "Today, 09:14"
/// reads the same in every time zone.
fn local_noon() -> i64 {
    let local = LocalDateTime::from_unix(NOON_UTC).unwrap();
    let days = i64::from(local.day) - i64::from(NOV_4);
    let clock = i64::from(local.hour) * HOUR + i64::from(local.minute) * MINUTE;
    NOON_UTC - (days * DAY + clock - 12 * HOUR)
}

fn entry((name, len, created, modified): (&str, u64, i64, i64)) -> Entry {
    let is_dir = name.ends_with('/');
    let name = name.trim_end_matches('/');
    let flags = if name.starts_with('.') {
        StatFlags::HIDDEN
    } else {
        StatFlags::empty()
    };
    let at = |offset| Timestamp::from_parts(local_noon() + offset, 0);
    Entry::new(
        name.into(),
        Stat {
            len,
            modified: at(modified),
            created: at(created),
            mode: if is_dir { DIRECTORY } else { FILE },
            flags,
        },
        EntryExtra::default(),
    )
}
