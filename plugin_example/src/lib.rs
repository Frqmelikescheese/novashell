//! Example NovaShell plugin: weather widget
//!
//! Shows how to write a `.so` plugin for NovaShell.
//! Build with: cargo build --release
//! Install to: ~/.config/novashell/plugins/nova_weather_plugin.so

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_uint, c_void};

// ── Plugin info struct (must match novashell_lib::plugin::api) ────────────

#[repr(C)]
pub struct NovaPluginInfo {
    pub name:         *const c_char,
    pub version:      *const c_char,
    pub description:  *const c_char,
    pub widget_count: c_uint,
    pub widget_names: *const *const c_char,
}

#[repr(C)]
pub struct NovaPluginCtx {
    pub engine_ptr: *mut c_void,
    pub get_var:    unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_char,
    pub log_info:   unsafe extern "C" fn(*mut c_void, *const c_char),
    pub log_warn:   unsafe extern "C" fn(*mut c_void, *const c_char),
    pub free_string: unsafe extern "C" fn(*mut c_char),
}

// ── Static strings ────────────────────────────────────────────────────────

static NAME:        &[u8] = b"nova_weather\0";
static VERSION:     &[u8] = b"0.1.0\0";
static DESCRIPTION: &[u8] = b"Weather widget using wttr.in\0";
static WIDGET_NAME: &[u8] = b"weather\0";

static mut WIDGET_NAMES: [*const c_char; 1] = [std::ptr::null()];
static mut PLUGIN_INFO: NovaPluginInfo = NovaPluginInfo {
    name:         NAME.as_ptr() as *const c_char,
    version:      VERSION.as_ptr() as *const c_char,
    description:  DESCRIPTION.as_ptr() as *const c_char,
    widget_count: 1,
    widget_names: std::ptr::null(),
};

// ── Plugin entry point ────────────────────────────────────────────────────

/// Called by NovaShell when the .so is loaded.
/// Returns a pointer to a static `NovaPluginInfo`.
///
/// # Safety
/// This function initialises static state. It must be called only once.
#[no_mangle]
pub unsafe extern "C" fn nova_plugin_init() -> *mut NovaPluginInfo {
    WIDGET_NAMES[0] = WIDGET_NAME.as_ptr() as *const c_char;
    PLUGIN_INFO.widget_names = WIDGET_NAMES.as_ptr();
    &mut PLUGIN_INFO as *mut NovaPluginInfo
}

/// Called by NovaShell to get the current value of a variable from this plugin.
/// The returned string must be freed by calling `nova_plugin_free_string`.
#[no_mangle]
pub unsafe extern "C" fn nova_plugin_get_var(
    _ctx: *mut NovaPluginCtx,
    var_name: *const c_char,
) -> *mut c_char {
    let name = unsafe { CStr::from_ptr(var_name) }.to_string_lossy();

    let value = match name.as_ref() {
        "weather::condition" => get_weather_condition(),
        "weather::temp"      => get_weather_temp(),
        "weather::location"  => get_weather_location(),
        _                    => String::new(),
    };

    CString::new(value)
        .unwrap_or_else(|_| CString::new("").unwrap())
        .into_raw()
}

#[no_mangle]
pub unsafe extern "C" fn nova_plugin_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}

// ── Weather data fetchers ─────────────────────────────────────────────────

fn get_weather_condition() -> String {
    // Fetch from wttr.in in format %C (condition text)
    run_cmd("curl -sf 'wttr.in?format=%C' 2>/dev/null || echo 'Unknown'")
}

fn get_weather_temp() -> String {
    run_cmd("curl -sf 'wttr.in?format=%t' 2>/dev/null || echo '?°C'")
}

fn get_weather_location() -> String {
    run_cmd("curl -sf 'wttr.in?format=%l' 2>/dev/null || echo ''")
}

fn run_cmd(cmd: &str) -> String {
    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output();
    match out {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        }
        _ => String::new(),
    }
}
