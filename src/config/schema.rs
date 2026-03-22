use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top-level NovaShell configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NovaConfig {
    /// Global display settings
    #[serde(default)]
    pub novashell: GlobalSettings,

    /// Per-screen widget configurations, keyed by screen name
    #[serde(default)]
    pub screens: IndexMap<String, ScreenConfig>,

    /// Global widget defaults
    #[serde(default)]
    pub defaults: WidgetDefaults,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalSettings {
    /// Logging level: error, warn, info, debug, trace
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Base config directory (tilde-expanded)
    #[serde(default = "default_config_dir")]
    pub config_dir: String,

    /// Whether to enable hot-reload of config/CSS on file change
    #[serde(default = "default_true")]
    pub hot_reload: bool,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_config_dir() -> String {
    "~/.config/novashell".to_string()
}

fn default_true() -> bool {
    true
}

/// Configuration for a single screen / monitor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenConfig {
    /// Monitor connector name (e.g. "DP-1", "HDMI-A-1", "eDP-1")
    #[serde(default = "default_monitor")]
    pub monitor: String,

    /// Layer to render on
    #[serde(default)]
    pub layer: LayerType,

    /// Exclusive zone: -1 = ignore existing, 0 = no exclusion, >0 = reserve N pixels
    #[serde(default)]
    pub exclusive_zone: i32,

    /// Whether this screen config is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Widgets to render on this screen
    #[serde(default)]
    pub widgets: Vec<WidgetInstance>,
}

fn default_monitor() -> String {
    "DP-1".to_string()
}

impl Default for ScreenConfig {
    fn default() -> Self {
        Self {
            monitor: default_monitor(),
            layer: LayerType::Top,
            exclusive_zone: 0,
            enabled: true,
            widgets: Vec::new(),
        }
    }
}

/// A widget instance placed on a screen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetInstance {
    /// Widget name (builtin) or path to .widget file
    pub widget: String,

    /// Unique ID for this instance (used by IPC commands)
    pub id: Option<String>,

    /// Position and sizing configuration
    #[serde(default)]
    pub position: PositionConfig,

    /// Per-instance variable overrides
    pub vars: Option<HashMap<String, String>>,

    /// Inline CSS to apply on top of widget defaults (scoped to display)
    pub style_override: Option<String>,

    /// Widget opacity 0.0 (transparent) – 1.0 (opaque). Default: 1.0
    pub opacity: Option<f64>,

    /// Whether this widget starts visible
    #[serde(default = "default_true")]
    pub visible: bool,

    /// Shell command to run when the widget is left-clicked
    pub on_click: Option<String>,

    /// Shell command to run on right-click
    pub on_right_click: Option<String>,

    /// Shell command to run on middle-click
    pub on_middle_click: Option<String>,

    /// Tooltip text shown on hover
    pub tooltip: Option<String>,
}

impl Default for WidgetInstance {
    fn default() -> Self {
        Self {
            widget: String::new(),
            id: None,
            position: PositionConfig::default(),
            vars: None,
            style_override: None,
            opacity: None,
            visible: true,
            on_click: None,
            on_right_click: None,
            on_middle_click: None,
            tooltip: None,
        }
    }
}

/// Position and sizing for a widget window
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionConfig {
    /// Anchor point on screen
    #[serde(default)]
    pub anchor: AnchorPoint,

    /// X offset from the anchor point in pixels
    #[serde(default)]
    pub x: i32,

    /// Y offset from the anchor point in pixels
    #[serde(default)]
    pub y: i32,

    /// Margins: [top, right, bottom, left] in pixels
    #[serde(default)]
    pub margin: [i32; 4],

    /// Fixed width in pixels (None = auto-size)
    pub width: Option<i32>,

    /// Fixed height in pixels (None = auto-size)
    pub height: Option<i32>,
}

impl Default for PositionConfig {
    fn default() -> Self {
        Self {
            anchor: AnchorPoint::TopLeft,
            x: 0,
            y: 0,
            margin: [0, 0, 0, 0],
            width: None,
            height: None,
        }
    }
}

/// Screen anchor point for positioning widgets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AnchorPoint {
    #[default]
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl AnchorPoint {
    /// Returns whether this anchor includes the top edge
    pub fn has_top(&self) -> bool {
        matches!(
            self,
            AnchorPoint::TopLeft | AnchorPoint::Top | AnchorPoint::TopRight
        )
    }

    /// Returns whether this anchor includes the bottom edge
    pub fn has_bottom(&self) -> bool {
        matches!(
            self,
            AnchorPoint::BottomLeft | AnchorPoint::Bottom | AnchorPoint::BottomRight
        )
    }

    /// Returns whether this anchor includes the left edge
    pub fn has_left(&self) -> bool {
        matches!(
            self,
            AnchorPoint::TopLeft | AnchorPoint::Left | AnchorPoint::BottomLeft
        )
    }

    /// Returns whether this anchor includes the right edge
    pub fn has_right(&self) -> bool {
        matches!(
            self,
            AnchorPoint::TopRight | AnchorPoint::Right | AnchorPoint::BottomRight
        )
    }
}

/// Wayland/X11 layer to render on
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LayerType {
    /// Behind all windows, at wallpaper level
    Background,
    /// Below normal windows
    Bottom,
    /// Above normal windows (default)
    #[default]
    Top,
    /// Above everything including full-screen windows
    Overlay,
}

/// Global defaults applied to all widgets
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WidgetDefaults {
    /// CSS class prefix for widget elements (default: "nova")
    #[serde(default = "default_class_prefix")]
    pub class_prefix: String,

    /// Default font family
    #[serde(default = "default_font")]
    pub font: String,

    /// Default font size in points
    #[serde(default = "default_font_size")]
    pub font_size: u32,

    /// Background color (CSS color string)
    pub background: Option<String>,

    /// Text color (CSS color string)
    pub foreground: Option<String>,

    /// Corner radius in pixels
    #[serde(default)]
    pub border_radius: u32,

    /// Padding in pixels
    #[serde(default = "default_padding")]
    pub padding: u32,
}

fn default_class_prefix() -> String {
    "nova".to_string()
}

fn default_font() -> String {
    "Sans".to_string()
}

fn default_font_size() -> u32 {
    12
}

fn default_padding() -> u32 {
    8
}
