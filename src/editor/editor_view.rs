/// Editor view using syntect for syntax highlighting, rendered via Cairo.

use crate::core::config;
use crate::core::types::Color;
use crate::platform::renderer::Renderer;

pub struct EditorView {
    lines: Vec<String>,
    file_path: String,
    scroll_offset: usize,
    highlight_line: Option<u32>,
    visible: bool,
}

impl EditorView {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            file_path: String::new(),
            scroll_offset: 0,
            highlight_line: None,
            visible: false,
        }
    }

    pub fn load_file(&mut self, path: &str) -> bool {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                self.lines = content.lines().map(|l| l.to_string()).collect();
                self.file_path = path.to_string();
                self.scroll_offset = 0;
                true
            }
            Err(_) => false,
        }
    }

    pub fn goto_line(&mut self, line: u32) {
        if line > 0 {
            let target = (line as usize).saturating_sub(1);
            // Center the target line in the view
            self.scroll_offset = target.saturating_sub(10);
        }
    }

    pub fn set_highlight_line(&mut self, line: u32) {
        self.highlight_line = Some(line);
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Render the editor content within the given bounds.
    pub fn render(&self, renderer: &dyn Renderer, x: f64, y: f64, w: f64, h: f64) {
        if !self.visible || self.lines.is_empty() {
            return;
        }

        // Background
        renderer.fill_rect(x, y, w, h, Color::from_hex(config::NODE_BG, 1.0));

        let line_height = 16.0;
        let font_size = 12.0;
        let gutter_width = 48.0;
        let max_visible = ((h / line_height) as usize).min(self.lines.len() - self.scroll_offset);

        for i in 0..max_visible {
            let line_idx = self.scroll_offset + i;
            if line_idx >= self.lines.len() {
                break;
            }

            let ly = y + i as f64 * line_height;

            // Highlight error line
            if self.highlight_line == Some((line_idx + 1) as u32) {
                renderer.fill_rect(x, ly, w, line_height, Color::from_hex(0x3D1E28, 0.6));
            }

            // Line number
            let line_num = format!("{:>4}", line_idx + 1);
            renderer.draw_text(
                x + 4.0,
                ly + 2.0,
                &line_num,
                font_size,
                Color::from_hex(config::TEXT_SECONDARY, 0.7),
            );

            // Gutter separator
            renderer.fill_rect(
                x + gutter_width - 2.0,
                ly,
                1.0,
                line_height,
                Color::from_hex(config::NODE_BORDER, 0.3),
            );

            // Code text (basic - Phase 3 will add syntect highlighting)
            let line_text = &self.lines[line_idx];
            let display = if line_text.len() > 80 {
                &line_text[..80]
            } else {
                line_text
            };

            renderer.draw_text(
                x + gutter_width + 4.0,
                ly + 2.0,
                display,
                font_size,
                Color::from_hex(config::TEXT_PRIMARY, 1.0),
            );
        }
    }
}

impl Default for EditorView {
    fn default() -> Self {
        Self::new()
    }
}
