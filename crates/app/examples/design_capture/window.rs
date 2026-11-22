use std::{cell::Cell as Slot, path::Path, rc::Rc, time::Duration};

use capture::{CaptureApp, Carrier};
use compact_str::CompactString;
use crownconfig::{Files, TagColor, load_appearance};
use crownos_files::{
    app::{
        drive_worker::DriveCommand,
        drives::DriveRow,
        file_jobs::JobCommand,
        listing::apply_ops,
        navigation::load_current,
        opener::{AppChoice, AppChoices, OpenCommand, OpenEvent},
        opening::finish_opening,
        peers::{Peer, PeerCommand, PeerId, PeerKind},
        properties::FileFacts,
        quick_look::QuickLookCommand,
        shell::shell,
        storage::{DiskSpace, MeasureBreakdown, finish_breakdown},
        tasks::{Task, TaskDone, finish_tasks},
        thumbnails::{ThumbCommand, finish_thumbnails},
        virtual_folder::{VirtualFolder, VirtualListing},
    },
    state::Store,
    util::friendly_date::Today,
};
use crownos_files_core::{
    fs::engine::EngineCmd,
    model::{dir_stamp::DirStamp, files_op::FilesOp, ticket::Ticket, timestamp::Timestamp},
    thumbs::{Thumbnail, ThumbnailEvent},
};
use crownos_files_devices::{
    storage::{MEASURED_DIRS, SpaceUsage, StorageBreakdown},
    tags::Tags,
    udisks::{Drive, Hints, OwnedObjectPath, Volume},
};
use crowntest::{Query, Role, role};
use crownui::{
    AppConfig,
    kit::{WindowRadius, provide_configured_theme, provide_window_radius},
    prelude::{Cx, component},
};
use rustix::io::Errno;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

use crate::{
    fixture::{Fixture, MOUNTED_FOLDER},
    picture::{Picture, app_icon},
};

const FRAME_MILLIS: u64 = 16;
const MAX_ROUNDS: usize = 12;
const GB: u64 = 1_000_000_000;
const DISK: SpaceUsage = SpaceUsage {
    total: 1_000 * GB,
    free: 412 * GB,
    available: 412 * GB,
};
/// Documents, Downloads, Pictures, Music, Videos and Public, in `MEASURED_DIRS` order.
const BREAKDOWN: [u64; MEASURED_DIRS.len()] =
    [96 * GB, 38 * GB, 74 * GB, 41 * GB, 182 * GB, 2 * GB];
const VIDEO_LENGTH: Duration = Duration::from_secs(84);
/// Where the pointer rests while a frame is captured: the empty corner of the sidebar header.
const POINTER_REST: (f32, f32) = (2.0, 2.0);

const TAGGED: &[(&str, &[&str])] = &[
    ("keynote.pdf", &["Final"]),
    ("Brand", &["Brand"]),
    ("palette.svg", &["Brand"]),
    ("launch-film.webm", &["Final", "Brand"]),
    ("aurora-wallpaper.avif", &["Brand", "Review"]),
    ("release-notes.md", &["Review"]),
];

/// Answers the window would get from a worker, held back to show the state before it.
#[derive(Debug, Clone, Copy, Default)]
pub struct Held {
    pub listing: bool,
    pub inspection: bool,
    pub folder_size: bool,
    pub menu_apps: bool,
}

struct Workers {
    engine: UnboundedReceiver<EngineCmd>,
    tasks: UnboundedReceiver<Task>,
    thumbs: UnboundedReceiver<ThumbCommand>,
    opener: UnboundedReceiver<OpenCommand>,
    viewer: UnboundedReceiver<QuickLookCommand>,
    jobs: UnboundedReceiver<JobCommand>,
    drives: UnboundedReceiver<DriveCommand>,
    peers: UnboundedReceiver<PeerCommand>,
    breakdown: UnboundedReceiver<MeasureBreakdown>,
}

/// The real window over the fixture home, with this harness playing every worker.
pub struct Window<'f> {
    pub app: CaptureApp,
    pub store: Store,
    pub held: Held,
    workers: Workers,
    fixture: &'f Fixture,
}

impl<'f> Window<'f> {
    pub fn open(fixture: &'f Fixture, config: &AppConfig, start: &str) -> anyhow::Result<Self> {
        Self::open_held(fixture, config, start, Held::default())
    }

