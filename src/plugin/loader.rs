/// Re-export: plugin loading is implemented in the parent `plugin` module.
///
/// This file exists to keep the module tree explicit and to allow future
/// expansion of the plugin loader API without touching `mod.rs`.
pub use super::{LoadedPlugin, PluginLoader};
