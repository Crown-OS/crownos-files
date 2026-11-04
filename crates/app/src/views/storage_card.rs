use crownui::{
    kit::{Segment, SegmentedBarOptions, card, icon_muted, icons, segmented_bar, use_theme},
    prelude::{Cx, Runtime, Styled, View, bind, hstack, show, spacer, text, vstack},
};

use crate::{
    model::disk_usage::DiskUsage,
    state::Store,
    util::{human_size::HumanSize, theme::tint},
};

const BAR_HEIGHT: f32 = 6.0;

/// The sidebar's card of how full the current folder's disk is.
pub fn storage_card(cx: &mut Cx, store: Store) -> impl View + use<> {
    let theme = use_theme(cx);
    let usage = move |runtime: &mut Runtime| store.storage.get(runtime);
    let body = move || {
        let percent = move |runtime: &mut Runtime| {
            usage(runtime).map_or_else(String::new, |usage| {
                format!("{:.0}%", (usage.used_fraction() * 100.0).floor())
            })
        };
        let caption = move |runtime: &mut Runtime| {
            usage(runtime).map_or_else(String::new, |usage: DiskUsage| {
                format!(
                    "{} of {} used",
                    HumanSize(usage.used()),
                    HumanSize(usage.total)
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
                        let (used, track) =
                            theme.with(runtime, |theme| (theme.accent.start, theme.control.track));
                        usage(runtime).map_or_else(Vec::new, |usage| {
                            vec![
                                Segment::new(usage.used() as f32, used),
                                Segment::new(usage.available as f32, track),
                            ]
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
        bind(move |runtime: &mut Runtime| usage(runtime).is_some()),
        body,
        || vstack(()),
    )
    .flex_col()
}
