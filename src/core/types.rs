/// Common type definitions used across all phases.

/// Node type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeType {
    Function,
    Type,
    Variable,
    Include,
    ErrorSource,
}

/// Edge type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeType {
    Call,
    Reference,
    Include,
    Inherit,
    ErrorPath,
}

/// Parsed compiler error information
#[derive(Debug, Clone)]
pub struct ErrorInfo {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    pub error_code: String,
    pub message: String,
}

/// A node in the relay graph
#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: u32,
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    pub symbol_name: String,
    pub node_type: NodeType,
    pub is_error: bool,

    // Rendering state
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub expanded: bool,
}

impl GraphNode {
    pub fn new(id: u32, symbol_name: &str, node_type: NodeType) -> Self {
        Self {
            id,
            file_path: String::new(),
            line: 0,
            column: 0,
            symbol_name: symbol_name.to_string(),
            node_type,
            is_error: false,
            x: 0.0,
            y: 0.0,
            width: super::config::NODE_COLLAPSED_W,
            height: super::config::NODE_COLLAPSED_H,
            expanded: false,
        }
    }
}

/// An edge connecting two nodes
#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub source_id: u32,
    pub target_id: u32,
    pub edge_type: EdgeType,
    pub on_error_path: bool,
}

/// The complete relay graph
#[derive(Debug, Clone, Default)]
pub struct RelayGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

impl RelayGraph {
    pub fn find_node(&self, id: u32) -> Option<&GraphNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn find_node_mut(&mut self, id: u32) -> Option<&mut GraphNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }
}

/// 2D coordinate vector
#[derive(Debug, Clone, Copy, Default)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn length(&self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn normalized(&self) -> Self {
        let len = self.length();
        if len < 1e-10 {
            Self::default()
        } else {
            Self {
                x: self.x / len,
                y: self.y / len,
            }
        }
    }
}

impl std::ops::Add for Vec2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl std::ops::Sub for Vec2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl std::ops::Mul<f64> for Vec2 {
    type Output = Self;
    fn mul(self, s: f64) -> Self {
        Self {
            x: self.x * s,
            y: self.y * s,
        }
    }
}

impl std::ops::AddAssign for Vec2 {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

/// RGBA color
#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Color {
    pub fn from_hex(hex: u32, alpha: f64) -> Self {
        Self {
            r: ((hex >> 16) & 0xFF) as f64 / 255.0,
            g: ((hex >> 8) & 0xFF) as f64 / 255.0,
            b: (hex & 0xFF) as f64 / 255.0,
            a: alpha,
        }
    }

    pub fn with_alpha(self, alpha: f64) -> Self {
        Self { a: alpha, ..self }
    }
}

/// Mouse event data
#[derive(Debug, Clone, Default)]
pub struct MouseEvent {
    pub x: f64,
    pub y: f64,
    pub button: u8,
    pub scroll_y: f64,
    pub pressed: bool,
    pub released: bool,
    pub dragging: bool,
}

/// Key event data
#[derive(Debug, Clone, Default)]
pub struct KeyEvent {
    pub keycode: u32,
    pub pressed: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}
