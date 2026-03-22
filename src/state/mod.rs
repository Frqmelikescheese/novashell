use crate::config::NovaConfig;
use crate::widgets::WidgetDefinition;
use indexmap::IndexMap;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Unique identifier for a rendered screen
pub type ScreenId = u32;

/// Registry mapping widget names to their definitions
#[derive(Debug, Default)]
pub struct WidgetRegistry {
    inner: IndexMap<String, WidgetDefinition>,
}

impl WidgetRegistry {
    pub fn new() -> Self {
        Self {
            inner: IndexMap::new(),
        }
    }

    /// Register a widget definition under its name
    pub fn register(&mut self, def: WidgetDefinition) {
        self.inner.insert(def.name.clone(), def);
    }

    /// Look up a widget definition by name
    pub fn get(&self, name: &str) -> Option<&WidgetDefinition> {
        self.inner.get(name)
    }

    /// Returns all registered widget names
    pub fn names(&self) -> Vec<String> {
        self.inner.keys().cloned().collect()
    }

    /// Returns the number of registered widgets
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns true if no widgets are registered
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// Global application state, shared via `Arc<RwLock<AppState>>`
#[derive(Debug)]
pub struct AppState {
    /// Path to the config file (used for reload)
    pub config_path: PathBuf,

    /// Currently active configuration
    pub config: NovaConfig,

    /// Whether the CSS provider has been loaded
    pub css_loaded: bool,

    /// Map from screen name to its rendered screen ID
    pub active_screens: HashMap<String, ScreenId>,

    /// Registry of available widget definitions
    pub widget_registry: WidgetRegistry,

    /// Available profile names
    pub profiles: Vec<String>,

    /// Name of the currently active profile
    pub active_profile: String,

    /// Whether the daemon is running
    pub running: bool,

    /// Visibility state of individual widget instances, keyed by widget ID
    pub widget_visibility: HashMap<String, bool>,
}

impl AppState {
    pub fn new(config: NovaConfig, config_path: PathBuf) -> Self {
        Self {
            config_path,
            config,
            css_loaded: false,
            active_screens: HashMap::new(),
            widget_registry: WidgetRegistry::new(),
            profiles: vec!["default".to_string()],
            active_profile: "default".to_string(),
            running: true,
            widget_visibility: HashMap::new(),
        }
    }

    /// Returns true if the widget with the given ID is visible
    pub fn is_visible(&self, widget_id: &str) -> bool {
        *self.widget_visibility.get(widget_id).unwrap_or(&true)
    }

    /// Set the visibility of a widget by ID
    pub fn set_visible(&mut self, widget_id: &str, visible: bool) {
        self.widget_visibility.insert(widget_id.to_string(), visible);
    }

    /// Toggle the visibility of a widget by ID
    pub fn toggle_visible(&mut self, widget_id: &str) {
        let current = self.is_visible(widget_id);
        self.set_visible(widget_id, !current);
    }
}

/// Thread-safe handle to the global app state
pub type SharedState = Arc<RwLock<AppState>>;

/// Create a new shared state handle
pub fn new_shared(config: NovaConfig, config_path: PathBuf) -> SharedState {
    Arc::new(RwLock::new(AppState::new(config, config_path)))
}
