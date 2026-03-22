pub mod layer;
pub mod widget_factory;
pub mod window;

use crate::config::schema::{ScreenConfig, WidgetInstance};
use crate::error::{NovaError, Result};
use crate::renderer::layer::{configure_layer_for_anchor, find_monitor_by_name, map_layer_type_pub};
use crate::renderer::widget_factory::WidgetFactory;
use crate::renderer::window::NovaWindow;
use crate::state::SharedState;
use crate::widgets::{BuiltinRegistry, traits::WidgetContext};
use gdk4::prelude::*;
use gtk4::prelude::*;
use gtk4::Application;
use gtk4_layer_shell::LayerShell;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// The main renderer: manages GTK Application and all layer shell windows.
pub struct Renderer {
    app: Application,
    windows: HashMap<String, Vec<NovaWindow>>,
    builtin_registry: Arc<BuiltinRegistry>,
    state: SharedState,
}

impl Renderer {
    /// Create a new Renderer for the given GTK Application
    pub fn new(app: Application, state: SharedState) -> Self {
        Self {
            app,
            windows: HashMap::new(),
            builtin_registry: Arc::new(BuiltinRegistry::new()),
            state,
        }
    }

    /// Render all screens from the current config.
    ///
    /// Creates one `ApplicationWindow` per widget instance (positioned via
    /// layer shell anchors) and populates it with the widget tree.
    pub fn render_all(&mut self) {
        let config = {
            let state = self.state.read();
            state.config.clone()
        };

        for (screen_name, screen_config) in &config.screens {
            if !screen_config.enabled {
                debug!("Renderer: screen '{screen_name}' disabled, skipping");
                continue;
            }

            info!("Renderer: rendering screen '{screen_name}'");
            self.render_screen(screen_name, screen_config);
        }
    }

    /// Render a single screen, creating windows for each widget instance
    fn render_screen(&mut self, screen_name: &str, screen_config: &ScreenConfig) {
        let mut screen_windows = Vec::new();

        for (idx, widget_instance) in screen_config.widgets.iter().enumerate() {
            let instance_id = widget_instance
                .id
                .clone()
                .unwrap_or_else(|| format!("{screen_name}-widget-{idx}"));

            if !widget_instance.visible {
                debug!("Renderer: widget '{instance_id}' hidden, skipping");
                continue;
            }

            match self.build_widget_window(screen_name, screen_config, widget_instance, &instance_id) {
                Ok(nova_win) => {
                    nova_win.show();
                    screen_windows.push(nova_win);
                }
                Err(e) => {
                    error!(
                        "Renderer: failed to build widget '{}' on screen '{}': {e}",
                        widget_instance.widget, screen_name
                    );
                }
            }
        }

        self.windows.insert(screen_name.to_string(), screen_windows);
    }

    /// Build a dedicated layer shell window for a single widget instance
    fn build_widget_window(
        &self,
        screen_name: &str,
        screen_config: &ScreenConfig,
        instance: &WidgetInstance,
        instance_id: &str,
    ) -> Result<NovaWindow> {
        // Create the layer shell window
        let nova_win = {
            let win = gtk4::ApplicationWindow::new(&self.app);
            win.set_decorated(false);
            win.set_resizable(false);
            win.init_layer_shell();

            // Set layer
            win.set_layer(map_layer_type_pub(&screen_config.layer));

            // Set namespace so Hyprland layerrules can target us
            win.set_namespace(Some("novashell"));

            // Configure anchor and margins
            configure_layer_for_anchor(
                &win,
                instance.position.anchor,
                instance.position.margin,
            );

            win.set_exclusive_zone(0);

            // Pin to correct physical monitor by connector name
            debug!("Renderer: pinning to monitor '{}'", screen_config.monitor);
            if let Some(monitor) = find_monitor_by_name(&screen_config.monitor) {
                win.set_monitor(Some(&monitor));
                debug!("Renderer: monitor '{}' pinned OK", screen_config.monitor);
            } else {
                warn!("Renderer: monitor '{}' not found, using compositor default", screen_config.monitor);
            }

            // Set explicit size if requested
            if let (Some(w), Some(h)) = (instance.position.width, instance.position.height) {
                win.set_default_size(w, h);
            }

            NovaWindow::new(win, instance_id)
        };

        // Build the GTK widget tree.
        // Priority: built-in Rust implementation → XML template from file/registry
        let gtk_widget = if let Some(builtin) = self.builtin_registry.get(&instance.widget) {
            // Use the NovaWidget Rust implementation (proper timers, no main-thread blocking)
            let mut ctx = WidgetContext::new(instance_id, &instance.widget);
            if let Some(vars) = &instance.vars {
                for (k, v) in vars {
                    ctx.set_var(k, v);
                }
            }
            debug!("Renderer: using builtin impl for '{}' on '{screen_name}'", instance.widget);
            builtin.build(&ctx)
        } else {
            // Fall back to XML template (custom .widget files)
            let widget_def = self.resolve_widget_definition(&instance.widget)?;
            WidgetFactory::build(&widget_def, instance)?
        };

        // Apply per-instance style override (inline CSS)
        if let Some(css) = &instance.style_override {
            let provider = gtk4::CssProvider::new();
            provider.load_from_data(css.as_str());
            gtk4::style_context_add_provider_for_display(
                &gdk4::Display::default().expect("no display"),
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
            );
        }

        // Apply per-instance opacity
        if let Some(opacity) = instance.opacity {
            gtk_widget.set_opacity(opacity);
        }

        // Apply tooltip
        if let Some(tooltip) = &instance.tooltip {
            gtk_widget.set_tooltip_text(Some(tooltip.as_str()));
            gtk_widget.set_has_tooltip(true);
        }

        // Apply click handlers via GestureClick
        let has_click = instance.on_click.is_some()
            || instance.on_right_click.is_some()
            || instance.on_middle_click.is_some();
        if has_click {
            let gesture = gtk4::GestureClick::new();
            gesture.set_button(0); // listen to all buttons
            let on_click = instance.on_click.clone();
            let on_right = instance.on_right_click.clone();
            let on_middle = instance.on_middle_click.clone();
            gesture.connect_released(move |g, _, _, _| {
                let btn = g.current_button();
                let cmd = match btn {
                    1 => on_click.as_deref(),
                    2 => on_middle.as_deref(),
                    3 => on_right.as_deref(),
                    _ => None,
                };
                if let Some(cmd) = cmd {
                    std::process::Command::new("sh").arg("-c").arg(cmd).spawn().ok();
                }
            });
            gtk_widget.add_controller(gesture);
        }

        nova_win.add_widget(&gtk_widget);

        debug!("Renderer: built widget '{instance_id}' ({}) on '{screen_name}'", instance.widget);
        Ok(nova_win)
    }

