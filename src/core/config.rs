/// Visual and behavioral constants.

// --- Visual colors (hex) ---
pub const BG_COLOR: u32 = 0x0D1117;
pub const NODE_BG: u32 = 0x161B22;
pub const NODE_BORDER: u32 = 0x30363D;
pub const ERROR_BORDER: u32 = 0xE94560;
pub const EDGE_CALL: u32 = 0x4A90D9;
pub const EDGE_INCLUDE: u32 = 0x2ECC71;
pub const EDGE_INHERIT: u32 = 0x8E8EA0;
pub const EDGE_ERROR: u32 = 0xE94560;
pub const TEXT_PRIMARY: u32 = 0xE6EDF3;
pub const TEXT_SECONDARY: u32 = 0x8B949E;

// --- Animation ---
pub const ANIM_EXPAND_MS: f64 = 200.0;
pub const ANIM_EASE_OUT: f64 = 0.25;
pub const HOVER_SHADOW_GROW: f64 = 4.0;

// --- Node sizes ---
pub const NODE_COLLAPSED_W: f64 = 180.0;
pub const NODE_COLLAPSED_H: f64 = 100.0;
pub const NODE_EXPANDED_W: f64 = 640.0;
pub const NODE_EXPANDED_H: f64 = 420.0;
pub const NODE_CORNER_RADIUS: f64 = 8.0;

// --- Zoom ---
pub const ZOOM_MIN: f64 = 0.1;
pub const ZOOM_MAX: f64 = 5.0;
pub const ZOOM_SPEED: f64 = 0.1;

// --- Graph layout ---
pub const LAYOUT_NODE_GAP_X: f64 = 60.0;
pub const LAYOUT_NODE_GAP_Y: f64 = 40.0;
