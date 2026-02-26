/// Node card rendering and hit testing.

use crate::core::config;
use crate::core::types::*;
use crate::platform::renderer::Renderer;

pub struct GraphNodeRenderer;

impl GraphNodeRenderer {
    pub fn new() -> Self {
        Self
    }

    /// Render a collapsed node card.
    pub fn render_collapsed(
        &self,
        renderer: &dyn Renderer,
        node: &GraphNode,
        hover_t: f64,
        focus_t: f64,
    ) {
        let opacity = lerp_f64(1.0, 0.3, focus_t);
        let scale = lerp_f64(1.0, 0.95, focus_t);

        let x = node.x;
        let y = node.y;
        let w = node.width * scale;
        let h = node.height * scale;
        let r = config::NODE_CORNER_RADIUS;

        // Shadow on hover
        if hover_t > 0.01 {
            renderer.draw_shadow(
                x,
                y,
                w,
                h,
                r,
                Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.5 * hover_t * opacity,
                },
                config::HOVER_SHADOW_GROW * hover_t,
            );
        }

        // Error glow
        if node.is_error {
            renderer.draw_shadow(
                x,
                y,
                w,
                h,
                r,
                Color::from_hex(config::ERROR_BORDER, 0.3 * opacity),
                8.0,
            );
        }

        // Background
        renderer.fill_rounded_rect(
            x,
            y,
            w,
            h,
            r,
            Color::from_hex(config::NODE_BG, opacity),
        );

        // Border
        let border_color = if node.is_error {
            Color::from_hex(config::ERROR_BORDER, opacity)
        } else if hover_t > 0.01 {
            let base = Color::from_hex(config::NODE_BORDER, opacity);
            let highlight = Color::from_hex(config::EDGE_CALL, opacity);
            lerp_color(base, highlight, hover_t)
        } else {
            Color::from_hex(config::NODE_BORDER, opacity)
        };

        renderer.stroke_rounded_rect(x, y, w, h, r, border_color, 1.0 + hover_t);

        // Header: file:line [type]
        let header = if node.file_path.is_empty() {
            format!("{}", node.symbol_name)
        } else {
            let filename = node
                .file_path
                .rsplit('/')
                .next()
                .unwrap_or(&node.file_path);
            format!("{}:{}", filename, node.line)
        };

        let type_label = match node.node_type {
            NodeType::Function => "fn",
            NodeType::Type => "type",
            NodeType::Variable => "var",
            NodeType::Include => "inc",
            NodeType::ErrorSource => "err",
        };

        // Node type indicator dot
        let dot_color = if node.is_error {
            Color::from_hex(config::ERROR_BORDER, opacity)
        } else {
            match node.node_type {
                NodeType::Function => Color::from_hex(config::EDGE_CALL, opacity),
                NodeType::Type => Color::from_hex(config::EDGE_INHERIT, opacity),
                NodeType::Include => Color::from_hex(config::EDGE_INCLUDE, opacity),
                _ => Color::from_hex(config::TEXT_SECONDARY, opacity),
            }
        };
        renderer.fill_circle(x + 12.0, y + 14.0, 4.0, dot_color);

        // Header text
        renderer.draw_text(
            x + 22.0,
            y + 6.0,
            &header,
            12.0,
            Color::from_hex(config::TEXT_PRIMARY, opacity),
        );

        // Type badge
        renderer.draw_text(
            x + w - 30.0,
            y + 6.0,
            type_label,
            10.0,
            Color::from_hex(config::TEXT_SECONDARY, opacity),
        );

        // Separator line
        renderer.fill_rect(
            x + 8.0,
            y + 26.0,
            w - 16.0,
            1.0,
            Color::from_hex(config::NODE_BORDER, opacity * 0.5),
        );

        // Symbol name
        let display_name = truncate_str(&node.symbol_name, 24);
        renderer.draw_text(
            x + 10.0,
            y + 34.0,
            &display_name,
            11.0,
            Color::from_hex(config::TEXT_PRIMARY, opacity),
        );

        // Code preview placeholder
        renderer.draw_text(
            x + 10.0,
            y + 52.0,
            "// ...",
            10.0,
            Color::from_hex(config::TEXT_SECONDARY, opacity * 0.7),
        );
    }

    /// Hit test in graph coordinates.
    pub fn hit_test(&self, node: &GraphNode, point: Vec2) -> bool {
        point.x >= node.x
            && point.x <= node.x + node.width
            && point.y >= node.y
            && point.y <= node.y + node.height
    }
}

fn lerp_f64(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn lerp_color(a: Color, b: Color, t: f64) -> Color {
    Color {
        r: lerp_f64(a.r, b.r, t),
        g: lerp_f64(a.g, b.g, t),
        b: lerp_f64(a.b, b.b, t),
        a: lerp_f64(a.a, b.a, t),
    }
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
