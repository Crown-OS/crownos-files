use std::{path::PathBuf, sync::Arc};

use crownconfig::{load_appearance, load_files};
use crownui::{
    kit::{WindowRadius, provide_configured_theme, provide_window_radius},
    prelude::{Cx, View, component},
};
use tokio::sync::{
    mpsc::{UnboundedReceiver, UnboundedSender},
    watch,
};

use crate::{
    app::{
        config_sync::sync_config,
        listing::apply_ops,
        navigation::load_current,
        shell::shell,
        tasks::{finish_tasks, run_tasks},
    },
    fs::{
        engine::{EngineCmd, run_engine},
        mounts::MountTable,
        mounts_watch::MountWatch,
    },
    model::{files_op::FilesOp, places::Places},
    state::Store,
    util::{friendly_date::Today, worker::spawn_worker},
};

/// The window on `start` (home when `None`). The store, the theme, the config
/// watch and every worker live in this component's scope, so they last
/// exactly as long as the window.
pub fn root(start: Option<PathBuf>) -> impl View {
    component(move |cx| {
        provide_configured_theme(cx);
        provide_window_radius(cx, WindowRadius(f32::from(load_appearance().border_radius)));
        let files = load_files();
        let places = Places::load();
        let dir = start
            .or_else(|| places.home.clone())
            .unwrap_or_else(|| PathBuf::from("/"));
        let dir = std::path::absolute(&dir).unwrap_or(dir);
        let store = Store::new(cx, dir.into(), &files, places, Today::now());
        start_workers(cx, store);
        sync_config(cx, store, files);
        load_current(cx, store);
        shell(cx, store)
    })
}

fn start_workers(cx: &mut Cx, store: Store) {
    let engine = spawn_worker(cx, serve_engine, move |cx, ops| {
        store.today.update(cx, |today| *today = Today::now());
        apply_ops(cx, store, ops);
    });
    store.engine.update(cx, |slot| *slot = Some(engine));
    let tasks = spawn_worker(cx, run_tasks, move |cx, done| finish_tasks(cx, store, done));
    store.tasks.update(cx, |slot| *slot = Some(tasks));
}

async fn serve_engine(ops: UnboundedSender<FilesOp>, commands: UnboundedReceiver<EngineCmd>) {
    let live = MountWatch::spawn();
    let fallback = watch::channel(Arc::new(MountTable::read().unwrap_or_default()));
    let mounts = live
        .as_ref()
        .map_or_else(|_| fallback.1.clone(), MountWatch::subscribe);
    if let Err(error) = run_engine(ops, commands, mounts).await {
        eprintln!("crownos-files: the filesystem engine stopped: {error}");
    }
}
