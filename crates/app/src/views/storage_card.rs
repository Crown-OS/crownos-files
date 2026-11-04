use crownui::{
    kit::{Segment, SegmentedBarOptions, card, icon_muted, icons, segmented_bar, use_theme},
    prelude::{Cx, Runtime, Styled, View, bind, hstack, show, spacer, text, vstack},
};

use crate::{
    app::storage::{DiskSpace, Share, shares},
    state::Store,
    util::{human_size::HumanSize, theme::tint},
};

const BAR_HEIGHT: f32 = 6.0;

/// The sidebar's card of how full the current folder's disk is.
pub fn storage_card(cx: &mut Cx, store: Store) -> impl View + use<> {
    let theme = use_theme(cx);
    let space = move |runtime: &mut Runtime| store.storage.space.get(runtime);
    let body = move || {
        let percent = move |runtime: &mut Runtime| {
            space(runtime).map_or_else(String::new, |space| {
                format!("{:.0}%", (space.usage.used_fraction() * 100.0).floor())
            })
        };
        let caption = move |runtime: &mut Runtime| {
            space(runtime).map_or_else(String::new, |space: DiskSpace| {
                format!(
                    "{} of {} used",
                    HumanSize(space.usage.used()),
                    HumanSize(space.usage.total)
                )
            })
        };
        card(
            vstack((
                hstack((
                    icon_muted(icons::HARD_DRIVE).size(15.0),
                    text("Storage")
                        .text_size(13.0)
                        .font_medium()
                        .text_color(tint(theme, |theme| theme.text.primary)),
                    spacer(),
                    text(bind(percent))
                        .text_size(12.0)
                        .text_color(tint(theme, |theme| theme.text.muted)),
                ))
                .items_center()
                .gap(8.0),
                segmented_bar([])
                    .segments_from(move |runtime| {
                        let Some(space) = space(runtime) else {
                            return Vec::new();
                        };
                        let (accent, light, dim, track) = theme.with(runtime, |theme| {
                            (
                                theme.accent.start,
                                theme.text.muted,
                                theme.text.disabled,
                                theme.control.track,
                            )
                        });
                        store.storage.breakdown.with(runtime, |breakdown| {
                            shares(space, breakdown.as_ref())
                                .into_iter()
                                .map(|(share, bytes)| {
                                    let color = match share {
                                        Share::Used | Share::Media => accent,
                                        Share::Documents => light,
                                        Share::Other => dim,
                                        Share::Free => track,
                                    };
                                    Segment::new(bytes as f32, color)
                                })
                                .collect()
                        })
                    })
                    .h(BAR_HEIGHT),
                text(bind(caption))
                    .text_size(11.0)
                    .text_color(tint(theme, |theme| theme.text.muted)),
            ))
            .gap(8.0)
            .p(12.0)
            .w_full(),
        )
        .w_full()
    };
    show(
        bind(move |runtime: &mut Runtime| space(runtime).is_some()),
        body,
        || vstack(()),
    )
    .flex_col()
}