    pub fn open_held(
        fixture: &'f Fixture,
        config: &AppConfig,
        start: &str,
        held: Held,
    ) -> anyhow::Result<Self> {
        let (engine, engine_commands) = unbounded_channel();
        let (tasks, task_commands) = unbounded_channel();
        let (thumbs, thumb_commands) = unbounded_channel();
        let (opener, open_commands) = unbounded_channel();
        let (viewer, viewer_commands) = unbounded_channel();
        let (jobs, job_commands) = unbounded_channel();
        let (drives, drive_commands) = unbounded_channel();
        let (peers, peer_commands) = unbounded_channel();
        let (breakdown, breakdown_requests) = unbounded_channel();
        let built = Rc::new(Slot::new(None));
        let slot = Rc::clone(&built);
        let dir = fixture.path(start);
        let places = fixture.places();
        let today = Today::at(fixture.noon);
        let internal = internal_drive(&fixture.home);
        let root = component(move |cx: &mut Cx| {
            provide_configured_theme(cx);
            provide_window_radius(cx, WindowRadius(f32::from(load_appearance().border_radius)));
            let store = Store::new(cx, dir, &Files::default(), places, today);
            store.engine.update(cx, |sender| *sender = Some(engine));
            store.tasks.update(cx, |sender| *sender = Some(tasks));
            store
                .thumbs
                .service
                .update(cx, |sender| *sender = Some(thumbs));
            store
                .opening
                .sender
                .update(cx, |sender| *sender = Some(opener));
            store
                .quick_look
                .sender
                .update(cx, |sender| *sender = Some(viewer));
            store.jobs.sender.update(cx, |sender| *sender = Some(jobs));
            store
                .drives
                .sender
                .update(cx, |sender| *sender = Some(drives));
            store
                .peers
                .sender
                .update(cx, |sender| *sender = Some(peers));
            store
                .storage
                .sender
                .update(cx, |sender| *sender = Some(breakdown));
            store.drives.rows.set(cx, vec![internal]);
            store.peers.paired.set(cx, vec![phone()]);
            store.tags.set(cx, tag_palette());
            load_current(cx, store);
            slot.set(Some(store));
            shell(cx, store)
        });
        let app = capture::mount_app(root, config, None)?;
        let store = built
            .take()
            .ok_or_else(|| anyhow::anyhow!("the window did not mount"))?;
        let mut window = Self {
            app,
            store,
            held,
            workers: Workers {
                engine: engine_commands,
                tasks: task_commands,
                thumbs: thumb_commands,
                opener: open_commands,
                viewer: viewer_commands,
                jobs: job_commands,
                drives: drive_commands,
                peers: peer_commands,
                breakdown: breakdown_requests,
            },
            fixture,
        };
        window.pump();
        Ok(window)
    }

    /// Runs `act` on the store, answers what it asked of the workers and settles.
    pub fn act<R>(&mut self, act: impl FnOnce(&mut Cx, Store) -> R) -> R {
        let store = self.store;
        let result = act(self.app.ui().cx(), store);
        self.pump();
        result
    }

    pub fn click(&mut self, query: Query) -> &mut Self {
        self.app.click(query);
        self.pump()
    }

    /// Clicks an entry with the pointer, so a menu opened next appears where it went down.
    pub fn click_entry(&mut self, name: &str) -> &mut Self {
        let center = self
            .app
            .find(role(Role::ListItem).name(name))
            .bounds
            .center();
        self.app
            .pointer_down(center.x, center.y)
            .pointer_up(center.x, center.y);
        self.app.focus(role(Role::List).name("Files"));
        self.pump()
    }

    pub fn key(&mut self, shortcut: &str) -> &mut Self {
        if self.app.ui().focused_semantic_id().is_none() {
            self.app.focus(role(Role::List).name("Files"));
        }
        self.app.press_key(shortcut);
        self.pump()
    }

    pub fn capture(
        &mut self,
        carrier: &mut Carrier,
        group: &str,
        name: &str,
        description: &str,
    ) -> anyhow::Result<()> {
        self.app.pointer_move(POINTER_REST.0, POINTER_REST.1);
        self.pump();
        self.capture_here(carrier, group, name, description)
    }

    /// Captures with the pointer left where the state put it, e.g. mid-drag.
    pub fn capture_here(
        &mut self,
        carrier: &mut Carrier,
        group: &str,
        name: &str,
        description: &str,
    ) -> anyhow::Result<()> {
        carrier.capture_state(&mut self.app, group, name, description)?;
        Ok(())
    }

    /// Answers every worker request until none is left, then settles.
    pub fn pump(&mut self) -> &mut Self {
        for _ in 0..MAX_ROUNDS {
            if !self.answer_round() {
                break;
            }
            self.app.advance_ms(FRAME_MILLIS);
        }
        self.app.settle();
        self
    }

