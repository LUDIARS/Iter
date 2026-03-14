/// Software renderer implementation using tiny-skia + fontdue.

use crate::core::types::{Color, Vec2};
use crate::platform::renderer::Renderer;
use std::cell::RefCell;
use tiny_skia::{
    FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, StrokeDash, Transform,
};

/// Rasterized font glyph cache.
struct FontCache {
    font: fontdue::Font,
}

impl FontCache {
    fn new() -> Self {
        let font_data = include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf");
        let font = fontdue::Font::from_bytes(
            font_data as &[u8],
            fontdue::FontSettings::default(),
        )
        .expect("Failed to load embedded font");
        Self { font }
    }

    fn rasterize(&self, ch: char, size: f32) -> (fontdue::Metrics, Vec<u8>) {
        self.font.rasterize(ch, size)
    }
}

pub struct RendererSoft {
    pixmap: RefCell<Pixmap>,
    font_cache: FontCache,
    /// Transform stack: each entry is (offset_x, offset_y, scale)
    transform_stack: RefCell<Vec<(f64, f64, f64)>>,
}

impl RendererSoft {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            pixmap: RefCell::new(Pixmap::new(width.max(1), height.max(1)).expect("Failed to create pixmap")),
            font_cache: FontCache::new(),
            transform_stack: RefCell::new(Vec::new()),
        }
    }

    /// Resize the internal pixmap if needed.
    pub fn resize(&self, width: u32, height: u32) {
        let w = width.max(1);
        let h = height.max(1);
        let mut pixmap = self.pixmap.borrow_mut();
        if pixmap.width() != w || pixmap.height() != h {
            *pixmap = Pixmap::new(w, h).expect("Failed to resize pixmap");
        }
    }

    /// Copy the rendered pixmap into the window's u32 pixel buffer (ARGB format).
    pub fn copy_to_buffer(&self, buf: &mut [u32]) {
        let pixmap = self.pixmap.borrow();
        let data = pixmap.data();
        let pixel_count = (pixmap.width() * pixmap.height()) as usize;
        let len = pixel_count.min(buf.len());
        for i in 0..len {
            let offset = i * 4;
            let r = data[offset] as u32;
            let g = data[offset + 1] as u32;
            let b = data[offset + 2] as u32;
            let a = data[offset + 3] as u32;
            buf[i] = (a << 24) | (r << 16) | (g << 8) | b;
        }
    }

    fn make_paint(color: Color) -> Paint<'static> {
        let mut paint = Paint::default();
        paint.set_color_rgba8(
            (color.r * 255.0) as u8,
            (color.g * 255.0) as u8,
            (color.b * 255.0) as u8,
            (color.a * 255.0) as u8,
        );
        paint.anti_alias = true;
        paint
    }

    fn current_transform(&self) -> Transform {
        let stack = self.transform_stack.borrow();
        if let Some(&(ox, oy, scale)) = stack.last() {
            Transform::from_row(scale as f32, 0.0, 0.0, scale as f32, ox as f32, oy as f32)
        } else {
            Transform::identity()
        }
    }

    fn rounded_rect_path(x: f64, y: f64, w: f64, h: f64, r: f64) -> Option<tiny_skia::Path> {
        let mut pb = PathBuilder::new();
        let (x, y, w, h, r) = (x as f32, y as f32, w as f32, h as f32, r as f32);

        pb.move_to(x + r, y);
        pb.line_to(x + w - r, y);
        pb.quad_to(x + w, y, x + w, y + r);
        pb.line_to(x + w, y + h - r);
        pb.quad_to(x + w, y + h, x + w - r, y + h);
        pb.line_to(x + r, y + h);
        pb.quad_to(x, y + h, x, y + h - r);
        pb.line_to(x, y + r);
        pb.quad_to(x, y, x + r, y);
        pb.close();
        pb.finish()
    }
}

impl Renderer for RendererSoft {
    fn begin_frame(&mut self, width: i32, height: i32) {
        self.resize(width as u32, height as u32);
        self.pixmap.borrow_mut().fill(tiny_skia::Color::TRANSPARENT);
        self.transform_stack.borrow_mut().clear();
    }

    fn end_frame(&mut self) {
        self.transform_stack.borrow_mut().clear();
    }

    fn fill_rect(&self, x: f64, y: f64, w: f64, h: f64, color: Color) {
        let transform = self.current_transform();
        let paint = Self::make_paint(color);
        let rect = match tiny_skia::Rect::from_xywh(x as f32, y as f32, w as f32, h as f32) {
            Some(r) => r,
            None => return,
        };
        self.pixmap.borrow_mut().fill_rect(rect, &paint, transform, None);
    }

    fn fill_rounded_rect(&self, x: f64, y: f64, w: f64, h: f64, radius: f64, color: Color) {
        let transform = self.current_transform();
        let paint = Self::make_paint(color);
        if let Some(path) = Self::rounded_rect_path(x, y, w, h, radius) {
            self.pixmap.borrow_mut().fill_path(&path, &paint, FillRule::Winding, transform, None);
        }
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
        let transform = self.current_transform();
        let paint = Self::make_paint(color);
        let mut stroke = Stroke::default();
        stroke.width = line_width as f32;
        if let Some(path) = Self::rounded_rect_path(x, y, w, h, radius) {
            self.pixmap.borrow_mut().stroke_path(&path, &paint, &stroke, transform, None);
        }
    }

