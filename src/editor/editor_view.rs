/// Editor view with syntect syntax highlighting, rendered via Cairo.

use crate::core::config;
use crate::core::types::Color;
use crate::platform::renderer::Renderer;
use std::path::Path;
use syntect::highlighting::{self, ThemeSet};
use syntect::parsing::SyntaxSet;

/// A single highlighted token with position and color.
struct HighlightedToken {
    text: String,
    color: Color,
}

/// Pre-highlighted line: sequence of colored tokens.
struct HighlightedLine {
    tokens: Vec<HighlightedToken>,
}

pub struct EditorView {
    lines: Vec<String>,
    highlighted: Vec<HighlightedLine>,
    file_path: String,
    scroll_offset: usize,
    highlight_line: Option<u32>,
    visible: bool,
    syntax_set: SyntaxSet,
    theme: highlighting::Theme,
}

impl EditorView {
    pub fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        // Use a dark theme that matches our color scheme
        let theme = theme_set
            .themes
            .get("base16-ocean.dark")
            .cloned()
            .unwrap_or_else(|| theme_set.themes.values().next().unwrap().clone());

        Self {
            lines: Vec::new(),
            highlighted: Vec::new(),
            file_path: String::new(),
            scroll_offset: 0,
            highlight_line: None,
            visible: false,
            syntax_set,
            theme,
        }
    }

    pub fn load_file(&mut self, path: &str) -> bool {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return false,
        };

        self.lines = content.lines().map(|l| l.to_string()).collect();
        self.file_path = path.to_string();
        self.scroll_offset = 0;

        // Syntax highlight all lines
        self.highlight_content(&content, path);

        true
    }

    pub fn goto_line(&mut self, line: u32) {
        if line > 0 {
            let target = (line as usize).saturating_sub(1);
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
        renderer.fill_rect(x, y, w, h, Color::from_hex(0x0D1117, 1.0));

        let line_height = 16.0;
        let font_size = 12.0;
        let gutter_width = 48.0;
        let visible_lines = (h / line_height) as usize;
        let max_visible = visible_lines.min(self.lines.len().saturating_sub(self.scroll_offset));

        for i in 0..max_visible {
            let line_idx = self.scroll_offset + i;
            if line_idx >= self.lines.len() {
                break;
            }

            let ly = y + i as f64 * line_height;

            // Error line highlight
            if self.highlight_line == Some((line_idx + 1) as u32) {
                renderer.fill_rect(x, ly, w, line_height, Color::from_hex(0x3D1E28, 0.6));
                // Error line indicator bar
                renderer.fill_rect(x, ly, 3.0, line_height, Color::from_hex(config::ERROR_BORDER, 0.9));
            }

            // Line number
            let line_num = format!("{:>4}", line_idx + 1);
            let ln_color = if self.highlight_line == Some((line_idx + 1) as u32) {
                Color::from_hex(config::ERROR_BORDER, 0.8)
            } else {
                Color::from_hex(config::TEXT_SECONDARY, 0.5)
            };
            renderer.draw_text(x + 4.0, ly + 2.0, &line_num, font_size, ln_color);

            // Gutter separator
            renderer.fill_rect(
                x + gutter_width - 2.0,
                ly,
                1.0,
                line_height,
                Color::from_hex(config::NODE_BORDER, 0.3),
            );

            // Syntax-highlighted code text
            let code_x = x + gutter_width + 4.0;
            if line_idx < self.highlighted.len() {
                let hl_line = &self.highlighted[line_idx];
                let mut cx = code_x;
                for token in &hl_line.tokens {
                    renderer.draw_text(cx, ly + 2.0, &token.text, font_size, token.color);
                    // Approximate character width for monospace font
                    cx += token.text.len() as f64 * 7.2;
                }
            } else {
                // Fallback: uncolored text
                let line_text = &self.lines[line_idx];
                let display = if line_text.len() > 80 {
                    &line_text[..80]
                } else {
                    line_text
                };
                renderer.draw_text(
                    code_x,
                    ly + 2.0,
                    display,
                    font_size,
                    Color::from_hex(config::TEXT_PRIMARY, 1.0),
                );
            }
        }
    }

    /// Perform syntax highlighting on the full file content.
    fn highlight_content(&mut self, content: &str, file_path: &str) {
        let ext = Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("txt");

        let syntax = self
            .syntax_set
            .find_syntax_by_extension(ext)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut highlighter = syntect::easy::HighlightLines::new(syntax, &self.theme);

        self.highlighted.clear();
        for line in content.lines() {
            let line_with_nl = format!("{}\n", line);
            match highlighter.highlight_line(&line_with_nl, &self.syntax_set) {
                Ok(ranges) => {
                    let tokens: Vec<HighlightedToken> = ranges
                        .iter()
                        .map(|(style, text)| {
                            let text = text.trim_end_matches('\n').to_string();
                            HighlightedToken {
                                text,
                                color: syntect_color_to_color(style.foreground),
                            }
                        })
                        .filter(|t| !t.text.is_empty())
                        .collect();
                    self.highlighted.push(HighlightedLine { tokens });
                }
                Err(_) => {
                    self.highlighted.push(HighlightedLine {
                        tokens: vec![HighlightedToken {
                            text: line.to_string(),
                            color: Color::from_hex(config::TEXT_PRIMARY, 1.0),
                        }],
                    });
                }
            }
        }
    }
}

impl Default for EditorView {
    fn default() -> Self {
        Self::new()
    }
}

fn syntect_color_to_color(c: highlighting::Color) -> Color {
    Color {
        r: c.r as f64 / 255.0,
        g: c.g as f64 / 255.0,
        b: c.b as f64 / 255.0,
        a: c.a as f64 / 255.0,
    }
}