    fn answer_round(&mut self) -> bool {
        let ops: Vec<FilesOp> = drain(&mut self.workers.engine)
            .into_iter()
            .flat_map(|command| self.engine_answer(command))
            .collect();
        let done: Vec<TaskDone> = drain(&mut self.workers.tasks)
            .into_iter()
            .filter_map(|task| self.task_answer(task))
            .collect();
        let thumbnails: Vec<ThumbnailEvent> = drain(&mut self.workers.thumbs)
            .into_iter()
            .filter_map(thumbnail_answer)
            .collect();
        let opened: Vec<OpenEvent> = drain(&mut self.workers.opener)
            .into_iter()
            .filter_map(|command| self.open_answer(command))
            .collect();
        let breakdowns: Vec<StorageBreakdown> = drain(&mut self.workers.breakdown)
            .into_iter()
            .map(|MeasureBreakdown| StorageBreakdown { sizes: BREAKDOWN })
            .collect();
        drain(&mut self.workers.viewer);
        drain(&mut self.workers.jobs);
        drain(&mut self.workers.drives);
        drain(&mut self.workers.peers);
        let answered = !(ops.is_empty()
            && done.is_empty()
            && thumbnails.is_empty()
            && opened.is_empty()
            && breakdowns.is_empty());
        let store = self.store;
        let cx = self.app.ui().cx();
        if !ops.is_empty() {
            apply_ops(cx, store, ops);
        }
        if !done.is_empty() {
            finish_tasks(cx, store, done);
        }
        if !thumbnails.is_empty() {
            finish_thumbnails(cx, store, thumbnails);
        }
        if !opened.is_empty() {
            finish_opening(cx, store, opened);
        }
        if !breakdowns.is_empty() {
            finish_breakdown(cx, store.storage, breakdowns);
        }
        answered
    }

    fn engine_answer(&self, command: EngineCmd) -> Vec<FilesOp> {
        let dir = match command {
            EngineCmd::Load(dir) | EngineCmd::Revalidate { dir, .. } => dir,
            EngineCmd::MeasureDirs { dir, .. } => {
                let sizes = self.fixture.folder_sizes(&dir);
                return vec![FilesOp::Size { dir, sizes }];
            }
            EngineCmd::Cancel(_) | EngineCmd::Watch(_) => return Vec::new(),
        };
        if self.held.listing {
            return Vec::new();
        }
        let ticket = Ticket::next();
        match self.fixture.listing(&dir) {
            Some(entries) => vec![
                FilesOp::Part {
                    dir: dir.clone(),
                    ticket,
                    entries,
                },
                FilesOp::Done {
                    dir,
                    ticket,
                    stamp: stamp(),
                },
            ],
            None => vec![FilesOp::Fail {
                dir,
                ticket,
                error: Errno::ACCESS,
            }],
        }
    }

    fn task_answer(&self, task: Task) -> Option<TaskDone> {
        match task {
            Task::MeasureDisk { dir, .. } => Some(TaskDone::Measured(
                dir,
                DiskSpace {
                    usage: DISK,
                    holds_home: true,
                },
            )),
            Task::ListVirtual(folder) => Some(TaskDone::Listed(self.virtual_listing(folder))),
            Task::ReadTags {
                dir,
                generation,
                entries,
            } => Some(TaskDone::TagsRead {
                dir,
                generation,
                tags: entries
                    .into_iter()
                    .filter_map(|(name, _)| Some((name.clone(), tags_of(&name)?)))
                    .collect(),
            }),
            Task::Inspect(path) if !self.held.inspection => {
                let facts = self.facts(&path);
                Some(TaskDone::Inspected(path, facts))
            }
            Task::MeasureFolder { path, .. } if !self.held.folder_size => {
                let bytes = self.size_of(&path);
                Some(TaskDone::FolderMeasured(path, bytes))
            }
            _ => None,
        }
    }

    fn virtual_listing(&self, folder: VirtualFolder) -> VirtualListing {
        let (entries, trashed) = match folder {
            VirtualFolder::Trash => self.fixture.trashed().into_iter().unzip(),
            VirtualFolder::Recents => (
                crate::fixture::RECENTS
                    .iter()
                    .filter_map(|relative| self.fixture.absolute_entry(relative))
                    .collect(),
                Vec::new(),
            ),
            VirtualFolder::Search => (Vec::new(), Vec::new()),
        };
        VirtualListing {
            folder,
            entries,
            trashed,
        }
    }

    fn size_of(&self, path: &Path) -> u64 {
        let name = path.file_name().and_then(|name| name.to_str());
        path.parent()
            .and_then(|parent| self.fixture.listing(parent))
            .and_then(|entries| {
                entries
                    .into_iter()
                    .find(|entry| Some(entry.name.as_str()) == name)
            })
            .map_or(0, |entry| entry.stat.len)
    }

    fn facts(&self, path: &Path) -> FileFacts {
        let at = |offset| Timestamp::from_parts(self.fixture.noon + offset, 0);
        let is_dir = path.extension().is_none();
        FileFacts {
            created: at(-5 * 86_400),
            modified: at(-4 * 3_600),
            accessed: at(-20 * 60),
            mode: if is_dir { 0o040755 } else { 0o100644 },
            owner: crate::fixture::USER.to_owned(),
            group: "users".to_owned(),
            len: self.size_of(path),
            volume: (*path == *self.fixture.path(MOUNTED_FOLDER)).then_some(DISK),
        }
    }

