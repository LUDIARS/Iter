/// Main graph view: camera management, node interaction, and rendering coordination.
/// Integrates EditorView for expanded node display.

use crate::core::config;
use crate::core::types::*;
use crate::editor::editor_view::EditorView;
use crate::graph::animation::{lerp, smooth_towards, Animation};
use crate::graph::graph_edge::GraphEdgeRenderer;
use crate::graph::graph_node::GraphNodeRenderer;
use crate::platform::renderer::Renderer;

pub struct GraphView {
    graph: RelayGraph,

    // Camera (pan + zoom)
    camera_offset: Vec2,
    zoom: f64,

    // Mouse state
    panning: bool,
    pan_start: Vec2,
    camera_start: Vec2,

    // Node interaction
    hovered_node_id: Option<u32>,
    selected_node_id: Option<u32>,
    hover_values: Vec<f64>,

    // Expand/collapse animation
    expand_anim: Animation,
    collapse_anim: Animation,
    expanding_node_id: Option<u32>,
    // Store original position/size for animation
    expand_origin: (f64, f64, f64, f64), // (x, y, w, h)

    // Embedded editor for expanded nodes
    editor: EditorView,

    // Renderers
    node_renderer: GraphNodeRenderer,
    edge_renderer: GraphEdgeRenderer,

    // Timing
    time_sec: f64,

    // View dimensions (updated on render)
    view_width: f64,
    view_height: f64,
}

impl GraphView {
    pub fn new() -> Self {
        Self {
            graph: RelayGraph::default(),
            camera_offset: Vec2::default(),
            zoom: 1.0,
            panning: false,
            pan_start: Vec2::default(),
            camera_start: Vec2::default(),
            hovered_node_id: None,
            selected_node_id: None,
            hover_values: Vec::new(),
            expand_anim: Animation::new(),
            collapse_anim: Animation::new(),
            expanding_node_id: None,
            expand_origin: (0.0, 0.0, 0.0, 0.0),
            editor: EditorView::new(),
            node_renderer: GraphNodeRenderer::new(),
            edge_renderer: GraphEdgeRenderer::new(),
            time_sec: 0.0,
            view_width: 1280.0,
            view_height: 720.0,
        }
    }

    pub fn set_graph(&mut self, graph: RelayGraph) {
        let n = graph.nodes.len();
        self.graph = graph;
        self.hover_values = vec![0.0; n];
        self.hovered_node_id = None;
        self.selected_node_id = None;
        self.expanding_node_id = None;
    }

    pub fn handle_mouse(&mut self, e: &MouseEvent) {
        let graph_pos = self.screen_to_graph(Vec2::new(e.x, e.y));

        // Scroll → zoom
        if e.scroll_y.abs() > 0.01 {
            let zoom_old = self.zoom;
            self.zoom = (self.zoom + e.scroll_y * config::ZOOM_SPEED)
                .clamp(config::ZOOM_MIN, config::ZOOM_MAX);

            let mouse = Vec2::new(e.x, e.y);
            self.camera_offset = mouse - (mouse - self.camera_offset) * (self.zoom / zoom_old);
            return;
        }

        // Middle button drag → pan
        if e.button == 2 {
            if e.pressed {
                self.panning = true;
                self.pan_start = Vec2::new(e.x, e.y);
                self.camera_start = self.camera_offset;
            } else if e.released {
                self.panning = false;
            }
        }

        if self.panning && e.dragging {
            let delta = Vec2::new(e.x, e.y) - self.pan_start;
            self.camera_offset = self.camera_start + delta;
            return;
        }

        // Hover detection
        self.hovered_node_id = None;
        for node in &self.graph.nodes {
            if self.node_renderer.hit_test(node, graph_pos) {
                self.hovered_node_id = Some(node.id);
                break;
            }
        }

        // Left click → select / expand
        if e.button == 1 && e.pressed {
            if let Some(hovered) = self.hovered_node_id {
                if self.selected_node_id == Some(hovered) {
                    if self.expanding_node_id.is_some() {
                        self.collapse_node();
                    }
                } else {
                    if self.expanding_node_id.is_some() {
                        self.collapse_node();
                    }
                    self.selected_node_id = Some(hovered);
                    self.expand_node(hovered);
                }
            } else {
                if self.expanding_node_id.is_some() {
                    self.collapse_node();
                }
                self.selected_node_id = None;
            }
        }
    }

