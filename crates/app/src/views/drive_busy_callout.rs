//! The callout over the file area when a drive could not be ejected because
//! something still has files open on it.

use crownui::{
    kit::{ButtonOptions, ButtonSize, ButtonVariant, CalloutOptions, Tone, button, callout, icons},
    prelude::{Listeners, Runtime, Styled, View, bind, hstack, show, vstack},
};

use crate::{
    app::drives::{dismiss_busy, retry_release},
    state::Store,
};

const MAX_WIDTH: f32 = 520.0;

pub fn drive_busy_callout(store: Store) -> impl View {
    let title = move |runtime: &mut Runtime| {
        store.drives.busy.with(runtime, |busy| {
            busy.as_ref()
                .map(|busy| format!("{} is in use", busy.name))
                .unwrap_or_default()
        })
    };
    let banner = move || {
        hstack((
            callout(Tone::Warning, bind(title))
                .body("Close the files and apps using it, then eject it again.")
                .icon(icons::CIRCLE_ALERT)
                .flex_1()
                .min_w(0.0),
            vstack((
                button("Try Again")
                    .variant(ButtonVariant::Primary)
                    .control_size(ButtonSize::Small)
                    .leading_icon(icons::ARROW_UP_FROM_LINE)
                    .on_click(move |cx, _| retry_release(cx, store)),
                button("Dismiss")
                    .variant(ButtonVariant::Ghost)
                    .control_size(ButtonSize::Small)
                    .on_click(move |cx, _| dismiss_busy(cx, store)),
            ))
            .gap(6.0),
        ))
        .items_center()
        .gap(10.0)
        .max_w(MAX_WIDTH)
        .pointer_events_auto()
    };
    vstack(show(
        bind(move |runtime: &mut Runtime| store.drives.busy.with(runtime, Option::is_some)),
        banner,
        || hstack(()),
    ))
    .w_full()
    .h_full()
    .items_center()
    .justify_start()
    .p(12.0)
    .pointer_events_none()
}
