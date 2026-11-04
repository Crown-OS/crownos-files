//! The whole window driven headlessly over the mockup's folder: no engine,
//! no workers, no config files; what the window asks of each worker is
//! caught on a channel, and the tests answer for the workers.

use std::{cell::Cell, path::Path, rc::Rc, sync::Arc};

use crowntest::{Query, Role, TestApp, role};
use crownui::{
    ext::Modifiers,
    kit::{Theme, provide_theme},
    prelude::{Cx, component},
};
use tempfile::TempDir;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

use crate::{
    app::{
        drive_worker::DriveCommand,
        file_jobs::JobCommand,
        fixtures::{mockup_home, mockup_store},
        opener::OpenCommand,
        peers::PeerCommand,
        quick_look::QuickLookCommand,
        search::{Found, SearchRequest, finish_search},
        shell::shell,
        storage::MeasureBreakdown,
        tasks::Task,
        thumbnails::ThumbCommand,
        virtual_folder::entry_at,
    },
    fs::engine::EngineCmd,
    state::Store,
};

pub const WIDTH: f32 = 1100.0;
pub const HEIGHT: f32 = 700.0;
const FRAME_MILLIS: u64 = 16;
const DIALOG_KIND: &str = "kit_dialog";

/// The 16 visible entries in name order, folders first.
pub const LISTED: [&str; 16] = [
    "Brand",
    "Press Kit",
    "Renders",
    "Soundtrack",
    "aurora-wallpaper.avif",
    "benchmarks.csv",
    "compositor.rs",
    "crownbook-hero.png",
    "crownos-2026.11-x86_64.iso",
    "interview.flac",
    "keynote.pdf",
    "launch-assets.tar.zst",
    "launch-film.webm",
    "launch-plan.odt",
    "palette.svg",
    "release-notes.md",
];

pub const CTRL: Modifiers = Modifiers {
    ctrl_key: true,
    ..Modifiers::NONE
};
pub const SHIFT: Modifiers = Modifiers {
    shift_key: true,
    ..Modifiers::NONE
};

/// What the window sent each worker.
pub struct Workers {
    pub engine: UnboundedReceiver<EngineCmd>,
    pub tasks: UnboundedReceiver<Task>,
    pub thumbs: UnboundedReceiver<ThumbCommand>,
    pub opener: UnboundedReceiver<OpenCommand>,
    pub viewer: UnboundedReceiver<QuickLookCommand>,
    pub jobs: UnboundedReceiver<JobCommand>,
    pub drives: UnboundedReceiver<DriveCommand>,
    pub peers: UnboundedReceiver<PeerCommand>,
    pub breakdown: UnboundedReceiver<MeasureBreakdown>,
}

pub struct Window {
    pub app: TestApp,
    pub workers: Workers,
    pub store: Store,
    pub home: TempDir,
}

pub fn window_with(theme: Theme) -> Window {
    let home = mockup_home();
    let path = home.path().to_path_buf();
    let (engine, engine_commands) = unbounded_channel();
    let (tasks, task_commands) = unbounded_channel();
    let (thumbs, thumb_commands) = unbounded_channel();
    let (opener, open_commands) = unbounded_channel();
    let (viewer, viewer_commands) = unbounded_channel();
    let (jobs, job_commands) = unbounded_channel();
    let (drives, drive_commands) = unbounded_channel();
    let (peers, peer_commands) = unbounded_channel();
    let (breakdown, breakdown_requests) = unbounded_channel();
    let built = Rc::new(Cell::new(None));
    let slot = built.clone();
    let mut app = TestApp::with_size(
        component(move |cx| {
            provide_theme(cx, theme);
            let store = mockup_store(cx, &path);
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
            slot.set(Some(store));
            shell(cx, store)
        }),
        WIDTH,
        HEIGHT,
    );
    app.settle();
    let store = built.take().expect("the root component built the store");
    Window {
        app,
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
        store,
        home,
    }
}

pub fn window() -> Window {
    window_with(Theme::dark())
}

/// Everything waiting on `receiver`.
pub fn drain<T>(receiver: &mut UnboundedReceiver<T>) -> Vec<T> {
    std::iter::from_fn(|| receiver.try_recv().ok()).collect()
}

