/// Abstract rendering interface.

use crate::core::types::{Color, Vec2};

pub trait Renderer {
    fn begin_frame(&mut self, width: i32, height: i32);
    fn end_frame(&mut self);

    // Primitives
    fn fill_rect(&self, x: f64, y: f64, w: f64, h: f64, color: Color);
    fn fill_rounded_rect(&self, x: f64, y: f64, w: f64, h: f64, radius: f64, color: Color);
    fn stroke_rounded_rect(
        &self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        radius: f64,
        color: Color,
        line_width: f64,
    );
    fn draw_text(&self, x: f64, y: f64, text: &str, size: f64, color: Color);

    // Bezier curve
    fn draw_bezier(&self, p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, color: Color, line_width: f64);

    // Circle
    fn fill_circle(&self, cx: f64, cy: f64, radius: f64, color: Color);

    // Dashed line (bezier)
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
    );

    // Clipping
    fn push_clip(&self, x: f64, y: f64, w: f64, h: f64);
    fn pop_clip(&self);

    // Transform (camera)
    fn push_transform(&self, offset: Vec2, scale: f64);
    fn pop_transform(&self);

    // Shadow (glow effect)
    fn draw_shadow(
        &self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        radius: f64,
        color: Color,
        blur: f64,
    );
}
