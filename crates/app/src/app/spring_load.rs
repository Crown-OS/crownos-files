//! Spring-loaded folders: holding a drag over a folder for a moment opens
//! it, so files can be dropped deeper without letting go.

use std::{path::Path, sync::Arc, time::Duration};

use crownui::prelude::{Cx, Runtime};

use crate::{app::navigation::open_dir, state::Store};

pub const SPRING_DELAY: Duration = Duration::from_millis(700);

/// The folder a drag rests on, since which frame, and a generation that
/// retires the frame callbacks of earlier folders.
#[derive(Debug, Default)]
pub struct SpringLoad {
    target: Option<Arc<Path>>,
    since: Option<Duration>,
    generation: u64,
}

enum Tick {
    Stop,
    Wait,
    Open(Arc<Path>),
}

/// Starts the countdown for `dir` unless it is already running for it.
pub fn arm(cx: &mut Cx, store: Store, dir: Arc<Path>) {
    let generation = store.drag.spring.update(cx, |spring| {
        if spring.target.as_ref() == Some(&dir) {
            return None;
        }
        spring.generation += 1;
        spring.target = Some(dir);
        spring.since = None;
        Some(spring.generation)
    });
    if let Some(generation) = generation {
        wait(cx, store, generation);
    }
}

pub fn disarm(runtime: &mut Runtime, store: Store) {
    store.drag.spring.update(runtime, |spring| {
        spring.generation += 1;
        spring.target = None;
    });
}

/// The drag left `dir`; the countdown stops if it was for it.
pub fn disarm_from(runtime: &mut Runtime, store: Store, dir: &Path) {
    let armed = store
        .drag
        .spring
        .with(runtime, |spring| spring.target.as_deref() == Some(dir));
    if armed {
        disarm(runtime, store);
    }
}

fn wait(cx: &mut Cx, store: Store, generation: u64) {
    cx.request_animation_frame(move |cx, now| tick(cx, store, generation, now));
}

fn tick(cx: &mut Cx, store: Store, generation: u64, now: Duration) {
    let step = store.drag.spring.update(cx, |spring| {
        if spring.generation != generation {
            return Tick::Stop;
        }
        let since = *spring.since.get_or_insert(now);
        if now.saturating_sub(since) < SPRING_DELAY {
            return Tick::Wait;
        }
        spring.generation += 1;
        spring.target.take().map_or(Tick::Stop, Tick::Open)
    });
    match step {
        Tick::Stop => {}
        Tick::Wait => wait(cx, store, generation),
        Tick::Open(dir) => open_dir(cx, store, dir),
    }
}
