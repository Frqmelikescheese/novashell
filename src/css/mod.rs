use crate::error::{NovaError, Result};
use gtk4::prelude::*;
use gtk4::CssProvider;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// CSS manager: loads style.css from the config directory and applies it to GTK.
///
/// Supports `@import` statements by recursively expanding referenced files
/// before handing the CSS to GTK's CssProvider.
pub struct CssManager {
    /// Path to the primary style.css
    style_path: PathBuf,

    /// The GTK CssProvider we manage
    provider: CssProvider,

    /// Priority at which the provider is installed
    priority: u32,
}

impl CssManager {
    /// Create a new CssManager for the given config directory.
    ///
    /// The CSS will be loaded from `<config_dir>/style.css`.
    pub fn new(config_dir: &Path) -> Self {
        let style_path = config_dir.join("style.css");
        let provider = CssProvider::new();

        Self {
            style_path,
            provider,
            priority: gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        }
    }

    /// Load the CSS file and apply it to the default GTK display.
    ///
    /// Returns an error if the file exists but cannot be read or parsed.
    /// If the file does not exist, a warning is logged and no CSS is applied.
    pub fn load(&self) -> Result<()> {
        let css = self.read_css_with_imports(&self.style_path)?;
        self.apply_css(&css);
        info!("CSS: loaded '{}'", self.style_path.display());
        Ok(())
    }

    /// Reload the CSS file and update the GTK provider.
    ///
    /// This replaces the existing provider data without removing the provider
    /// from the display, so the change is seamless.
    pub fn reload(&self) -> Result<()> {
        let css = self.read_css_with_imports(&self.style_path)?;
        self.apply_css(&css);
        info!("CSS: reloaded '{}'", self.style_path.display());
        Ok(())
    }

    /// Load an additional inline CSS string (e.g. widget default styles)
    pub fn load_inline(&self, css: &str) {
        let combined = format!("{}\n{}", self.current_css(), css);
        self.apply_css(&combined);
    }

    /// Apply a CSS string to the default GTK display
    fn apply_css(&self, css: &str) {
        self.provider.load_from_data(css);

        if let Some(display) = gdk4::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &self.provider,
                self.priority,
            );
        } else {
            warn!("CSS: no default GDK display available");
        }
    }

    /// Read CSS from a file, recursively expanding `@import` statements.
    ///
    /// Supports:
    /// - `@import "path.css";`
    /// - `@import 'path.css';`
    ///
    /// Paths are resolved relative to the importing file's directory.
    /// Circular imports are detected and skipped after a depth limit.
    fn read_css_with_imports(&self, path: &Path) -> Result<String> {
        self.read_css_recursive(path, 0)
    }

    fn read_css_recursive(&self, path: &Path, depth: usize) -> Result<String> {
        if depth > 16 {
            warn!("CSS: @import depth limit reached at '{}'", path.display());
            return Ok(String::new());
        }

        if !path.exists() {
            debug!("CSS: file '{}' does not exist, skipping", path.display());
            return Ok(String::new());
        }

        let content = std::fs::read_to_string(path).map_err(|e| {
            NovaError::Css(format!("Cannot read CSS file {}: {e}", path.display()))
        })?;

        let base_dir = path.parent().unwrap_or(Path::new("."));
        let mut result = String::with_capacity(content.len());

        for line in content.lines() {
            let trimmed = line.trim();

            // Handle @import
            if trimmed.starts_with("@import") {
                // Extract the quoted path
                if let Some(import_path) = extract_import_path(trimmed) {
                    let import_full = if import_path.starts_with('/') {
                        PathBuf::from(&import_path)
                    } else {
                        base_dir.join(&import_path)
                    };

                    debug!("CSS: expanding @import '{}'", import_full.display());
                    let imported = self.read_css_recursive(&import_full, depth + 1)?;
                    result.push_str(&imported);
                    result.push('\n');
                    continue;
                }
            }

            result.push_str(line);
            result.push('\n');
        }

        Ok(result)
    }

    /// Returns the current CSS string loaded in the provider (empty if none)
    fn current_css(&self) -> String {
        // CssProvider doesn't expose a getter, so we track manually if needed.
        // For simplicity, re-read the file.
        self.read_css_recursive(&self.style_path, 0)
            .unwrap_or_default()
    }

    /// Apply per-widget default styles to the display
    pub fn apply_widget_defaults(&self, widget_name: &str, css: &str) {
        if css.is_empty() {
            return;
        }
        debug!("CSS: applying default styles for widget '{widget_name}'");
        let provider = CssProvider::new();
        provider.load_from_data(css);

        if let Some(display) = gdk4::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION - 1,
            );
        }
    }
}

/// Extract the import path from an `@import` line.
///
/// Handles: `@import "foo.css";` and `@import 'foo.css';`
fn extract_import_path(line: &str) -> Option<String> {
    let rest = line.trim_start_matches("@import").trim();
    let rest = rest.trim_end_matches(';').trim();

    // Strip surrounding quotes
    if (rest.starts_with('"') && rest.ends_with('"'))
        || (rest.starts_with('\'') && rest.ends_with('\''))
    {
        Some(rest[1..rest.len() - 1].to_string())
    } else {
        // Try url() form: @import url("foo.css");
        if rest.starts_with("url(") && rest.ends_with(')') {
            let inner = &rest[4..rest.len() - 1].trim();
            if (inner.starts_with('"') && inner.ends_with('"'))
                || (inner.starts_with('\'') && inner.ends_with('\''))
            {
                return Some(inner[1..inner.len() - 1].to_string());
            }
            return Some(inner.to_string());
        }
        None
    }
}
