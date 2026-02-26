/// Cairo-based renderer implementation.

use crate::core::types::{Color, Vec2};
use crate::platform::renderer::Renderer;
use cairo::Context;
use std::f64::consts::PI;

pub struct RendererCairo {
    cr: Context,
}

impl RendererCairo {
    pub fn new(cr: Context) -> Self {
        Self { cr }
    }

    /// Update the Cairo context (e.g., after window resize).
    pub fn set_context(&mut self, cr: Context) {
        self.cr = cr;
    }

    fn set_color(&self, color: Color) {
        self.cr.set_source_rgba(color.r, color.g, color.b, color.a);
    }

    fn rounded_rect_path(&self, x: f64, y: f64, w: f64, h: f64, r: f64) {
        self.cr.new_path();
        self.cr.arc(x + w - r, y + r, r, -PI / 2.0, 0.0);
        self.cr.arc(x + w - r, y + h - r, r, 0.0, PI / 2.0);
        self.cr.arc(x + r, y + h - r, r, PI / 2.0, PI);
        self.cr.arc(x + r, y + r, r, PI, 3.0 * PI / 2.0);
        self.cr.close_path();
    }

    fn bezier_point(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f64) -> Vec2 {
        let u = 1.0 - t;
        let tt = t * t;
        let uu = u * u;
        let uuu = uu * u;
        let ttt = tt * t;

        Vec2 {
            x: uuu * p0.x + 3.0 * uu * t * p1.x + 3.0 * u * tt * p2.x + ttt * p3.x,
            y: uuu * p0.y + 3.0 * uu * t * p1.y + 3.0 * u * tt * p2.y + ttt * p3.y,
        }
    }
}

impl Renderer for RendererCairo {
    fn begin_frame(&mut self, _width: i32, _height: i32) {
        self.cr.save().ok();
    }

    fn end_frame(&mut self) {
        self.cr.restore().ok();
    }

    fn fill_rect(&self, x: f64, y: f64, w: f64, h: f64, color: Color) {
        self.set_color(color);
        self.cr.rectangle(x, y, w, h);
        self.cr.fill().ok();
    }

    fn fill_rounded_rect(&self, x: f64, y: f64, w: f64, h: f64, radius: f64, color: Color) {
        self.set_color(color);
        self.rounded_rect_path(x, y, w, h, radius);
        self.cr.fill().ok();
    }

    fn stroke_rounded_rect(
        &self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        radius: f64,
        color: Color,
        line_width: f64,
    ) {
        self.set_color(color);
        self.cr.set_line_width(line_width);
        self.rounded_rect_path(x, y, w, h, radius);
        self.cr.stroke().ok();
    }

    fn draw_text(&self, x: f64, y: f64, text: &str, size: f64, color: Color) {
        self.set_color(color);
        self.cr
            .select_font_face("monospace", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        self.cr.set_font_size(size);
        self.cr.move_to(x, y + size);
        self.cr.show_text(text).ok();
    }

    fn draw_bezier(&self, p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, color: Color, line_width: f64) {
        self.set_color(color);
        self.cr.set_line_width(line_width);
        self.cr.new_path();
        self.cr.move_to(p0.x, p0.y);
        self.cr.curve_to(p1.x, p1.y, p2.x, p2.y, p3.x, p3.y);
        self.cr.stroke().ok();
    }

    fn fill_circle(&self, cx: f64, cy: f64, radius: f64, color: Color) {
        self.set_color(color);
        self.cr.new_path();
        self.cr.arc(cx, cy, radius, 0.0, 2.0 * PI);
        self.cr.fill().ok();
    }

    fn draw_bezier_dashed(
        &self,
        p0: Vec2,
        p1: Vec2,
        p2: Vec2,
        p3: Vec2,
        color: Color,
        line_width: f64,
        dash_on: f64,
        dash_off: f64,
    ) {
        self.set_color(color);
        self.cr.set_line_width(line_width);
        self.cr.set_dash(&[dash_on, dash_off], 0.0);
        self.cr.new_path();
        self.cr.move_to(p0.x, p0.y);
        self.cr.curve_to(p1.x, p1.y, p2.x, p2.y, p3.x, p3.y);
        self.cr.stroke().ok();
        self.cr.set_dash(&[], 0.0);
    }

    fn push_clip(&self, x: f64, y: f64, w: f64, h: f64) {
        self.cr.save().ok();
        self.cr.rectangle(x, y, w, h);
        self.cr.clip();
    }

    fn pop_clip(&self) {
        self.cr.restore().ok();
    }

    fn push_transform(&self, offset: Vec2, scale: f64) {
        self.cr.save().ok();
        self.cr.translate(offset.x, offset.y);
        self.cr.scale(scale, scale);
    }

    fn pop_transform(&self) {
        self.cr.restore().ok();
    }

    fn draw_shadow(
        &self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        radius: f64,
        color: Color,
        blur: f64,
    ) {
        // Approximate shadow with multiple expanding rounded rects
        let steps = 5;
        for i in 0..steps {
            let expand = blur * (i as f64 + 1.0) / steps as f64;
            let alpha = color.a * (1.0 - i as f64 / steps as f64) * 0.3;
            self.fill_rounded_rect(
                x - expand,
                y - expand + 2.0,
                w + expand * 2.0,
                h + expand * 2.0,
                radius + expand,
                Color {
                    r: color.r,
                    g: color.g,
                    b: color.b,
                    a: alpha,
                },
            );
        }
    }
}
