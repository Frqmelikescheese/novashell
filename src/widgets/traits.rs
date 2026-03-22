use indexmap::IndexMap;
use std::collections::HashMap;

/// Context passed to widgets during build and update operations
#[derive(Debug, Clone)]
pub struct WidgetContext {
    /// Current variable values, including builtin and user-defined
    pub vars: HashMap<String, String>,

    /// Widget instance ID
    pub id: String,

    /// Widget definition name
    pub widget_name: String,

    /// Whether the widget is currently visible
    pub visible: bool,
}

impl WidgetContext {
    pub fn new(id: impl Into<String>, widget_name: impl Into<String>) -> Self {
        Self {
            vars: HashMap::new(),
            id: id.into(),
            widget_name: widget_name.into(),
            visible: true,
        }
    }

    /// Get a variable value, returning the fallback if not found
    pub fn var(&self, name: &str, fallback: &str) -> String {
        self.vars
            .get(name)
            .cloned()
            .unwrap_or_else(|| fallback.to_string())
    }

    /// Set a variable value
    pub fn set_var(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.vars.insert(name.into(), value.into());
    }
}

/// Events that can be dispatched to widgets
#[derive(Debug, Clone)]
pub enum WidgetEvent {
    /// A button with a named action was clicked
    ButtonClick { action: String },
    /// A slider value changed
    SliderChange { action: String, value: f64 },
    /// A variable was updated (builtin refresh)
    VarUpdate { name: String, value: String },
    /// Widget was shown
    Show,
    /// Widget was hidden
    Hide,
}

/// Trait that all NovaShell widget implementations must satisfy.
///
/// Implementations are Send + Sync so they can be shared across threads.
pub trait NovaWidget: Send + Sync {
    /// Returns the canonical name of this widget type
    fn name(&self) -> &str;

    /// Build the GTK widget tree for this widget instance.
    /// Called once when the widget is first rendered.
    fn build(&self, ctx: &WidgetContext) -> gtk4::Widget;

    /// Update an already-built widget with new context data.
    /// Called periodically or when variables change.
    fn update(&self, widget: &gtk4::Widget, ctx: &WidgetContext);

    /// Handle a widget event (button click, slider change, etc.)
    fn on_event(&self, event: &WidgetEvent, widget: &gtk4::Widget);
}

/// A map of variable definitions indexed by variable name
pub type VarMap = IndexMap<String, super::VarDef>;
