use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use crownos_files::util::local_time::LocalDateTime;
use crownos_files_core::model::{
    entry::{Entry, EntryExtra},
    places::Places,
    special_dir::SpecialDirs,
    stat::{Stat, StatFlags},
    timestamp::Timestamp,
    trash::{TrashInfo, TrashedItem},
};

use crate::picture::{Picture, write_png};

/// Nov 4, 2026, 12:00 UTC: the mockup's "today".
const NOON_UTC: i64 = 1_793_793_600;
const NOV_4: u8 = 4;
const DAY: i64 = 86_400;
const HOUR: i64 = 3_600;
const MINUTE: i64 = 60;
const KB: u64 = 1_000;
const MB: u64 = 1_000_000;
const DIRECTORY: u16 = 0o040755;
const FILE: u16 = 0o100644;

pub const USER: &str = "alice";
pub const LAUNCH: &str = "Documents/Launch";
pub const EMPTY_FOLDER: &str = "Desktop";
/// The folder a disk is mounted on, whose properties show its capacity.
pub const MOUNTED_FOLDER: &str = LAUNCH;
pub const TRASH_CAN: &str = ".local/share/Trash";

/// `(name, bytes, created, modified)`, times as offsets from noon of "today"; folders end in `/`.
type Row = (&'static str, u64, i64, i64);

const HOME: &[Row] = &[
    ("Desktop/", 0, -400 * DAY, -2 * DAY),
    ("Documents/", 0, -400 * DAY, -HOUR),
    ("Downloads/", 0, -400 * DAY, -2 * HOUR),
    ("Music/", 0, -400 * DAY, -11 * DAY),
    ("Pictures/", 0, -400 * DAY, -DAY),
    ("Public/", 0, -400 * DAY, -90 * DAY),
    ("Videos/", 0, -400 * DAY, -3 * DAY),
    ("notes.md", 3 * KB, -20 * DAY, -5 * HOUR),
    (".bashrc", 4 * KB, -400 * DAY, -60 * DAY),
];

const DOCUMENTS: &[Row] = &[
    ("Launch/", 7_398 * MB, -84 * DAY, -HOUR),
    ("Invoices/", 0, -200 * DAY, -9 * DAY),
    ("Taxes 2026/", 0, -150 * DAY, -40 * DAY),
    ("budget-2026.ods", 64 * KB, -120 * DAY, -6 * DAY),
    ("contract-signed.pdf", 1_200 * KB, -30 * DAY, -30 * DAY),
    ("meeting-notes.md", 9 * KB, -2 * DAY, -2 * HOUR),
    ("resume.pdf", 240 * KB, -300 * DAY, -45 * DAY),
    ("roadmap.odt", 112 * KB, -60 * DAY, -DAY),
];

const LAUNCH_ROWS: &[Row] = &[
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

const DOWNLOADS: &[Row] = &[
    ("Inter-4.1/", 38 * MB, -9 * DAY, -9 * DAY),
    ("crownos-2026.11-x86_64.iso", 2_400 * MB, -DAY, -DAY),
    ("figma-export.zip", 24_800 * KB, -2 * DAY, -2 * HOUR),
    ("invoice-october.pdf", 182 * KB, -8 * DAY, -8 * DAY),
    ("photo.jpg", 4_200 * KB, -4 * HOUR, -4 * HOUR),
    ("rustup-init.sh", 24 * KB, -60 * DAY, -60 * DAY),
    ("setup.AppImage", 96 * MB, -15 * DAY, -15 * DAY),
    ("boarding-pass.pdf", 96 * KB, -3 * DAY, -3 * DAY),
];

const PICTURES: &[Row] = &[
    ("Screenshots/", 412 * MB, -300 * DAY, -2 * HOUR),
    ("Wallpapers/", 1_900 * MB, -300 * DAY, -12 * DAY),
    ("alpine-lake.jpg", 5_400 * KB, -20 * DAY, -20 * DAY),
    ("aurora.png", 7_200 * KB, -7 * DAY, -7 * DAY),
    ("desert-dunes.jpg", 4_900 * KB, -33 * DAY, -33 * DAY),
    ("harbor-dusk.jpg", 6_100 * KB, -2 * DAY, -2 * DAY),
    ("lavender-field.jpg", 3_800 * KB, -60 * DAY, -60 * DAY),
    ("misty-forest.png", 8_300 * KB, -14 * DAY, -14 * DAY),
    ("sunset-coast.jpg", 4_400 * KB, -DAY, -DAY),
    ("glacier.webp", 2_100 * KB, -90 * DAY, -90 * DAY),
];

const VIDEOS: &[Row] = &[
    ("Screencasts/", 2_300 * MB, -100 * DAY, -4 * DAY),
    ("launch-film.webm", 84_200 * KB, -3 * DAY, -3 * DAY),
    ("keynote-rehearsal.mp4", 612 * MB, -6 * DAY, -6 * DAY),
    ("timelapse-harbor.mkv", 1_800 * MB, -40 * DAY, -40 * DAY),
];

const MUSIC: &[Row] = &[
    ("Soundtrack/", 1_100 * MB, -29 * DAY, -3 * DAY),
    ("ambient-loop.ogg", 8_200 * KB, -50 * DAY, -50 * DAY),
    ("interview.flac", 42_600 * KB, -11 * DAY, -11 * DAY),
    ("night-drive.mp3", 9_400 * KB, -120 * DAY, -120 * DAY),
    ("piano-sketch.wav", 31_000 * KB, -5 * DAY, -5 * DAY),
];

/// Every listed folder, relative to home.
const FOLDERS: &[(&str, &[Row])] = &[
    ("", HOME),
    ("Documents", DOCUMENTS),
    (LAUNCH, LAUNCH_ROWS),
    ("Downloads", DOWNLOADS),
    ("Pictures", PICTURES),
    ("Videos", VIDEOS),
    ("Music", MUSIC),
    (EMPTY_FOLDER, &[]),
];

/// Recently used files, relative to home.
pub const RECENTS: &[&str] = &[
    "Documents/Launch/keynote.pdf",
    "Documents/Launch/release-notes.md",
    "Downloads/photo.jpg",
    "Documents/meeting-notes.md",
    "Pictures/harbor-dusk.jpg",
    "Documents/roadmap.odt",
    "Documents/Launch/aurora-wallpaper.avif",
    "Videos/launch-film.webm",
];

/// `(name in the trash, where it came from, bytes, deleted)`.
const TRASHED: &[(&str, &str, u64, i64)] = &[
    ("draft one.md", "Documents", 6 * KB, -2 * DAY),
    ("old-logo.svg", "Documents/Launch/Brand", 18 * KB, -5 * DAY),
    ("Old Mockups", "Documents/Launch", 86 * MB, -9 * DAY),
    (
        "screenshot-0412.png",
        "Pictures/Screenshots",
        1_400 * KB,
        -DAY,
    ),
];

/// The fake home the window browses, written to disk under the capture output.
pub struct Fixture {
    pub home: PathBuf,
    pub noon: i64,
}

impl Fixture {
    pub fn create(root: &Path) -> anyhow::Result<Self> {
        let home = root.join("fixture").join(USER);
        if home.exists() {
            fs::remove_dir_all(&home)?;
        }
        let fixture = Self {
            home,
            noon: local_noon(),
        };
        for &(folder, rows) in FOLDERS {
            let dir = fixture.home.join(folder);
            fs::create_dir_all(&dir)?;
            for &(name, ..) in rows {
                fixture.write_row(&dir, name)?;
            }
        }
        let trash = fixture.home.join(TRASH_CAN);
        fs::create_dir_all(trash.join("info"))?;
        for &(name, ..) in TRASHED {
            fs::write(trash.join("files").join(name), [])
                .or_else(|_| fs::create_dir_all(trash.join("files").join(name)))?;
        }
        Ok(fixture)
    }

    fn write_row(&self, dir: &Path, name: &str) -> anyhow::Result<()> {
        if let Some(folder) = name.strip_suffix('/') {
            fs::create_dir_all(dir.join(folder))?;
            return Ok(());
        }
        let path = dir.join(name);
        match Picture::for_name(name) {
            Some(picture) if is_still(name) => write_png(&path, picture)?,
            _ => fs::write(path, [])?,
        }
        Ok(())
    }

    pub fn path(&self, relative: &str) -> Arc<Path> {
        if relative.is_empty() {
            return self.home.as_path().into();
        }
        self.home.join(relative).into()
    }

    pub fn places(&self) -> Places {
        Places {
            home: Some(self.home.clone()),
            special: SpecialDirs::from_user_dirs("", &self.home),
            trash: Some(self.home.join(TRASH_CAN).join("files")),
        }
    }

    /// The entries of `dir` as the engine would list them, or `None` for a folder it can't list.
    pub fn listing(&self, dir: &Path) -> Option<Vec<Entry>> {
        let relative = dir.strip_prefix(&self.home).ok()?.to_str()?;
        let (_, rows) = FOLDERS.iter().find(|(folder, _)| *folder == relative)?;
        Some(rows.iter().map(|&row| self.entry(row, None)).collect())
    }

    /// `relative` as an entry reachable from any folder, as search and Recents list it.
    pub fn absolute_entry(&self, relative: &str) -> Option<Entry> {
        let path = self.home.join(relative);
        let parent = path.parent()?.strip_prefix(&self.home).ok()?.to_str()?;
        let name = path.file_name()?.to_str()?;
        let (_, rows) = FOLDERS.iter().find(|(folder, _)| *folder == parent)?;
        let row = rows
            .iter()
            .find(|(row_name, ..)| row_name.trim_end_matches('/') == name)?;
        Some(self.entry(*row, Some(&path)))
    }

    pub fn trashed(&self) -> Vec<(Entry, TrashedItem)> {
        let can: Arc<Path> = self.home.join(TRASH_CAN).into();
        TRASHED
            .iter()
            .map(|&(name, from, bytes, deleted)| {
                let is_dir = !name.contains('.');
                let shown = if is_dir {
                    format!("{name}/")
                } else {
                    name.to_owned()
                };
                let row = (shown.as_str(), bytes, deleted, deleted);
                let in_can = can.join("files").join(name);
                let entry = self.entry(row, Some(&in_can));
                let item = TrashedItem {
                    trash: can.clone(),
                    name: name.into(),
                    info: TrashInfo {
                        original: self.home.join(from).join(name),
                        deleted_at: LocalDateTime::from_unix(self.noon + deleted),
                    },
                    is_dir,
                    size: Some(bytes),
                };
                (entry, item)
            })
            .collect()
    }

    /// The sizes of the folders in `dir`, as the engine measures them.
    pub fn folder_sizes(&self, dir: &Path) -> Vec<(compact_str::CompactString, u64)> {
        self.listing(dir)
            .unwrap_or_default()
            .into_iter()
            .filter(Entry::is_dir)
            .map(|entry| (entry.name.clone(), entry.stat.len))
            .collect()
    }

    fn entry(
        &self,
        (name, len, created, modified): (&str, u64, i64, i64),
        absolute: Option<&Path>,
    ) -> Entry {
        let is_dir = name.ends_with('/');
        let name = name.trim_end_matches('/');
        let flags = if name.starts_with('.') {
            StatFlags::HIDDEN
        } else {
            StatFlags::empty()
        };
        let at = |offset| Timestamp::from_parts(self.noon + offset, 0);
        let extra = absolute.map_or_else(EntryExtra::default, |path| {
            EntryExtra::new(None, Some(path.as_os_str().into()))
        });
        Entry::new(
            name.into(),
            Stat {
                len,
                modified: at(modified),
                created: at(created),
                mode: if is_dir { DIRECTORY } else { FILE },
                flags,
            },
            extra,
        )
    }
}

fn is_still(name: &str) -> bool {
    [".png", ".jpg", ".webp", ".avif"]
        .iter()
        .any(|extension| name.ends_with(extension))
}

/// Nov 4, 2026, 12:00 on this machine's clock, so "Today, 09:14" reads the same in every time zone.
fn local_noon() -> i64 {
    let Some(local) = LocalDateTime::from_unix(NOON_UTC) else {
        return NOON_UTC;
    };
    let days = i64::from(local.day) - i64::from(NOV_4);
    let clock = i64::from(local.hour) * HOUR + i64::from(local.minute) * MINUTE;
    NOON_UTC - (days * DAY + clock - 12 * HOUR)
}
