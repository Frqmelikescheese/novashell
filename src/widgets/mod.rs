pub mod battery;
pub mod cava;
pub mod exec;
pub mod clock;
pub mod launcher;
pub mod media;
pub mod sysmon;
pub mod traits;
pub mod volume;

pub use traits::{NovaWidget, WidgetContext, WidgetEvent};

use crate::error::{NovaError, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tracing::debug;

/// Definition of a variable that a widget can expose
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VarDef {
    /// Shell command to run for this variable's value
    pub script: Option<String>,

    /// Name of a built-in data source (e.g. "clock::time")
    pub builtin: Option<String>,

    /// How often to refresh this variable, in milliseconds
    #[serde(default = "default_interval")]
    pub interval_ms: u64,

    /// Default value before first refresh
    #[serde(default)]
    pub default: String,
}

fn default_interval() -> u64 {
    1000
}

/// Full definition of a widget type (from a .widget file or built-in)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetDefinition {
    /// Canonical name used to reference the widget
    pub name: String,

    /// Human-readable description
    #[serde(default)]
    pub description: String,

    /// XML template string defining the GTK widget tree
    pub template: String,

    /// Variable definitions indexed by variable name
    #[serde(default)]
    pub vars: IndexMap<String, VarDef>,

    /// Inline CSS applied for this widget type
    #[serde(default)]
    pub default_style: String,
}

impl WidgetDefinition {
    /// Parse a .widget YAML file and return a `WidgetDefinition`
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|e| {
            NovaError::Widget(format!("Cannot read widget file {}: {e}", path.display()))
        })?;

        let def: WidgetDefinition = serde_yaml::from_str(&content).map_err(|e| {
            NovaError::Widget(format!(
                "YAML parse error in widget file {}: {e}",
                path.display()
            ))
        })?;

        debug!("Loaded widget definition '{}' from {}", def.name, path.display());
        Ok(def)
    }
}

/// Registry of built-in widget implementations (as `Arc<dyn NovaWidget>`)
pub struct BuiltinRegistry {
    widgets: IndexMap<String, Arc<dyn NovaWidget>>,
    definitions: IndexMap<String, WidgetDefinition>,
}

impl BuiltinRegistry {
    /// Create and register all built-in widgets
    pub fn new() -> Self {
        let mut reg = Self {
            widgets: IndexMap::new(),
            definitions: IndexMap::new(),
        };

        reg.register(Arc::new(clock::ClockWidget::new()));
        reg.register(Arc::new(sysmon::SysmonWidget::new()));
        reg.register(Arc::new(cava::CavaWidget::new()));
        reg.register(Arc::new(media::MediaWidget::new()));
        reg.register(Arc::new(battery::BatteryWidget::new()));
        reg.register(Arc::new(volume::VolumeWidget::new()));
        reg.register(Arc::new(launcher::LauncherWidget::new()));
        reg.register(Arc::new(exec::ExecWidget::new()));

        reg
    }

    fn register(&mut self, w: Arc<dyn NovaWidget>) {
        self.widgets.insert(w.name().to_string(), w);
    }

    /// Look up a widget implementation by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn NovaWidget>> {
        self.widgets.get(name).cloned()
    }

    /// Returns all registered widget names
    pub fn names(&self) -> Vec<String> {
        self.widgets.keys().cloned().collect()
    }

    /// Register a widget definition from a .widget file into this registry
    pub fn register_definition(&mut self, def: WidgetDefinition) {
        self.definitions.insert(def.name.clone(), def);
    }

    /// Get a widget definition by name
    pub fn get_definition(&self, name: &str) -> Option<&WidgetDefinition> {
        self.definitions.get(name)
    }
}

impl Default for BuiltinRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Evaluate a built-in data source by its path (e.g. "clock::time")
pub fn eval_builtin(source: &str, _vars: &IndexMap<String, String>) -> String {
    match source {
        "clock::time" => clock::get_time("%H:%M:%S"),
        "clock::date" => clock::get_date("%B %-d"),
        "clock::day" => clock::get_day("%A"),
        "sysmon::cpu_percent" => format!("{:.0}", sysmon::get_cpu_percent()),
        "sysmon::cpu_fraction" => format!("{:.3}", sysmon::get_cpu_percent() / 100.0),
        "sysmon::ram_used" => sysmon::get_ram_used(),
        "sysmon::ram_fraction" => format!("{:.3}", sysmon::get_ram_fraction()),
        "sysmon::net_rx" => sysmon::get_net_rx(),
        "sysmon::net_tx" => sysmon::get_net_tx(),
        "battery::percent" => battery::get_percent(),
        "battery::fraction" => format!("{:.3}", battery::get_fraction()),
        "battery::status" => battery::get_status(),
        "battery::icon" => battery::get_icon(),
        "volume::fraction" => format!("{:.3}", volume::get_fraction()),
        "volume::percent" => volume::get_percent(),
        "volume::icon" => volume::get_icon(),
        "media::title" => media::get_title(),
        "media::artist" => media::get_artist(),
        "media::play_icon" => media::get_play_icon(),
        "media::position_fraction" => format!("{:.3}", media::get_position_fraction()),
        "media::art_path" => media::get_art_path(),
        // cava::bars is handled specially by the cava widget
        "cava::bars" => String::new(),
        _ => String::new(),
    }
}

/// Evaluate a shell script command, capturing stdout
pub fn eval_script(script: &str) -> String {
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(script)
        .output();

    match output {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            tracing::warn!("Script '{script}' failed: {stderr}");
            String::new()
        }
        Err(e) => {
            tracing::warn!("Failed to run script '{script}': {e}");
            String::new()
        }
    }
}