    /// Resolve a widget name to its WidgetDefinition (builtin or from file)
    fn resolve_widget_definition(
        &self,
        widget_ref: &str,
    ) -> Result<crate::widgets::WidgetDefinition> {
        // Check if it's a file path
        if widget_ref.contains('/') || widget_ref.ends_with(".widget") {
            let path = if widget_ref.starts_with("~/") {
                crate::config::expand_tilde(widget_ref)
            } else {
                std::path::PathBuf::from(widget_ref)
            };
            return crate::widgets::WidgetDefinition::load_from_file(&path);
        }

        // Check state registry
        {
            let state = self.state.read();
            if let Some(def) = state.widget_registry.get(widget_ref) {
                return Ok(def.clone());
            }
        }

        // Try built-in registry's definitions
        if let Some(def) = self.builtin_registry.get_definition(widget_ref) {
            return Ok(def.clone());
        }

        // Try to load from the widgets/ directory
        let candidates = vec![
            format!("widgets/{widget_ref}.widget"),
            format!("/usr/share/novashell/widgets/{widget_ref}.widget"),
        ];

        if let Some(home) = dirs::config_dir() {
            let path = home.join("novashell").join("widgets").join(format!("{widget_ref}.widget"));
            if path.exists() {
                return crate::widgets::WidgetDefinition::load_from_file(&path);
            }
        }

        for candidate in &candidates {
            let path = std::path::Path::new(candidate);
            if path.exists() {
                return crate::widgets::WidgetDefinition::load_from_file(path);
            }
        }

        Err(NovaError::Widget(format!(
            "Widget definition '{}' not found (not a path, not builtin, no .widget file)",
            widget_ref
        )))
    }

    /// Show a widget or screen by ID
    pub fn show_target(&mut self, target: &str) {
        self.set_target_visibility(target, true);
    }

    /// Hide a widget or screen by ID
    pub fn hide_target(&mut self, target: &str) {
        self.set_target_visibility(target, false);
    }

    /// Toggle visibility of a widget or screen by ID
    pub fn toggle_target(&mut self, target: &str) {
        // Check current visibility and flip
        for windows in self.windows.values() {
            for win in windows {
                if win.screen_name == target {
                    if win.window.is_visible() {
                        win.hide();
                    } else {
                        win.show();
                    }
                    return;
                }
            }
        }
        warn!("Renderer: toggle target '{target}' not found");
    }

    fn set_target_visibility(&mut self, target: &str, visible: bool) {
        let mut found = false;
        for windows in self.windows.values() {
            for win in windows {
                if win.screen_name == target {
                    if visible {
                        win.show();
                    } else {
                        win.hide();
                    }
                    found = true;
                }
            }
        }
        if !found {
            warn!("Renderer: target '{target}' not found");
        }
    }

    /// Rebuild all windows from the current config (hot-reload)
    pub fn reload(&mut self) {
        info!("Renderer: reloading all screens");
        for (_, windows) in self.windows.drain() {
            for win in windows {
                win.window.close();
            }
        }
        self.render_all();
    }
}

