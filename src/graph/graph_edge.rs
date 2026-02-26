/// Edge rendering with bezier curves and per-type styling.

use crate::core::config;
use crate::core::types::*;
use crate::platform::renderer::Renderer;

pub struct GraphEdgeRenderer;

impl GraphEdgeRenderer {
    pub fn new() -> Self {
        Self
    }

    /// Render an edge between two nodes.
    pub fn render(
        &self,
        renderer: &dyn Renderer,
        edge: &GraphEdge,
        source: &GraphNode,
        target: &GraphNode,
        time_sec: f64,
        focus_opacity: f64,
    ) {
        let (p0, p1, p2, p3) = Self::calc_bezier_points(source, target);

        let (color_hex, line_width, dash) = match edge.edge_type {
            EdgeType::ErrorPath => (config::EDGE_ERROR, 3.0, None),
            EdgeType::Call => (config::EDGE_CALL, 2.0, None),
            EdgeType::Include => (config::EDGE_INCLUDE, 1.0, Some((6.0, 4.0))),
            EdgeType::Inherit => (config::EDGE_INHERIT, 1.0, Some((2.0, 4.0))),
            EdgeType::Reference => (config::EDGE_CALL, 1.5, None),
        };

        let color = Color::from_hex(color_hex, focus_opacity);

        match dash {
            Some((on, off)) => {
                renderer.draw_bezier_dashed(p0, p1, p2, p3, color, line_width, on, off);
            }
            None => {
                renderer.draw_bezier(p0, p1, p2, p3, color, line_width);
            }
        }

        // Connection dots at endpoints
        renderer.fill_circle(p0.x, p0.y, 4.0, color);
        renderer.fill_circle(p3.x, p3.y, 4.0, color);

        // Error path particles
        if edge.edge_type == EdgeType::ErrorPath || edge.on_error_path {
            self.render_error_particles(renderer, p0, p1, p2, p3, time_sec, focus_opacity);
        }
    }

    /// Calculate bezier control points: source right-center to target left-center.
    fn calc_bezier_points(src: &GraphNode, dst: &GraphNode) -> (Vec2, Vec2, Vec2, Vec2) {
        let p0 = Vec2::new(src.x + src.width, src.y + src.height / 2.0);
        let p3 = Vec2::new(dst.x, dst.y + dst.height / 2.0);
        let dx = (p3.x - p0.x).abs() * 0.5;
        let p1 = Vec2::new(p0.x + dx, p0.y);
        let p2 = Vec2::new(p3.x - dx, p3.y);
        (p0, p1, p2, p3)
    }

    /// Render animated particles flowing along the error path.
    fn render_error_particles(
        &self,
        renderer: &dyn Renderer,
        p0: Vec2,
        p1: Vec2,
        p2: Vec2,
        p3: Vec2,
        time_sec: f64,
        opacity: f64,
    ) {
        let color = Color::from_hex(config::EDGE_ERROR, opacity);
        let base_t = (time_sec * 0.5) % 1.0;

        for i in 0..3 {
            let t = (base_t + i as f64 * 0.33) % 1.0;
            let pos = bezier_point(p0, p1, p2, p3, t);
            renderer.fill_circle(pos.x, pos.y, 3.0, color);
        }
    }
}

/// Evaluate a cubic bezier at parameter t.
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
