//! The worker that opens files in applications: it owns the MIME
//! associations, the icon theme and the recently-used list, and does all its
//! reading and launching on blocking threads.

use std::path::{Path, PathBuf};

use tokio::{
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    task::spawn_blocking,
};

use crate::{
    app::open_plan::OpenTarget,
    mime::MimeDatabase,
    model::mime_id::MimeId,
    open::{DesktopEntry, IconThemes, MimeApps, OpenRoute, RecentFiles, launch, route},
    thumbs::{Frame, decode},
    util::xdg_dirs::XdgDirs,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppsPurpose {
    /// The context menu's "Open With" list: names only.
    Menu,
    /// The "Open With" dialog: names and icons.
    Dialog,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenCommand {
    /// Opens each file in its default application, asking when there is none.
    Launch {
        targets: Vec<OpenTarget>,
        icon_edge: u32,
    },
    /// Lists the applications for the first target's type.
    Apps {
        targets: Vec<OpenTarget>,
        purpose: AppsPurpose,
        icon_edge: u32,
    },
    /// Opens `files` in `app`, making it the default for `default_for`.
    OpenWith {
        app: Box<str>,
        files: Vec<PathBuf>,
        default_for: Option<MimeId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppChoice {
    pub id: Box<str>,
    pub name: String,
    pub icon: Option<Frame>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppChoices {
    pub mime: MimeId,
    pub files: Vec<PathBuf>,
    pub apps: Vec<AppChoice>,
    pub purpose: AppsPurpose,
}

#[derive(Debug)]
pub enum OpenEvent {
    Failed(String),
    Apps(AppChoices),
}

struct Opener {
    apps: MimeApps,
    themes: IconThemes,
    recent: RecentFiles,
}

/// Serves `commands` one at a time until the window closes.
pub async fn run_opener(
    events: UnboundedSender<OpenEvent>,
    mut commands: UnboundedReceiver<OpenCommand>,
) {
    let Ok(Some(mut opener)) = spawn_blocking(Opener::load).await else {
        return;
    };
    while let Some(command) = commands.recv().await {
        let Ok((back, outcome)) = spawn_blocking(move || {
            let outcome = opener.handle(command);
            (opener, outcome)
        })
        .await
        else {
            return;
        };
        opener = back;
        for event in outcome {
            if events.send(event).is_err() {
                return;
            }
        }
    }
}

impl Opener {
    fn load() -> Option<Self> {
        let xdg = XdgDirs::from_env()?;
        Some(Self {
            apps: MimeApps::load(&xdg, MimeDatabase::system()),
            themes: IconThemes::load(&xdg),
            recent: RecentFiles::from_xdg(&xdg),
        })
    }

    fn handle(&mut self, command: OpenCommand) -> Vec<OpenEvent> {
        match command {
            OpenCommand::Launch { targets, icon_edge } => targets
                .into_iter()
                .filter_map(|target| self.open(target, icon_edge))
                .collect(),
            OpenCommand::Apps {
                targets,
                purpose,
                icon_edge,
            } => targets
                .first()
                .map(|first| {
                    let mime = self.mime_of(first);
                    let files = targets.iter().map(|target| target.path.clone()).collect();
                    OpenEvent::Apps(self.choices(mime, files, purpose, icon_edge))
                })
                .into_iter()
                .collect(),
            OpenCommand::OpenWith {
                app,
                files,
                default_for,
            } => self
                .open_with(&app, &files, default_for)
                .err()
                .into_iter()
                .collect(),
        }
    }

    fn open(&self, target: OpenTarget, icon_edge: u32) -> Option<OpenEvent> {
        let mime = self.mime_of(&target);
        match route(&target.path, target.kind, mime, &self.apps) {
            OpenRoute::App(entry) => self.start(&entry, &[target.path]).err(),
            OpenRoute::ChooseApp(mime) => Some(OpenEvent::Apps(self.choices(
                mime,
                vec![target.path],
                AppsPurpose::Dialog,
                icon_edge,
            ))),
            OpenRoute::Navigate | OpenRoute::Preview => None,
        }
    }

    fn open_with(
        &mut self,
        app: &str,
        files: &[PathBuf],
        default_for: Option<MimeId>,
    ) -> Result<(), OpenEvent> {
        let entry = self
            .apps
            .find(app)
            .ok_or_else(|| OpenEvent::Failed(format!("{app} is not installed")))?;
        if let Some(mime) = default_for {
            self.apps
                .set_default(mime, app)
                .map_err(|error| OpenEvent::Failed(error.to_string()))?;
        }
        self.start(&entry, files)
    }

    fn start(&self, entry: &DesktopEntry, files: &[PathBuf]) -> Result<(), OpenEvent> {
        let files: Vec<&Path> = files.iter().map(PathBuf::as_path).collect();
        launch(entry, &files, Some(&self.recent))
            .map_err(|error| OpenEvent::Failed(format!("{} could not start: {error}", entry.name)))
    }

    fn mime_of(&self, target: &OpenTarget) -> MimeId {
        if target.mime != MimeId::UNKNOWN {
            return target.mime;
        }
        let name = target
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        self.apps.db().mime_of_name(name)
    }

    fn choices(
        &self,
        mime: MimeId,
        files: Vec<PathBuf>,
        purpose: AppsPurpose,
        icon_edge: u32,
    ) -> AppChoices {
        let apps = self
            .apps
            .apps_for(mime)
            .into_iter()
            .map(|entry| AppChoice {
                icon: (purpose == AppsPurpose::Dialog)
                    .then(|| self.icon(entry.icon.as_deref()?, icon_edge))
                    .flatten(),
                id: entry.id,
                name: entry.name,
            })
            .collect();
        AppChoices {
            mime,
            files,
            apps,
            purpose,
        }
    }

    fn icon(&self, name: &str, edge: u32) -> Option<Frame> {
        let path = self.themes.find(name)?;
        decode(&path, edge).ok().map(|decoded| decoded.frame)
    }
}