    pub fn handle_key(&mut self, e: &KeyEvent) {
        if !e.pressed {
            return;
        }

        // ESC: collapse expanded node
        if e.keycode == 9 {
            if self.expanding_node_id.is_some() {
                self.collapse_node();
            }
        }

        // Space: fit all
        if e.keycode == 65 && self.expanding_node_id.is_none() {
            self.fit_all();
        }

        // Tab: next error node
        if e.keycode == 23 {
            self.focus_next_error();
        }
    }

    pub fn update(&mut self, dt_ms: f64) {
        self.time_sec += dt_ms / 1000.0;

        self.expand_anim.update(dt_ms);
        self.collapse_anim.update(dt_ms);

        // When expand animation completes, load editor content
        if self.expand_anim.progress() >= 0.99 && !self.editor.is_visible() {
            if let Some(id) = self.expanding_node_id {
                if let Some(node) = self.graph.find_node(id) {
                    if !node.file_path.is_empty() {
                        self.editor.load_file(&node.file_path);
                        self.editor.goto_line(node.line);
                        if node.is_error {
                            self.editor.set_highlight_line(node.line);
                        }
                        self.editor.set_visible(true);
                    }
                }
            }
        }

        // Hover interpolation
        for (i, node) in self.graph.nodes.iter().enumerate() {
            if i >= self.hover_values.len() {
                break;
            }
            let target = if self.hovered_node_id == Some(node.id) {
                1.0
            } else {
                0.0
            };
            self.hover_values[i] = smooth_towards(self.hover_values[i], target, dt_ms, 10.0);
        }
    }

    pub fn render(&self, renderer: &dyn Renderer) {
        renderer.push_transform(self.camera_offset, self.zoom);

        let is_expanding = self.expanding_node_id.is_some();
        let expand_t = if is_expanding {
            self.expand_anim.progress()
        } else {
            0.0
        };

        // Render edges (behind nodes)
        for edge in &self.graph.edges {
            let source = match self.graph.find_node(edge.source_id) {
                Some(n) => n,
                None => continue,
            };
            let target = match self.graph.find_node(edge.target_id) {
                Some(n) => n,
                None => continue,
            };

            let focus_opacity = if is_expanding {
                let expanding_id = self.expanding_node_id.unwrap();
                if edge.source_id == expanding_id || edge.target_id == expanding_id {
                    1.0
                } else {
                    0.15
                }
            } else {
                1.0
            };

            self.edge_renderer
                .render(renderer, edge, source, target, self.time_sec, focus_opacity);
        }

        // Render nodes
        for (i, node) in self.graph.nodes.iter().enumerate() {
            let hover_t = if i < self.hover_values.len() {
                self.hover_values[i]
            } else {
                0.0
            };

            let is_expanded_node = self.expanding_node_id == Some(node.id);

            if is_expanded_node && expand_t > 0.01 {
                // Render expanding/expanded node
                self.render_expanded_node(renderer, node, expand_t);
            } else {
                let focus_t = if is_expanding && !is_expanded_node {
                    expand_t
                } else {
                    0.0
                };

                self.node_renderer
                    .render_collapsed(renderer, node, hover_t, focus_t);
            }
        }

        renderer.pop_transform();
    }

