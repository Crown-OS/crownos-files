//! The toolbar's jobs indicator: a progress ring while jobs run, opening a
//! popover with each job's bar and a Cancel button.

use std::f32::consts::{FRAC_PI_2, TAU};

use crownui::{
    ext::{DrawCommand, Paint, Path, Stroke},
    kit::{ButtonOptions, ButtonVariant, Theme, icon_button, icons, progress, use_theme},
    prelude::{
        CornerRadii, Cx, Listeners, Point, Rect, Role, Runtime, Semantic, Shadow, Signal, Size,
        Styled, View, bind, canvas, for_each, hstack, popover, show, text, vstack,
    },
};

use crate::{
    app::file_jobs::{JobRow, cancel_job},
    state::Store,
    util::theme::tint,
};

const RING: f32 = 18.0;
const RING_WIDTH: f32 = 2.5;
const ANCHOR: f32 = 32.0;
const ARC_SEGMENTS: usize = 48;
const PANEL_WIDTH: f32 = 280.0;
const PANEL_RADIUS: f32 = 12.0;
/// The toolbar's gap, kept only while the indicator shows.
const GAP: f32 = 8.0;

pub fn jobs_indicator(cx: &mut Cx, store: Store) -> impl View + use<> {
    let theme = use_theme(cx);
    let open = cx.signal(false);
    let running = store.jobs.running;
    cx.effect(move |runtime| {
        if running.with(runtime, Vec::is_empty) && open.with_untracked(runtime, |open| *open) {
            open.set(runtime, false);
        }
    });
    show(
        bind(move |runtime: &mut Runtime| running.with(runtime, |rows| !rows.is_empty())),
        move || {
            popover(ring_button(store, open, theme), open, move || {
                panel(store, theme)
            })
            .mr(GAP)
        },
        || hstack(()),
    )
    .flex_row()
    .shrink(0.0)
}

fn overall(rows: &[JobRow]) -> f32 {
    rows.iter().map(|row| row.fraction).sum::<f32>() / rows.len().max(1) as f32
}

fn ring_button(store: Store, open: Signal<bool>, theme: Signal<Theme>) -> impl View {
    let running = store.jobs.running;
    let ring = canvas(move |runtime, size, list| {
        let fraction = running.with(runtime, |rows| overall(rows));
        let (track, arc) = theme.with(runtime, |theme| {
            (theme.surface.hairline, theme.accent.start)
        });
        let center = Point::new(size.width / 2.0, size.height / 2.0);
        let radius = (RING - RING_WIDTH) / 2.0;
        let bounds = Rect::new(
            Point::new(center.x - radius, center.y - radius),
            Size::new(radius * 2.0, radius * 2.0),
        );
        list.stroke(bounds, CornerRadii::uniform(radius), RING_WIDTH, track);
        if fraction > 0.0 {
            list.push(DrawCommand::StrokePath(
                arc_path(center, radius, fraction),
                Stroke::rounded(RING_WIDTH),
                Paint::solid(arc),
            ));
        }
    })
    .size(RING);
    hstack(ring)
        .size(ANCHOR)
        .items_center()
        .justify_center()
        .rounded(ANCHOR / 2.0)
        .role(Role::Button)
        .label("File operations")
        .on_click(move |cx, _| open.update(cx, |open| *open = !*open))
}

/// A clockwise arc from twelve o'clock over `fraction` of the circle.
fn arc_path(center: Point, radius: f32, fraction: f32) -> Path {
    let steps = ((ARC_SEGMENTS as f32 * fraction).ceil() as usize).max(1);
    let at = |step: usize| {
        let angle = TAU * fraction * step as f32 / steps as f32 - FRAC_PI_2;
        Point::new(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        )
    };
    (1..=steps).fold(Path::new().move_to(at(0)), |path, step| {
        path.line_to(at(step))
    })
}

fn panel(store: Store, theme: Signal<Theme>) -> impl View {
    vstack(
        for_each(
            store.jobs.running,
            |row: &JobRow| row.id,
            move |cx, row| job_row(cx, store, row),
        )
        .flex_col()
        .gap(10.0),
    )
    .w(PANEL_WIDTH)
    .ml(ANCHOR - PANEL_WIDTH)
    .p(12.0)
    .rounded(PANEL_RADIUS)
    .bg(tint(theme, |theme| theme.popover.bg))
    .border(1.0)
    .border_color(tint(theme, |theme| theme.popover.border))
    .shadow(Shadow::lg())
    .label("File operations")
}

fn job_row(cx: &mut Cx, store: Store, row: Signal<JobRow>) -> impl View + use<> {
    let theme = use_theme(cx);
    let id = row.with(cx, |row| row.id);
    vstack((
        hstack((
            text(bind(move |runtime: &mut Runtime| {
                row.with(runtime, |row| row.label.clone())
            }))
            .text_size(12.0)
            .truncate()
            .min_w(0.0)
            .flex_1()
            .text_color(tint(theme, |theme| theme.popover.text)),
            icon_button(icons::X, "Cancel")
                .variant(ButtonVariant::Ghost)
                .on_click(move |cx, _| cancel_job(cx, store, id)),
        ))
        .items_center()
        .gap(6.0),
        progress(bind(move |runtime: &mut Runtime| {
            row.with(runtime, |row| row.fraction)
        }))
        .h(6.0),
    ))
    .gap(4.0)
}