impl Window {
    pub fn entry(&self, entry: &str) -> Query {
        role(Role::ListItem).name(entry)
    }

    pub fn click(&mut self, entry: &str) -> &mut Self {
        let query = self.entry(entry);
        self.app.click(query).settle();
        self.focus_files();
        self
    }

    pub fn focus_files(&mut self) {
        self.app.focus(role(Role::List).name("Files")).settle();
    }

    pub fn click_with(&mut self, entry: &str, modifiers: Modifiers) -> &mut Self {
        self.app.hold_modifiers(modifiers);
        self.click(entry);
        self.app.hold_modifiers(Modifiers::NONE);
        self
    }

    pub fn key(&mut self, shortcut: &str) -> &mut Self {
        if self.app.ui().focused_semantic_id().is_none() {
            self.focus_files();
        }
        self.app.press_key(shortcut).settle();
        self
    }

    /// Runs `act` as a worker answering would, then draws one frame, so
    /// toasts are still up.
    pub fn answer<R>(&mut self, act: impl FnOnce(&mut Cx, Store) -> R) -> R {
        let store = self.store;
        let result = act(self.app.ui().cx(), store);
        self.app.advance_ms(FRAME_MILLIS);
        result
    }

    pub fn selected(&mut self) -> Vec<&'static str> {
        LISTED
            .into_iter()
            .filter(|entry| {
                let query = self.entry(entry);
                self.app.exists(query.clone())
                    && self.app.find(query).value.as_deref() == Some("selected")
            })
            .collect()
    }

    /// Clicks the dialog's own `label` button, which the window controls may share a name with.
    pub fn click_dialog_button(&mut self, label: &str) -> &mut Self {
        let center = self
            .app
            .ui()
            .semantic_tree()
            .iter()
            .find(|node| {
                node.role == Role::Button
                    && node.name.as_deref() == Some(label)
                    && node.path.contains(DIALOG_KIND)
            })
            .map(|node| node.bounds.center());
        let Some(center) = center else {
            panic!("no {label:?} button in a dialog");
        };
        self.app
            .pointer_down(center.x, center.y)
            .pointer_up(center.x, center.y)
            .settle();
        self
    }

    pub fn shows(&mut self, query: Query) -> bool {
        self.app.count(query) > 0
    }

    pub fn status(&mut self) -> String {
        let is_status = |name: &str| {
            let counted = name.split(',').next().unwrap_or_default();
            counted
                .strip_suffix(" items")
                .or_else(|| counted.strip_suffix(" item"))
                .is_some_and(|count| count.parse::<usize>().is_ok())
        };
        self.app
            .ui()
            .semantic_tree()
            .iter()
            .filter(|node| node.role == Role::Text)
            .filter(|node| node.name.as_deref().is_some_and(is_status))
            .max_by(|one, other| one.bounds.min_y().total_cmp(&other.bounds.min_y()))
            .and_then(|node| node.name.clone())
            .unwrap_or_default()
    }

    /// The latest walk the search box asked the task worker for.
    pub fn last_search(&mut self) -> Option<SearchRequest> {
        drain(&mut self.workers.tasks)
            .into_iter()
            .filter_map(|task| match task {
                Task::Search(request) => Some(request),
                _ => None,
            })
            .next_back()
    }

    /// Answers `request` as the worker would: `found` (paths under the
    /// mockup home), then the end of the walk.
    pub fn finish_search(&mut self, request: &SearchRequest, found: &[&str]) {
        let entries = found
            .iter()
            .filter_map(|relative| entry_at(&self.home.path().join(relative), None))
            .collect();
        let ticket = request.ticket;
        self.answer(|cx, store| {
            finish_search(cx, store, ticket, Found::Some(entries));
            finish_search(cx, store, ticket, Found::End);
        });
        self.app.settle();
    }

    pub fn dir(&self, relative: &str) -> Arc<Path> {
        self.home.path().join(relative).into()
    }

    pub fn engine_commands(&mut self) -> Vec<EngineCmd> {
        drain(&mut self.workers.engine)
    }

    pub fn job_commands(&mut self) -> Vec<JobCommand> {
        drain(&mut self.workers.jobs)
    }
}
