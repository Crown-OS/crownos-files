//! A recorded `DrawList` written out as SVG elements, so resvg can paint
//! the same drawing on the CPU where no GPU context can be had.

use std::fmt::Write;

use crownui::{
    ext::{Brush, DrawCommand, DrawList, Paint, Path, PathCommand, Stroke, StrokeCap, StrokeJoin},
    prelude::{Color, CornerRadii, Rect},
};

/// SVG for `list`, with every definition it needs inline.
pub fn draw_list_svg(list: &DrawList) -> String {
    let mut writer = SvgWriter::default();
    for command in list.commands() {
        writer.command(command);
    }
    while writer.open_groups.pop().is_some() {
        writer.svg.push_str("</g>");
    }
    writer.svg
}

#[derive(Default)]
struct SvgWriter {
    svg: String,
    next_id: usize,
    /// Groups opened since each unmatched `Save`, innermost last.
    open_groups: Vec<usize>,
}

impl SvgWriter {
    fn command(&mut self, command: &DrawCommand) {
        match command {
            DrawCommand::Save => self.open_groups.push(0),
            DrawCommand::SaveLayer { opacity, .. } => {
                let _ = write!(self.svg, r#"<g opacity="{opacity}">"#);
                self.open_groups.push(1);
            }
            DrawCommand::Restore => {
                for _ in 0..self.open_groups.pop().unwrap_or_default() {
                    self.svg.push_str("</g>");
                }
            }
            DrawCommand::Transform(transform) => {
                let [a, b, c, d, e, f] = transform.matrix;
                self.open(&format!(r#"transform="matrix({a} {b} {c} {d} {e} {f})""#));
            }
            DrawCommand::ClipRect(rect) => self.clip(&rounded_rect_path(*rect, CornerRadii::ZERO)),
            DrawCommand::ClipRoundedRect(rect, radii) => {
                self.clip(&rounded_rect_path(*rect, *radii));
            }
            DrawCommand::FillRect(rect, paint) => {
                self.fill(&rounded_rect_path(*rect, CornerRadii::ZERO), paint);
            }
            DrawCommand::FillRoundedRect(rect, radii, paint) => {
                self.fill(&rounded_rect_path(*rect, *radii), paint);
            }
            DrawCommand::StrokeRoundedRect(rect, radii, stroke, paint) => {
                self.stroke(&rounded_rect_path(*rect, *radii), stroke, paint);
            }
            DrawCommand::FillPath(path, paint) => self.fill(&path_data(path), paint),
            DrawCommand::StrokePath(path, stroke, paint) => {
                self.stroke(&path_data(path), stroke, paint);
            }
            DrawCommand::Shadow {
                rect,
                radii,
                color,
                blur,
                offset,
            } => {
                let cast = Rect {
                    origin: rect.origin + *offset,
                    size: rect.size,
                };
                let paint = Paint {
                    brush: Brush::Solid(*color),
                    blur: Some(blur / 2.0),
                };
                self.fill(&rounded_rect_path(cast, *radii), &paint);
            }
            DrawCommand::Text(..) | DrawCommand::Image(..) => {}
        }
    }

    /// Opens a group that closes with the innermost `Save`.
    fn open(&mut self, attributes: &str) {
        let _ = write!(self.svg, "<g {attributes}>");
        match self.open_groups.last_mut() {
            Some(count) => *count += 1,
            None => self.open_groups.push(1),
        }
    }

    fn clip(&mut self, data: &str) {
        let id = self.id();
        let _ = write!(
            self.svg,
            r#"<clipPath id="c{id}"><path d="{data}"/></clipPath>"#
        );
        self.open(&format!(r#"clip-path="url(#c{id})""#));
    }

    fn fill(&mut self, data: &str, paint: &Paint) {
        let fill = self.paint(paint);
        let filter = self.blur(paint.blur);
        let _ = write!(self.svg, r#"<path d="{data}" {fill}{filter}/>"#);
    }

    fn stroke(&mut self, data: &str, stroke: &Stroke, paint: &Paint) {
        let color = self.paint(paint).replace("fill", "stroke");
        let cap = match stroke.cap {
            StrokeCap::Butt => "butt",
            StrokeCap::Round => "round",
            StrokeCap::Square => "square",
        };
        let join = match stroke.join {
            StrokeJoin::Miter => "miter",
            StrokeJoin::Round => "round",
            StrokeJoin::Bevel => "bevel",
        };
        let _ = write!(
            self.svg,
            r#"<path d="{data}" fill="none" {color} stroke-width="{}" stroke-linecap="{cap}" stroke-linejoin="{join}"/>"#,
            stroke.width
        );
    }

    /// The `fill` attributes of `paint`, defining its gradient first.
    fn paint(&mut self, paint: &Paint) -> String {
        match &paint.brush {
            Brush::Solid(color) => solid(*color),
            Brush::LinearGradient { start, end, stops } => {
                let id = self.id();
                let _ = write!(
                    self.svg,
                    r#"<linearGradient id="g{id}" gradientUnits="userSpaceOnUse" x1="{}" y1="{}" x2="{}" y2="{}">"#,
                    start.x, start.y, end.x, end.y
                );
                for stop in stops {
                    let (hex, alpha) = hex(stop.color);
                    let _ = write!(
                        self.svg,
                        r#"<stop offset="{}" stop-color="{hex}" stop-opacity="{alpha}"/>"#,
                        stop.offset
                    );
                }
                self.svg.push_str("</linearGradient>");
                format!(r#"fill="url(#g{id})""#)
            }
            Brush::Image { .. } => String::from(r#"fill="none""#),
        }
    }

    fn blur(&mut self, blur: Option<f32>) -> String {
        let Some(deviation) = blur.filter(|blur| *blur > 0.0) else {
            return String::new();
        };
        let id = self.id();
        let _ = write!(
            self.svg,
            r#"<filter id="f{id}" x="-1" y="-1" width="3" height="3"><feGaussianBlur stdDeviation="{deviation}"/></filter>"#
        );
        format!(r#" filter="url(#f{id})""#)
    }

    fn id(&mut self) -> usize {
        self.next_id += 1;
        self.next_id
    }
}

/// `#rrggbb` and the alpha of `color`.
pub fn hex(color: Color) -> (String, f32) {
    let [red, green, blue, alpha] = color.to_rgba8();
    (
        format!("#{red:02x}{green:02x}{blue:02x}"),
        f32::from(alpha) / 255.0,
    )
}

fn solid(color: Color) -> String {
    let (hex, alpha) = hex(color);
    format!(r#"fill="{hex}" fill-opacity="{alpha}""#)
}

fn path_data(path: &Path) -> String {
    let mut data = String::new();
    for command in &path.commands {
        let _ = match command {
            PathCommand::MoveTo(to) => write!(data, "M{} {}", to.x, to.y),
            PathCommand::LineTo(to) => write!(data, "L{} {}", to.x, to.y),
            PathCommand::QuadTo(control, to) => {
                write!(data, "Q{} {} {} {}", control.x, control.y, to.x, to.y)
            }
            PathCommand::CubicTo(first, second, to) => write!(
                data,
                "C{} {} {} {} {} {}",
                first.x, first.y, second.x, second.y, to.x, to.y
            ),
            PathCommand::Close => write!(data, "Z"),
        };
    }
    data
}

/// A rounded rectangle as path data, each corner clamped to half the side.
pub fn rounded_rect_path(rect: Rect, radii: CornerRadii) -> String {
    let (x, y) = (rect.origin.x, rect.origin.y);
    let (width, height) = (rect.size.width, rect.size.height);
    let limit = width.min(height) / 2.0;
    let [top_left, top_right, bottom_right, bottom_left] = [
        radii.top_left,
        radii.top_right,
        radii.bottom_right,
        radii.bottom_left,
    ]
    .map(|radius| radius.clamp(0.0, limit));
    let (right, bottom) = (x + width, y + height);
    format!(
        "M{} {y}H{}A{top_right} {top_right} 0 0 1 {right} {}V{}A{bottom_right} {bottom_right} 0 0 1 {} {bottom}H{}A{bottom_left} {bottom_left} 0 0 1 {x} {}V{}A{top_left} {top_left} 0 0 1 {} {y}Z",
        x + top_left,
        right - top_right,
        y + top_right,
        bottom - bottom_right,
        right - bottom_right,
        x + bottom_left,
        bottom - bottom_left,
        y + top_left,
        x + top_left,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crownui::prelude::{Point, Size, rgb};

    #[test]
    fn saves_scope_their_transforms_and_clips() {
        let mut list = DrawList::new();
        list.save();
        list.transform(crownui::ext::Transform::scale(2.0, 2.0));
        list.clip(
            Rect::new(Point::ZERO, Size::new(4.0, 4.0)),
            CornerRadii::ZERO,
        );
        list.fill(
            Rect::new(Point::ZERO, Size::new(4.0, 4.0)),
            CornerRadii::ZERO,
            rgb(0xff0000),
        );
        list.restore();
        let svg = draw_list_svg(&list);
        assert!(svg.contains(r#"transform="matrix(2 0 0 2 0 0)""#), "{svg}");
        assert!(svg.contains(r#"clip-path="url(#c1)""#), "{svg}");
        assert!(svg.contains(r##"fill="#ff0000""##), "{svg}");
        assert_eq!(svg.matches("<g ").count(), svg.matches("</g>").count());
    }
}
