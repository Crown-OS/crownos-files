//! The right column of the content card: what is selected, previewed and described.

use crownconfig::TagColor;
use crownui::{
    kit::{ButtonOptions, ButtonSize, ButtonVariant, Theme, button, icons, tag_chip, use_theme},
    prelude::{
        AnyView, Curve, Cx, Listeners, Runtime, Semantic, Signal, Styled, View, bind, hstack,
        keyed, rgb, show, spacer, text, vstack,
    },
};

use crate::{
    app::actions::{Deferred, deferred, open_selection},
    model::{entry::Entry, places::Places},
    state::Store,
    util::{
        breadcrumbs::describe_location, friendly_date::Today, human_size::HumanSize,
        kind_label::kind_label, size_label::size_label, theme::tint,
    },
    views::glyph::{Glyph, glyph_view},
};

const CROSS_FADE_MILLIS: u64 = 240;
const PREVIEW_HEIGHT: f32 = 112.0;
const PREVIEW_SCALE: f32 = 1.45;

/// What the panel describes: one entry, several, or the folder itself.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Description {
    glyph: Glyph,
    title: String,
    subtitle: String,
    rows: Vec<(&'static str, String)>,
    is_entry: bool,
}

pub fn details(store: Store) -> impl View {
    keyed(
        move |runtime: &mut Runtime| {
            (
                store.revision.get(runtime),
                store.selection_rev.get(runtime),
                store.tags.get(runtime),
            )
        },
        move |cx, (_, _, tags)| panel(cx, store, tags),
    )
    .flex_col()
    .w_full()
    .h_full()
    .min_h(0.0)
    .overflow_hidden()
    .transition(Curve::ease_in_out(CROSS_FADE_MILLIS))
    .label("Details")
}

fn panel(cx: &mut Cx, store: Store, tags: Vec<TagColor>) -> impl View + use<> {
    let theme = use_theme(cx);
    let description = describe(cx, store);
    let preview = vstack(glyph_view(description.glyph, PREVIEW_SCALE))
        .h(PREVIEW_HEIGHT)
        .shrink(0.0)
        .items_center()
        .justify_center()
        .rounded(10.0)
        .bg(tint(theme, |theme| theme.surface.sunken))
        .border(1.0)
        .border_color(tint(theme, |theme| theme.surface.border))
        .overflow_hidden();
    let title = vstack((
        text(description.title)
            .text_size(14.0)
            .font_semibold()
            .truncate()
            .text_color(tint(theme, |theme| theme.text.primary)),
        text(description.subtitle)
            .text_size(12.0)
            .truncate()
            .text_color(tint(theme, |theme| theme.text.muted)),
    ))
    .gap(3.0);
    let rows: Vec<AnyView> = description
        .rows
        .into_iter()
        .map(|(label, value)| info_row(theme, label, value).boxed())
        .collect();
    let info = vstack((hairline(theme), vstack(rows).gap(8.0))).gap(12.0);
    let chips: Vec<AnyView> = tags
        .into_iter()
        .map(|tag| tag_chip(tag.name, rgb(tag.rgb)).boxed())
        .collect();
    let actions = show(description.is_entry, move || actions(store), || hstack(()));
    vstack((
        preview,
        title,
        info,
        hstack(chips).flex_wrap().gap(6.0),
        spacer(),
        actions,
    ))
    .gap(12.0)
    .p(16.0)
    .w_full()
    .h_full()
    .min_h(0.0)
}

fn hairline(theme: Signal<Theme>) -> impl View {
    vstack(())
        .h(1.0)
        .shrink(0.0)
        .bg(tint(theme, |theme| theme.surface.hairline))
}

fn info_row(theme: Signal<Theme>, label: &'static str, value: String) -> impl View {
    vstack((
        text(label)
            .text_size(11.0)
            .text_color(tint(theme, |theme| theme.text.muted)),
        text(value)
            .text_size(12.0)
            .truncate()
            .text_color(tint(theme, |theme| theme.text.body)),
    ))
    .gap(1.0)
}

fn actions(store: Store) -> impl View {
    let receiver = move |runtime: &mut Runtime| {
        store.devices.with(runtime, |devices| {
            devices
                .iter()
                .find(|device| device.can_receive_files())
                .map(|device| device.name.clone())
        })
    };
    hstack((
        button("Open")
            .variant(ButtonVariant::Primary)
            .control_size(ButtonSize::Small)
            .on_click(move |cx, _| open_selection(cx, store)),
        show(
            bind(move |runtime: &mut Runtime| receiver(runtime).is_some()),
            move || {
                button(bind(move |runtime: &mut Runtime| {
                    format!("Send to {}", receiver(runtime).unwrap_or_default())
                }))
                .variant(ButtonVariant::Secondary)
                .control_size(ButtonSize::Small)
                .leading_icon(icons::SMARTPHONE)
                .on_click(move |cx, _| deferred(cx, store, Deferred::SendToDevice))
            },
            || hstack(()),
        ),
    ))
    .flex_wrap()
    .gap(6.0)
}

fn describe(runtime: &Runtime, store: Store) -> Description {
    let today = store.today.with(runtime, |today| *today);
    store.places.with(runtime, |places| {
        store.folder.with(runtime, |folder| {
            let dir = folder.dir();
            let location = describe_location(dir, places.home.as_deref());
            let mut selected = folder.selected_entries();
            match (selected.next(), selected.next()) {
                (Some(entry), None) => describe_entry(entry, dir, places, today, location),
                (Some(_), Some(_)) => {
                    let total: u64 = folder.selected_entries().map(|entry| entry.stat.len).sum();
                    Description {
                        glyph: Glyph::Page(None),
                        title: format!("{} items", folder.selection().len()),
                        subtitle: HumanSize(total).to_string(),
                        rows: vec![("Where", location)],
                        is_entry: true,
                    }
                }
                (None, _) => Description {
                    glyph: Glyph::Folder(
                        places
                            .special(dir)
                            .and_then(|special| special.folder_glyph()),
                    ),
                    title: dir.file_name().map_or_else(
                        || "Computer".to_owned(),
                        |name| name.to_string_lossy().into_owned(),
                    ),
                    subtitle: format!("Folder · {} items", folder.len()),
                    rows: vec![("Where", location)],
                    is_entry: false,
                },
            }
        })
    })
}

fn describe_entry(
    entry: &Entry,
    dir: &std::path::Path,
    places: &Places,
    today: Today,
    location: String,
) -> Description {
    let special = entry
        .is_dir()
        .then(|| places.special(&entry.path_in(dir)))
        .flatten();
    let mut rows = Vec::with_capacity(4);
    if let Some(target) = entry.link_target() {
        rows.push(("Links to", target.display().to_string()));
    }
    for (label, stamp) in [
        ("Modified", entry.stat.modified),
        ("Created", entry.stat.created),
    ] {
        if let Some(when) = today.describe(stamp) {
            rows.push((label, when));
        }
    }
    rows.push(("Where", location));
    Description {
        glyph: Glyph::for_entry(entry, special),
        title: entry.name.to_string(),
        subtitle: format!("{} · {}", kind_label(entry), size_label(entry)),
        rows,
        is_entry: true,
    }
}
