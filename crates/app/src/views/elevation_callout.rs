//! The callout over the file area after a job was refused for lack of
//! permission, offering to run it again as root.

use crownui::{
    kit::{ButtonOptions, ButtonSize, ButtonVariant, CalloutOptions, Tone, button, callout, icons},
    prelude::{Cx, Listeners, Runtime, Styled, View, bind, hstack, show, vstack},
};

use crate::{
    app::privilege::{authenticate, dismiss_privileges},
    state::Store,
};

const MAX_WIDTH: f32 = 520.0;

pub fn elevation_callout(_cx: &mut Cx, store: Store) -> impl View + use<> {
    let message = move |runtime: &mut Runtime| {
        store.elevation.with(runtime, |prompt| {
            prompt
                .as_ref()
                .map(|prompt| prompt.message.clone())
                .unwrap_or_default()
        })
    };
    let banner = move || {
        hstack((
            callout(Tone::Warning, "You don't have permission for this")
                .body(bind(message))
                .icon(icons::SHIELD_ALERT)
                .flex_1()
                .min_w(0.0),
            vstack((
                button("Authenticate")
                    .variant(ButtonVariant::Primary)
                    .control_size(ButtonSize::Small)
                    .leading_icon(icons::KEY_ROUND)
                    .on_click(move |cx, _| authenticate(cx, store)),
                button("Dismiss")
                    .variant(ButtonVariant::Ghost)
                    .control_size(ButtonSize::Small)
                    .on_click(move |cx, _| dismiss_privileges(cx, store)),
            ))
            .gap(6.0),
        ))
        .items_center()
        .gap(10.0)
        .max_w(MAX_WIDTH)
        .pointer_events_auto()
    };
    vstack(show(
        bind(move |runtime: &mut Runtime| store.elevation.with(runtime, Option::is_some)),
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
