//! The right column of the content card: what is selected, previewed and described.

use std::path::Path;

use crownconfig::TagColor;
use crownos_files_devices::tags::Tags;
use crownui::{
    kit::{ButtonOptions, ButtonSize, ButtonVariant, Theme, button, icons, tag_chip, use_theme},
    prelude::{
        AnyView, Curve, Cx, Listeners, Runtime, Semantic, Signal, Styled, View, bind, hstack,
        keyed, rgb, show, spacer, text, vstack,
    },
};

use crate::{
    app::{
        opening::open_selection, peers::send_selection, thumbnails::thumb_source,
        virtual_folder::VirtualFolder,
    },
    model::{entry::Entry, places::Places, trash::TrashedItem},
    state::Store,
    util::{
        breadcrumbs::describe_location, friendly_date::Today, human_size::HumanSize,
        kind_label::kind_label, size_label::size_label, theme::tint, thumb_store::ThumbSource,
    },
    views::{entry_picture::entry_picture, glyph::Glyph},
};

const CROSS_FADE_MILLIS: u64 = 240;
const PREVIEW_HEIGHT: f32 = 112.0;
const PREVIEW_SCALE: f32 = 1.45;

/// What the panel describes: one entry, several, or the folder itself.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Description {
    glyph: Glyph,
    thumb: Option<ThumbSource>,
    title: String,
    subtitle: String,
    rows: Vec<(&'static str, String)>,
    tags: Tags,
    is_entry: bool,
}

pub fn details(store: Store) -> impl View {
    keyed(
        move |runtime: &mut Runtime| {
            (
                store.revision.get(runtime),
                store.selection_rev.get(runtime),
                store.tagging.revision.get(runtime),
                store.tags.get(runtime),
            )
        },
        move |cx, (_, _, _, palette)| panel(cx, store, &palette),
    )
    .flex_col()
    .w_full()
    .h_full()
    .min_h(0.0)
    .overflow_hidden()
    .transition(Curve::ease_in_out(CROSS_FADE_MILLIS))
    .label("Details")
}

fn panel(cx: &mut Cx, store: Store, palette: &[TagColor]) -> impl View + use<> {
    let theme = use_theme(cx);
    let description = describe(cx, store);
    let preview = vstack(entry_picture(
        store,
        description.glyph,
        description.thumb.clone(),
        PREVIEW_SCALE,
    ))
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
    let chips: Vec<AnyView> = description
        .tags
        .iter()
        .map(|tag| chip(theme, palette, tag))
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

/// A chip in the configured color of `tag`, or the neutral one.
fn chip(theme: Signal<Theme>, palette: &[TagColor], tag: &str) -> AnyView {
    match palette
        .iter()
        .find(|known| known.name.eq_ignore_ascii_case(tag))
    {
        Some(known) => tag_chip(tag, rgb(known.rgb)).boxed(),
        None => tag_chip(tag, tint(theme, |theme| theme.status.neutral)).boxed(),
    }
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
    let receiver =
        move |runtime: &mut Runtime| store.peers.first_phone(runtime).map(|phone| phone.name);
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
                .on_click(move |cx, _| {
                    if let Some(phone) = store.peers.first_phone(cx) {
                        send_selection(cx, store, &phone);
                    }
                })
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
            let virtual_folder = VirtualFolder::of(dir);
            let location = virtual_folder.map_or_else(
                || describe_location(dir, places.home.as_deref()),
                |folder| folder.label().to_owned(),
            );
            let mut selected = folder.selected_entries();
            match (selected.next(), selected.next()) {
                (Some(entry), None) => {
                    let location = match virtual_folder {
                        Some(_) => store.trash_items.with(runtime, |trashed| {
                            entry_location(entry, dir, trashed, places)
                        }),
                        None => location,
                    };
                    let mut description = describe_entry(entry, dir, places, today, location);
                    description.tags = store.tagging.of(runtime, &entry.name);
                    description
                }
                (Some(_), Some(_)) => {
                    let total: u64 = folder.selected_entries().map(|entry| entry.stat.len).sum();
                    Description {
                        glyph: Glyph::Page(None),
                        thumb: None,
                        title: format!("{} items", folder.selection().len()),
                        subtitle: HumanSize(total).to_string(),
                        rows: vec![("Where", location)],
                        tags: Tags::new(),
                        is_entry: true,
                    }
                }
                (None, _) => Description {
                    glyph: Glyph::Folder(
                        places
                            .special(dir)
                            .and_then(|special| special.folder_glyph()),
                    ),
                    thumb: None,
                    title: match virtual_folder {
                        Some(folder) => folder.label().to_owned(),
                        None => dir.file_name().map_or_else(
                            || "Computer".to_owned(),
                            |name| name.to_string_lossy().into_owned(),
                        ),
                    },
                    subtitle: format!("Folder · {} items", folder.len()),
                    rows: vec![("Where", location)],
                    tags: Tags::new(),
                    is_entry: false,
                },
            }
        })
    })
}

/// Where an entry of a virtual folder really is: the folder it was trashed
/// from, or the one holding it.
fn entry_location(entry: &Entry, dir: &Path, trashed: &[TrashedItem], places: &Places) -> String {
    let path = entry.path_in(dir);
    let original = trashed
        .iter()
        .find(|item| item.file_path() == path)
        .map(|item| item.info.original.as_path());
    original
        .unwrap_or(&path)
        .parent()
        .map(|parent| describe_location(parent, places.home.as_deref()))
        .unwrap_or_default()
}

fn describe_entry(
    entry: &Entry,
    dir: &Path,
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
    let glyph = Glyph::for_entry(entry, special);
    Description {
        glyph,
        thumb: thumb_source(entry, dir, glyph),
        title: entry.name.to_string(),
        subtitle: format!("{} · {}", kind_label(entry), size_label(entry)),
        rows,
        tags: Tags::new(),
        is_entry: true,
    }
}
