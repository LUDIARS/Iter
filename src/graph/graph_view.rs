/// Main graph view: camera management, node interaction, and rendering coordination.

use crate::core::config;
use crate::core::types::*;
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
    hover_values: Vec<f64>, // per-node hover animation value (0..1)

    // Expand/collapse animation
    expand_anim: Animation,
    collapse_anim: Animation,
    expanding_node_id: Option<u32>,

    // Renderers
    node_renderer: GraphNodeRenderer,
    edge_renderer: GraphEdgeRenderer,

    // Timing
    time_sec: f64,
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
            node_renderer: GraphNodeRenderer::new(),
            edge_renderer: GraphEdgeRenderer::new(),
            time_sec: 0.0,
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

            // Zoom centered on cursor
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
                    // Already selected, toggle expand
                    if self.expanding_node_id.is_some() {
                        self.collapse_node();
                    }
                } else {
                    // Select and expand
                    if self.expanding_node_id.is_some() {
                        self.collapse_node();
                    }
                    self.selected_node_id = Some(hovered);
                    self.expand_node(hovered);
                }
            } else {
                // Click on empty space
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

        // ESC: collapse or quit signal
        if e.keycode == 9 {
            if self.expanding_node_id.is_some() {
                self.collapse_node();
            }
        }

        // Space: fit all
        if e.keycode == 65 {
            self.fit_all();
        }

        // Tab: next error node
        if e.keycode == 23 {
            self.focus_next_error();
        }

        // Ctrl+Q: quit (handled externally)
    }

    pub fn update(&mut self, dt_ms: f64) {
        self.time_sec += dt_ms / 1000.0;

        // Update expand/collapse animations
        self.expand_anim.update(dt_ms);
        self.collapse_anim.update(dt_ms);

        // Update hover interpolation
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
        // Apply camera transform
        renderer.push_transform(self.camera_offset, self.zoom);

        let is_expanding = self.expanding_node_id.is_some();

        // Render edges first (behind nodes)
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

            let focus_t = if is_expanding {
                let expanding_id = self.expanding_node_id.unwrap();
                if node.id == expanding_id {
                    0.0
                } else {
                    self.expand_anim.progress()
                }
            } else {
                0.0
            };

            self.node_renderer
                .render_collapsed(renderer, node, hover_t, focus_t);
        }

        renderer.pop_transform();
    }

    // ===== Private helpers =====

    fn screen_to_graph(&self, screen: Vec2) -> Vec2 {
        Vec2 {
            x: (screen.x - self.camera_offset.x) / self.zoom,
            y: (screen.y - self.camera_offset.y) / self.zoom,
        }
    }

    fn _graph_to_screen(&self, graph: Vec2) -> Vec2 {
        Vec2 {
            x: graph.x * self.zoom + self.camera_offset.x,
            y: graph.y * self.zoom + self.camera_offset.y,
        }
    }

    fn expand_node(&mut self, id: u32) {
        self.expanding_node_id = Some(id);
        self.expand_anim.start(config::ANIM_EXPAND_MS);
    }

    fn collapse_node(&mut self) {
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

        // This would need window dimensions; approximate with 1280x720
        let view_w = 1280.0;
        let view_h = 720.0;

        self.zoom = (view_w / graph_w).min(view_h / graph_h).min(config::ZOOM_MAX);
        self.camera_offset = Vec2 {
            x: view_w / 2.0 - (min_x + graph_w / 2.0) * self.zoom,
            y: view_h / 2.0 - (min_y + graph_h / 2.0) * self.zoom,
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
                    x: 640.0 - node.x * self.zoom,
                    y: 360.0 - node.y * self.zoom,
                };
            }
        }
    }
}

impl Default for GraphView {
    fn default() -> Self {
        Self::new()
    }
}
