pub mod api;

use crate::error::{NovaError, Result};
use crate::plugin::api::{
    cstr_to_string, NovaPluginCtx, NovaPluginInfo, PluginInitFn, PLUGIN_INIT_SYMBOL,
};
use crate::state::SharedState;
use crate::widgets::WidgetDefinition;
use indexmap::IndexMap;
use libloading::Library;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, info, warn, error};

/// A loaded plugin
pub struct LoadedPlugin {
    /// The dynamic library handle — must be kept alive
    _lib: Library,

    /// Plugin metadata
    pub name: String,
    pub version: String,
    pub description: String,

    /// Widget names exported by this plugin
    pub widget_names: Vec<String>,
}

/// Plugin loader: scans a directory for .so files and loads them
pub struct PluginLoader {
    plugins: Vec<LoadedPlugin>,
    /// Widget definitions contributed by plugins
    contributed_defs: IndexMap<String, WidgetDefinition>,
}

impl PluginLoader {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            contributed_defs: IndexMap::new(),
        }
    }

    /// Scan `plugin_dir` for `.so` files and attempt to load each one.
    ///
    /// Errors loading individual plugins are logged as warnings, not propagated.
    pub fn scan_and_load(&mut self, plugin_dir: &Path) {
        if !plugin_dir.exists() {
            debug!("PluginLoader: plugin dir '{}' does not exist", plugin_dir.display());
            return;
        }

        let entries = match std::fs::read_dir(plugin_dir) {
            Ok(e) => e,
            Err(e) => {
                warn!("PluginLoader: cannot read plugin dir: {e}");
                return;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("so") {
                match self.load_plugin(&path) {
                    Ok(plugin) => {
                        info!(
                            "PluginLoader: loaded '{}' v{} ({} widgets)",
                            plugin.name,
                            plugin.version,
                            plugin.widget_names.len()
                        );
                        self.plugins.push(plugin);
                    }
                    Err(e) => {
                        warn!("PluginLoader: failed to load '{}': {e}", path.display());
                    }
                }
            }
        }
    }

    /// Load a single plugin .so file
    fn load_plugin(&mut self, path: &Path) -> Result<LoadedPlugin> {
        // SAFETY: loading a shared library is inherently unsafe.
        // We do our best to validate the returned pointers.
        let lib = unsafe { Library::new(path) }.map_err(|e| {
            NovaError::Plugin(format!("dlopen '{}': {e}", path.display()))
        })?;

        let info_ptr: *mut NovaPluginInfo = unsafe {
            let init_fn: libloading::Symbol<PluginInitFn> =
                lib.get(PLUGIN_INIT_SYMBOL).map_err(|e| {
                    NovaError::Plugin(format!("Symbol nova_plugin_init not found: {e}"))
                })?;

            init_fn()
        };

        if info_ptr.is_null() {
            return Err(NovaError::Plugin("nova_plugin_init returned null".to_string()));
        }

        let (name, version, description, widget_names) = unsafe {
            let info = &*info_ptr;

            let name = cstr_to_string(info.name)
                .unwrap_or_else(|| "unknown".to_string());
            let version = cstr_to_string(info.version)
                .unwrap_or_else(|| "0.0.0".to_string());
            let description = cstr_to_string(info.description)
                .unwrap_or_default();

            let mut widget_names = Vec::new();
            for i in 0..info.widget_count as usize {
                let name_ptr = *info.widget_names.add(i);
                if let Some(n) = cstr_to_string(name_ptr) {
                    widget_names.push(n);
                }
            }

            // Build WidgetDefinition stubs for each plugin widget
            // (plugin widgets use a special "plugin://" template marker)
            for wn in &widget_names {
                let def = WidgetDefinition {
                    name: wn.clone(),
                    description: format!("Plugin widget from {name}"),
                    template: format!("<box class=\"nova-plugin-{wn}\"/>"),
                    vars: IndexMap::new(),
                    default_style: String::new(),
                };
                self.contributed_defs.insert(wn.clone(), def);
            }

            (name, version, description, widget_names)
        };

        Ok(LoadedPlugin {
            _lib: lib,
            name,
            version,
            description,
            widget_names,
        })
    }

    /// Returns all widget definitions contributed by loaded plugins
    pub fn contributed_definitions(&self) -> &IndexMap<String, WidgetDefinition> {
        &self.contributed_defs
    }

    /// Returns the number of loaded plugins
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }

    /// Returns names of all loaded plugins
    pub fn plugin_names(&self) -> Vec<String> {
        self.plugins.iter().map(|p| p.name.clone()).collect()
    }
}

impl Default for PluginLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a `NovaPluginCtx` backed by the given `SharedState`.
///
/// The returned context has function pointers to engine callbacks.
/// The engine_ptr is a raw pointer to a `SharedState` clone on the heap.
pub fn make_plugin_ctx(state: SharedState) -> (*mut NovaPluginCtx, *mut c_void) {
    let state_box: Box<SharedState> = Box::new(state);
    let engine_ptr = Box::into_raw(state_box) as *mut c_void;

    let ctx = Box::new(NovaPluginCtx {
        engine_ptr,
        get_var: plugin_get_var,
        log_info: plugin_log_info,
        log_warn: plugin_log_warn,
        free_string: plugin_free_string,
    });

    let ctx_ptr = Box::into_raw(ctx);
    (ctx_ptr, engine_ptr)
}

/// Plugin callback: get variable value
unsafe extern "C" fn plugin_get_var(
    _engine: *mut c_void,
    var_name: *const std::ffi::c_char,
) -> *mut std::ffi::c_char {
    let name = match cstr_to_string(var_name) {
        Some(n) => n,
        None => return std::ptr::null_mut(),
    };

    let value = crate::widgets::eval_builtin(&name, &IndexMap::new());
    let cstring = match std::ffi::CString::new(value) {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    cstring.into_raw()
}

/// Plugin callback: log info
unsafe extern "C" fn plugin_log_info(
    _engine: *mut c_void,
    message: *const std::ffi::c_char,
) {
    if let Some(msg) = cstr_to_string(message) {
        info!("[plugin] {msg}");
    }
}

/// Plugin callback: log warn
unsafe extern "C" fn plugin_log_warn(
    _engine: *mut c_void,
    message: *const std::ffi::c_char,
) {
    if let Some(msg) = cstr_to_string(message) {
        warn!("[plugin] {msg}");
    }
}

/// Plugin callback: free a string returned by get_var
unsafe extern "C" fn plugin_free_string(ptr: *mut std::ffi::c_char) {
    if !ptr.is_null() {
        drop(std::ffi::CString::from_raw(ptr));
    }
}