    fn open_answer(&self, command: OpenCommand) -> Option<OpenEvent> {
        let OpenCommand::Apps {
            targets, purpose, ..
        } = command
        else {
            return None;
        };
        if self.held.menu_apps {
            return None;
        }
        let first = targets.first()?;
        let name = first.path.file_name()?.to_string_lossy().into_owned();
        let apps = applications_for(&name)
            .iter()
            .enumerate()
            .map(|(index, app)| AppChoice {
                id: format!("org.example.{}.desktop", app.replace(' ', "")).into(),
                name: (*app).to_owned(),
                icon: app_icon(index),
            })
            .collect();
        Some(OpenEvent::Apps(AppChoices {
            mime: first.mime,
            files: targets.into_iter().map(|target| target.path).collect(),
            apps,
            purpose,
        }))
    }
}

fn applications_for(name: &str) -> &'static [&'static str] {
    if Picture::for_name(name).is_some() {
        return &[
            "Image Viewer",
            "Photo Studio",
            "Vector Studio",
            "Web Browser",
        ];
    }
    if name.ends_with(".zst") || name.ends_with(".zip") {
        return &["Archive Manager", "Text Editor"];
    }
    &[
        "Document Viewer",
        "Text Editor",
        "Office Writer",
        "Web Browser",
    ]
}

fn thumbnail_answer(command: ThumbCommand) -> Option<ThumbnailEvent> {
    let ThumbCommand::Request(key) = command else {
        return None;
    };
    let name = key.path.file_name()?.to_str()?;
    let picture = Picture::for_name(name)?;
    let is_video = [".webm", ".mp4", ".mkv"]
        .iter()
        .any(|extension| name.ends_with(extension));
    let (width, height, rgba) = picture.render(key.size.edge());
    Some(ThumbnailEvent::Ready(Thumbnail {
        key,
        width,
        height,
        rgba,
        duration: is_video.then_some(VIDEO_LENGTH),
    }))
}

fn tags_of(name: &str) -> Option<Tags> {
    TAGGED
        .iter()
        .find(|(tagged, _)| *tagged == name)
        .map(|(_, tags)| tags.iter().map(|&tag| CompactString::from(tag)).collect())
}

fn tag_palette() -> Vec<TagColor> {
    [
        ("Brand", 0x8f6dfb),
        ("Final", 0x4ade80),
        ("Review", 0xf59e0b),
    ]
    .into_iter()
    .map(|(name, rgb)| TagColor {
        name: name.to_owned(),
        rgb,
    })
    .collect()
}

fn stamp() -> DirStamp {
    DirStamp {
        modified: Timestamp::UNKNOWN,
        changed: Timestamp::UNKNOWN,
        len: 0,
        inode: 1,
        device: 1,
    }
}

pub fn drain<T>(receiver: &mut UnboundedReceiver<T>) -> Vec<T> {
    std::iter::from_fn(|| receiver.try_recv().ok()).collect()
}

/// An unmounted volume `label` on `/dev/<device>`.
pub fn volume(device: &str, label: &str, removable: bool) -> Volume {
    let path = |kind: &str| {
        OwnedObjectPath::try_from(format!("/org/freedesktop/UDisks2/{kind}/{device}"))
            .unwrap_or_default()
    };
    Volume {
        object_path: path("block_devices"),
        device: format!("/dev/{device}").into(),
        label: label.into(),
        size: 64 * GB,
        fs_type: "ext4".into(),
        mount_points: Vec::new(),
        drive: Some(Drive {
            object_path: path("drives"),
            removable,
            ejectable: removable,
            ..Drive::default()
        }),
        is_encrypted: false,
        cleartext_device: None,
        hints: Hints {
            auto: removable,
            ..Hints::default()
        },
    }
}

pub fn drive_row(volume: Volume) -> DriveRow {
    DriveRow {
        volume,
        activity: None,
    }
}

fn internal_drive(home: &Path) -> DriveRow {
    let mut crownbook = volume("nvme0n1p2", "CrownBook", false);
    crownbook.mount_points.push(home.to_path_buf());
    drive_row(crownbook)
}

pub fn phone() -> Peer {
    Peer {
        id: PeerId([9; 32]),
        name: "Pixel 9 Pro".to_owned(),
        kind: PeerKind::Phone,
        connected: true,
    }
}

pub fn tablet(connected: bool) -> Peer {
    Peer {
        id: PeerId([4; 32]),
        name: "Galaxy Tab".to_owned(),
        kind: PeerKind::Tablet,
        connected,
    }
}