    fn draw_text(&self, x: f64, y: f64, text: &str, size: f64, color: Color) {
        let transform = self.current_transform();
        let size_f32 = size as f32;

        let mut cursor_x = x as f32;
        let cursor_y = y as f32 + size_f32;

        for ch in text.chars() {
            if ch == ' ' {
                cursor_x += size_f32 * 0.6;
                continue;
            }
            if ch == '\t' {
                cursor_x += size_f32 * 0.6 * 4.0;
                continue;
            }

            let (metrics, bitmap) = self.font_cache.rasterize(ch, size_f32);

            if metrics.width == 0 || metrics.height == 0 {
                cursor_x += metrics.advance_width;
                continue;
            }

            if let Some(mut glyph_pixmap) = Pixmap::new(metrics.width as u32, metrics.height as u32) {
                let glyph_data = glyph_pixmap.data_mut();
                for (i, &coverage) in bitmap.iter().enumerate() {
                    let offset = i * 4;
                    if offset + 3 < glyph_data.len() {
                        let a = (coverage as f64 * color.a) as u8;
                        // Premultiplied alpha
                        let pa = a as f64 / 255.0;
                        glyph_data[offset] = (color.r * 255.0 * pa) as u8;
                        glyph_data[offset + 1] = (color.g * 255.0 * pa) as u8;
                        glyph_data[offset + 2] = (color.b * 255.0 * pa) as u8;
                        glyph_data[offset + 3] = a;
                    }
                }

                let gx = cursor_x + metrics.xmin as f32;
                let gy = cursor_y - metrics.height as f32 - metrics.ymin as f32;

                let glyph_offset = Transform::from_translate(gx, gy);
                let combined = transform.pre_concat(glyph_offset);

                self.pixmap.borrow_mut().draw_pixmap(
                    0,
                    0,
                    glyph_pixmap.as_ref(),
                    &tiny_skia::PixmapPaint::default(),
                    combined,
                    None,
                );
            }

            cursor_x += metrics.advance_width;
        }
    }

    fn draw_bezier(
        &self,
        p0: Vec2,
        p1: Vec2,
        p2: Vec2,
        p3: Vec2,
        color: Color,
        line_width: f64,
    ) {
        let transform = self.current_transform();
        let paint = Self::make_paint(color);
        let mut stroke = Stroke::default();
        stroke.width = line_width as f32;
        stroke.line_cap = LineCap::Round;
        stroke.line_join = LineJoin::Round;

        let mut pb = PathBuilder::new();
        pb.move_to(p0.x as f32, p0.y as f32);
        pb.cubic_to(
            p1.x as f32, p1.y as f32,
            p2.x as f32, p2.y as f32,
            p3.x as f32, p3.y as f32,
        );
        if let Some(path) = pb.finish() {
            self.pixmap.borrow_mut().stroke_path(&path, &paint, &stroke, transform, None);
        }
    }

    fn fill_circle(&self, cx: f64, cy: f64, radius: f64, color: Color) {
        let transform = self.current_transform();
        let paint = Self::make_paint(color);

        let r = radius as f32;
        let cx = cx as f32;
        let cy = cy as f32;
        let k = 0.5522847498_f32;
        let kr = k * r;

        let mut pb = PathBuilder::new();
        pb.move_to(cx + r, cy);
        pb.cubic_to(cx + r, cy + kr, cx + kr, cy + r, cx, cy + r);
        pb.cubic_to(cx - kr, cy + r, cx - r, cy + kr, cx - r, cy);
        pb.cubic_to(cx - r, cy - kr, cx - kr, cy - r, cx, cy - r);
        pb.cubic_to(cx + kr, cy - r, cx + r, cy - kr, cx + r, cy);
        pb.close();

        if let Some(path) = pb.finish() {
            self.pixmap.borrow_mut().fill_path(&path, &paint, FillRule::Winding, transform, None);
        }
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
        let transform = self.current_transform();
        let paint = Self::make_paint(color);
        let dash = StrokeDash::new(vec![dash_on as f32, dash_off as f32], 0.0);
        let mut stroke = Stroke::default();
        stroke.width = line_width as f32;
        stroke.line_cap = LineCap::Round;
        stroke.dash = dash;

        let mut pb = PathBuilder::new();
        pb.move_to(p0.x as f32, p0.y as f32);
        pb.cubic_to(
            p1.x as f32, p1.y as f32,
            p2.x as f32, p2.y as f32,
            p3.x as f32, p3.y as f32,
        );
        if let Some(path) = pb.finish() {
            self.pixmap.borrow_mut().stroke_path(&path, &paint, &stroke, transform, None);
        }
    }

    fn push_clip(&self, _x: f64, _y: f64, _w: f64, _h: f64) {
        // Clipping not implemented in software renderer (tiny-skia has limited clip support)
        // For now this is a no-op; the visual difference is minimal
    }

    fn pop_clip(&self) {
        // No-op (see push_clip)
    }

    fn push_transform(&self, offset: Vec2, scale: f64) {
        let mut stack = self.transform_stack.borrow_mut();
        if let Some(&(ox, oy, s)) = stack.last() {
            stack.push((ox + offset.x * s, oy + offset.y * s, s * scale));
        } else {
            stack.push((offset.x, offset.y, scale));
        }
    }

    fn pop_transform(&self) {
        self.transform_stack.borrow_mut().pop();
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
