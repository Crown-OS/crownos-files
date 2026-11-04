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
        file_jobs::JobCommand,
        fixtures::{mockup_home, mockup_store},
        opener::OpenCommand,
        quick_look::QuickLookCommand,
        shell::shell,
        tasks::Task,
        thumbnails::ThumbCommand,
    },
    fs::engine::EngineCmd,
    state::Store,
};

pub const WIDTH: f32 = 1100.0;
pub const HEIGHT: f32 = 700.0;
const FRAME_MILLIS: u64 = 16;

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
            .filter_map(|node| node.name.clone())
            .filter(|name| is_status(name))
            .last()
            .unwrap_or_default()
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
