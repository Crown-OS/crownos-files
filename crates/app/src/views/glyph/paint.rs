//! Drawing helpers the glyph painters share.

use crownui::{
    ext::{Brush, DrawCommand, DrawList, GradientStop, Paint, Path, Stroke, Transform},
    prelude::{Color, CornerRadii, Point, Rect, Size, Vector},
};

/// A top-to-bottom gradient over `top..bottom`.
pub fn vertical(top: f32, bottom: f32, (from, to): (Color, Color)) -> Paint {
    linear(Point::new(0.0, top), Point::new(0.0, bottom), &[from, to])
}

/// A gradient through evenly spaced `colors` from `start` to `end`.
pub fn linear(start: Point, end: Point, colors: &[Color]) -> Paint {
    let last = colors.len().saturating_sub(1).max(1) as f32;
    Paint {
        brush: Brush::LinearGradient {
            start,
            end,
            stops: colors
                .iter()
                .enumerate()
                .map(|(index, &color)| GradientStop {
                    offset: index as f32 / last,
                    color,
                })
                .collect(),
        },
        blur: None,
    }
}

pub fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::new(Point::new(x, y), Size::new(width, height))
}

pub fn fill_path(list: &mut DrawList, path: Path, paint: impl Into<Paint>) {
    list.push(DrawCommand::FillPath(path, paint.into()));
}

pub fn stroke_path(list: &mut DrawList, path: Path, width: f32, color: Color) {
    list.push(DrawCommand::StrokePath(
        path,
        Stroke::rounded(width),
        Paint::solid(color),
    ));
}

/// A 1px line just inside a rounded rectangle, like CSS `inset 0 0 0 1px`.
pub fn inner_ring(list: &mut DrawList, bounds: Rect, radius: f32, color: Color) {
    let inset = rect(
        bounds.min_x() + 0.5,
        bounds.min_y() + 0.5,
        bounds.size.width - 1.0,
        bounds.size.height - 1.0,
    );
    list.stroke(inset, CornerRadii::uniform(radius - 0.5), 1.0, color);
}

/// The soft drop shadow under photos, discs and archives (`0 6px 14px -6px`).
pub fn drop_shadow(list: &mut DrawList, bounds: Rect, radius: f32, color: Color) {
    let spread = 6.0;
    let casting = rect(
        bounds.min_x() + spread,
        bounds.min_y() + spread,
        bounds.size.width - 2.0 * spread,
        bounds.size.height - 2.0 * spread,
    );
    list.shadow(
        casting,
        CornerRadii::uniform(radius),
        color,
        14.0,
        Vector::new(0.0, 6.0),
    );
}

/// Draws `paint` scaled by `scale` from the canvas origin.
pub fn scaled(list: &mut DrawList, scale: f32, paint: impl FnOnce(&mut DrawList)) {
    list.save();
    list.transform(Transform::scale(scale, scale));
    paint(list);
    list.restore();
}

/// A page outline: a rounded rectangle whose top-right corner is cut off by the fold.
pub fn page_outline(width: f32, height: f32, fold: Size, radius: f32) -> Path {
    let fold_left = width - fold.width;
    Path::new()
        .move_to(Point::new(radius, 0.0))
        .line_to(Point::new(fold_left, 0.0))
        .line_to(Point::new(width, fold.height))
        .line_to(Point::new(width, height - radius))
        .quad_to(
            Point::new(width, height),
            Point::new(width - radius, height),
        )
        .line_to(Point::new(radius, height))
        .quad_to(Point::new(0.0, height), Point::new(0.0, height - radius))
        .line_to(Point::new(0.0, radius))
        .quad_to(Point::ZERO, Point::new(radius, 0.0))
        .close()
}