    /// Render a node in its expanded state with editor view.
    fn render_expanded_node(&self, renderer: &dyn Renderer, node: &GraphNode, t: f64) {
        let (ox, oy, ow, oh) = self.expand_origin;
        let target_w = config::NODE_EXPANDED_W;
        let target_h = config::NODE_EXPANDED_H;
        // Center expanded node on its original position
        let target_x = ox + (ow - target_w) / 2.0;
        let target_y = oy + (oh - target_h) / 2.0;

        let x = lerp(ox, target_x, t);
        let y = lerp(oy, target_y, t);
        let w = lerp(ow, target_w, t);
        let h = lerp(oh, target_h, t);
        let r = config::NODE_CORNER_RADIUS;

        // Shadow
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
                a: 0.6 * t,
            },
            12.0 * t,
        );

        // Error glow
        if node.is_error {
            renderer.draw_shadow(x, y, w, h, r, Color::from_hex(config::ERROR_BORDER, 0.4 * t), 12.0);
        }

        // Background
        renderer.fill_rounded_rect(x, y, w, h, r, Color::from_hex(config::NODE_BG, 1.0));

        // Border
        let border_color = if node.is_error {
            Color::from_hex(config::ERROR_BORDER, 1.0)
        } else {
            Color::from_hex(config::EDGE_CALL, 0.8)
        };
        renderer.stroke_rounded_rect(x, y, w, h, r, border_color, 2.0);

        // Header
        let filename = node
            .file_path
            .rsplit('/')
            .next()
            .unwrap_or(&node.file_path);
        let header = format!("{}:{} — {}", filename, node.line, node.symbol_name);
        renderer.draw_text(
            x + 12.0,
            y + 8.0,
            &header,
            13.0,
            Color::from_hex(config::TEXT_PRIMARY, 1.0),
        );

        // Separator
        renderer.fill_rect(
            x + 8.0,
            y + 28.0,
            w - 16.0,
            1.0,
            Color::from_hex(config::NODE_BORDER, 0.6),
        );

        // Editor content (rendered when animation completes)
        if t > 0.95 {
            let editor_x = x + 4.0;
            let editor_y = y + 32.0;
            let editor_w = w - 8.0;
            let editor_h = h - 36.0;
            self.editor.render(renderer, editor_x, editor_y, editor_w, editor_h);
        }
    }

    // ===== Private helpers =====

    fn screen_to_graph(&self, screen: Vec2) -> Vec2 {
        Vec2 {
            x: (screen.x - self.camera_offset.x) / self.zoom,
            y: (screen.y - self.camera_offset.y) / self.zoom,
        }
    }

    fn expand_node(&mut self, id: u32) {
        // Store original position for animation
        if let Some(node) = self.graph.find_node(id) {
            self.expand_origin = (node.x, node.y, node.width, node.height);
        }
        self.expanding_node_id = Some(id);
        self.expand_anim.start(config::ANIM_EXPAND_MS);
        self.editor.set_visible(false);
    }

    fn collapse_node(&mut self) {
        self.editor.set_visible(false);
        self.expanding_node_id = None;
        self.collapse_anim.start(config::ANIM_EXPAND_MS);
        self.selected_node_id = None;
    }

    fn fit_all(&mut self) {
        if self.graph.nodes.is_empty() {
            return;
        }

        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;

        for node in &self.graph.nodes {
            min_x = min_x.min(node.x);
            min_y = min_y.min(node.y);
            max_x = max_x.max(node.x + node.width);
            max_y = max_y.max(node.y + node.height);
        }

        let graph_w = max_x - min_x + 100.0;
        let graph_h = max_y - min_y + 100.0;

        self.zoom = (self.view_width / graph_w)
            .min(self.view_height / graph_h)
            .min(config::ZOOM_MAX);
        self.camera_offset = Vec2 {
            x: self.view_width / 2.0 - (min_x + graph_w / 2.0) * self.zoom,
            y: self.view_height / 2.0 - (min_y + graph_h / 2.0) * self.zoom,
        };
    }

    fn focus_next_error(&mut self) {
        let error_nodes: Vec<u32> = self
            .graph
            .nodes
            .iter()
            .filter(|n| n.is_error)
            .map(|n| n.id)
            .collect();

        if error_nodes.is_empty() {
            return;
        }

        let current = self.selected_node_id.unwrap_or(u32::MAX);
        let next = error_nodes
            .iter()
            .find(|&&id| id > current)
            .or_else(|| error_nodes.first())
            .copied();

        if let Some(id) = next {
            self.selected_node_id = Some(id);
            if let Some(node) = self.graph.find_node(id) {
                self.camera_offset = Vec2 {
                    x: self.view_width / 2.0 - node.x * self.zoom,
                    y: self.view_height / 2.0 - node.y * self.zoom,
                };
            }
        }
    }

    /// Update the known view dimensions (called from main loop).
    pub fn set_view_size(&mut self, width: f64, height: f64) {
        self.view_width = width;
        self.view_height = height;
    }
}

impl Default for GraphView {
    fn default() -> Self {
        Self::new()
    }
}
