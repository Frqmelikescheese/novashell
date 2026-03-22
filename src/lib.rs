//! NovaShell — modular Linux desktop ricing engine.
//!
//! This library exposes the core engine for use by the binary,
//! integration tests, and external plugins.

pub mod cli;
pub mod config;
pub mod css;
pub mod error;
pub mod ipc;
pub mod plugin;
pub mod renderer;
pub mod state;
pub mod widgets;

pub use error::{NovaError, Result};
