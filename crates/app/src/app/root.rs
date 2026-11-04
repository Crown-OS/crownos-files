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
        drive_worker::run_drives,
        drives::finish_drives,
        file_jobs::{finish_jobs, run_jobs},
        listing::apply_ops,
        navigation::load_current,
        opener::run_opener,
        opening::finish_opening,
        quick_look::{finish_quick_look, run_quick_look},
        shell::shell,
        storage::{finish_breakdown, run_breakdown},
        tasks::{finish_tasks, run_tasks},
        thumbnails::{finish_thumbnails, run_thumbnailer},
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
    let thumbnailer = spawn_worker(cx, run_thumbnailer, move |cx, events| {
        finish_thumbnails(cx, store, events);
    });
    store
        .thumbs
        .service
        .update(cx, |slot| *slot = Some(thumbnailer));
    let opener = spawn_worker(cx, run_opener, move |cx, events| {
        finish_opening(cx, store, events);
    });
    store.opening.sender.update(cx, |slot| *slot = Some(opener));
    let viewer = spawn_worker(cx, run_quick_look, move |cx, events| {
        finish_quick_look(cx, store, events);
    });
    store
        .quick_look
        .sender
        .update(cx, |slot| *slot = Some(viewer));
    let jobs = spawn_worker(cx, run_jobs, move |cx, updates| {
        finish_jobs(cx, store, updates)
    });
    store.jobs.sender.update(cx, |slot| *slot = Some(jobs));
    let drives = spawn_worker(cx, run_drives, move |cx, events| {
        finish_drives(cx, store, events);
    });
    store.drives.sender.update(cx, |slot| *slot = Some(drives));
    #[cfg(feature = "connect")]
    {
        let peers = spawn_worker(cx, crate::app::peer_worker::run_peers, move |cx, events| {
            crate::app::peers::finish_peers(cx, store, events);
        });
        store.peers.sender.update(cx, |slot| *slot = Some(peers));
    }
    let dirs = store.places.with(cx, |places| places.special.clone());
    let storage = store.storage;
    let breakdown = spawn_worker(
        cx,
        move |breakdowns, requests| run_breakdown(dirs, breakdowns, requests),
        move |cx, burst| finish_breakdown(cx, storage, burst),
    );
    storage.sender.update(cx, |slot| *slot = Some(breakdown));
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
