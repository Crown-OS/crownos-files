//! Long-lived backend futures on CrownUI's task runtime, talking to the UI
//! thread through two channels: commands down, events up in bursts.

use std::future::Future;

use crownui::prelude::Cx;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

/// Events handed to the UI thread per wake-up, at most: a burst of listing
/// chunks or watcher events folds into one reactive flush.
const BURST: usize = 256;

/// Starts `run` for as long as the calling component lives and hands every
/// burst of events it sends to `on_burst` on the UI thread. The returned
/// sender is the command channel `run` receives from.
pub fn spawn_worker<C, E, F>(
    cx: &mut Cx,
    run: impl FnOnce(UnboundedSender<E>, UnboundedReceiver<C>) -> F,
    on_burst: impl FnMut(&mut Cx, Vec<E>) + 'static,
) -> UnboundedSender<C>
where
    C: Send + 'static,
    E: Send + 'static,
    F: Future<Output = ()> + Send + 'static,
{
    let (commands, command_receiver) = unbounded_channel();
    let (events, event_receiver) = unbounded_channel();
    cx.spawn(run(events, command_receiver), |_, ()| {});
    pump(cx, event_receiver, on_burst);
    commands
}

fn pump<E: Send + 'static>(
    cx: &mut Cx,
    mut events: UnboundedReceiver<E>,
    mut on_burst: impl FnMut(&mut Cx, Vec<E>) + 'static,
) {
    let next_burst = async move {
        let mut burst = Vec::new();
        let open = events.recv_many(&mut burst, BURST).await > 0;
        (events, burst, open)
    };
    cx.spawn(next_burst, move |cx, (events, burst, open)| {
        if !burst.is_empty() {
            on_burst(cx, burst);
        }
        if open {
            pump(cx, events, on_burst);
        }
    });
}
