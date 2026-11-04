//! The Properties dialog: name, type, size, place, times, owner and
//! permissions of one entry, and the capacity of a volume mounted there.

use crownui::{
    kit::{ButtonOptions, ButtonVariant, Theme, button, dialog, use_theme},
    prelude::{
        AnyView, Listeners, Runtime, Signal, Styled, View, hstack, keyed, spacer, text, vstack,
    },
};

use crate::{
    app::properties::{FileFacts, PropertySheet},
    state::Store,
    util::{
        friendly_date::Today,
        human_size::HumanSize,
        permission_label::{Audience, rwx},
        theme::tint,
    },
};

const WIDTH: f32 = 360.0;
const LABEL_WIDTH: f32 = 96.0;
const PENDING: &str = "…";

pub fn properties_dialog(store: Store) -> impl View {
    let properties = store.properties;
    dialog(properties.open, move || {
        keyed(
            move |runtime: &mut Runtime| properties.sheet.get(runtime),
            move |cx, sheet: Option<PropertySheet>| {
                let theme = use_theme(cx);
                let today = store.today.with(cx, |today| *today);
                let Some(sheet) = sheet else {
                    return vstack(()).boxed();
                };
                let rows: Vec<AnyView> = sheet_rows(&sheet, today)
                    .into_iter()
                    .map(|(label, value)| row(theme, label, value))
                    .collect();
                vstack((
                    vstack((
                        text(sheet.name).font_semibold().truncate(),
                        text(sheet.kind)
                            .text_size(12.0)
                            .text_color(tint(theme, |theme| theme.text.muted)),
                    ))
                    .gap(2.0),
                    vstack(rows).gap(6.0),
                    hstack((
                        spacer(),
                        button("Close")
                            .variant(ButtonVariant::Secondary)
                            .on_click(move |cx, _| properties.open.set(cx, false)),
                    )),
                ))
                .gap(14.0)
                .w(WIDTH)
                .boxed()
            },
        )
        .flex_col()
    })
}

/// The dialog's rows, in order; times and owners show "…" until read.
pub fn sheet_rows(sheet: &PropertySheet, today: Today) -> Vec<(&'static str, String)> {
    let size = match sheet.size {
        Some(bytes) => HumanSize(bytes).to_string(),
        None if sheet.is_dir => "Calculating…".to_owned(),
        None => PENDING.to_owned(),
    };
    let mut rows = vec![
        ("Type", sheet.mime.to_owned()),
        ("Size", size),
        ("Location", sheet.location.clone()),
    ];
    let Some(facts) = &sheet.facts else {
        rows.extend(
            [
                "Created", "Modified", "Accessed", "Owner", "Group", "Others",
            ]
            .map(|label| (label, PENDING.to_owned())),
        );
        return rows;
    };
    let when = |stamp| {
        today
            .describe(stamp)
            .unwrap_or_else(|| "Unknown".to_owned())
    };
    rows.extend([
        ("Created", when(facts.created)),
        ("Modified", when(facts.modified)),
        ("Accessed", when(facts.accessed)),
    ]);
    rows.extend(permission_rows(facts));
    if let Some(volume) = facts.volume {
        rows.extend([
            ("Capacity", HumanSize(volume.total).to_string()),
            ("Free", HumanSize(volume.available).to_string()),
        ]);
    }
    rows
}

fn permission_rows(facts: &FileFacts) -> [(&'static str, String); 3] {
    [
        (
            "Owner",
            format!("{} · {}", facts.owner, rwx(facts.mode, Audience::Owner)),
        ),
        (
            "Group",
            format!("{} · {}", facts.group, rwx(facts.mode, Audience::Group)),
        ),
        ("Others", rwx(facts.mode, Audience::Others)),
    ]
}

fn row(theme: Signal<Theme>, label: &'static str, value: String) -> AnyView {
    hstack((
        text(label)
            .text_size(12.0)
            .w(LABEL_WIDTH)
            .shrink(0.0)
            .text_color(tint(theme, |theme| theme.text.muted)),
        text(value)
            .text_size(12.0)
            .truncate()
            .min_w(0.0)
            .text_color(tint(theme, |theme| theme.text.body)),
    ))
    .gap(8.0)
    .boxed()
}
