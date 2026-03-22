use std::ffi::{c_char, c_void};

/// Information struct returned by plugin init. C-compatible layout.
#[repr(C)]
pub struct NovaPluginInfo {
    /// Null-terminated plugin name
    pub name: *const c_char,

    /// Null-terminated version string (e.g. "1.0.0")
    pub version: *const c_char,

    /// Null-terminated description
    pub description: *const c_char,

    /// Number of widget types this plugin provides
    pub widget_count: u32,

    /// Array of null-terminated widget name strings (length: widget_count)
    pub widget_names: *const *const c_char,
}

// SAFETY: NovaPluginInfo contains raw pointers into static string data
// from the plugin's own binary. We must not access them after the plugin is unloaded.
unsafe impl Send for NovaPluginInfo {}
unsafe impl Sync for NovaPluginInfo {}

/// Context passed to plugin widget build functions
#[repr(C)]
pub struct NovaPluginCtx {
    /// Opaque pointer to the engine's internal context
    pub engine_ptr: *mut c_void,

    /// Callback to get the current value of a named variable
    /// Returns a newly-allocated C string the caller must free with nova_free_string
    pub get_var: unsafe extern "C" fn(
        engine: *mut c_void,
        var_name: *const c_char,
    ) -> *mut c_char,

    /// Callback to log a message at INFO level
    pub log_info: unsafe extern "C" fn(engine: *mut c_void, message: *const c_char),

    /// Callback to log a message at WARN level
    pub log_warn: unsafe extern "C" fn(engine: *mut c_void, message: *const c_char),

    /// Free a string returned by a callback (must be called for all get_var results)
    pub free_string: unsafe extern "C" fn(ptr: *mut c_char),
}

// SAFETY: The plugin ctx contains function pointers that are thread-safe
unsafe impl Send for NovaPluginCtx {}
unsafe impl Sync for NovaPluginCtx {}

/// Type of the `nova_plugin_init` function that every plugin must export.
///
/// Returns a heap-allocated `NovaPluginInfo` that the engine takes ownership of.
/// The plugin must ensure the strings remain valid as long as the plugin is loaded.
pub type PluginInitFn = unsafe extern "C" fn() -> *mut NovaPluginInfo;

/// Type of the widget builder function that a plugin exports per widget.
///
/// - `name`: null-terminated widget name
/// - `ctx`: pointer to a `NovaPluginCtx` owned by the engine
///
/// Returns an opaque pointer to a GTK4 widget (GObject). The engine takes
/// ownership of the returned widget reference.
pub type PluginWidgetBuildFn =
    unsafe extern "C" fn(name: *const c_char, ctx: *const NovaPluginCtx) -> *mut c_void;

/// Name of the init symbol that every plugin must export
pub const PLUGIN_INIT_SYMBOL: &[u8] = b"nova_plugin_init\0";

/// Name of the widget builder symbol (per widget, prefixed with widget name)
/// e.g. "nova_widget_build_myclock"
pub const PLUGIN_BUILD_PREFIX: &str = "nova_widget_build_";

/// Safely extract a Rust String from a C pointer returned by a plugin.
///
/// Returns None if the pointer is null, otherwise clones the string content.
///
/// # Safety
/// The pointer must point to a valid null-terminated C string.
pub unsafe fn cstr_to_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    std::ffi::CStr::from_ptr(ptr)
        .to_str()
        .ok()
        .map(|s| s.to_owned())
}
