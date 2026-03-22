use serde::{Deserialize, Serialize};

/// Commands that can be sent from the CLI to the daemon via IPC
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum IpcCommand {
    /// Reload the full config and re-render all widgets
    Reload,

    /// Reload only CSS stylesheets
    ReloadCss,

    /// Toggle visibility of a widget or screen
    Toggle { target: String },

    /// Show a hidden widget or screen
    Show { target: String },

    /// Hide a visible widget or screen
    Hide { target: String },

    /// Move a widget to a new absolute position
    Move { target: String, x: i32, y: i32 },

    /// Switch to a named configuration profile
    SetProfile { name: String },

    /// List all active widgets and their visibility states
    ListWidgets,

    /// Gracefully shut down the daemon
    Quit,
}

/// Response returned by the daemon after processing an IPC command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    /// Whether the command was processed successfully
    pub ok: bool,

    /// Human-readable status message
    pub message: String,

    /// Optional structured data payload (e.g. widget list)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl IpcResponse {
    /// Create a successful response with a message
    pub fn ok(message: impl Into<String>) -> Self {
        Self {
            ok: true,
            message: message.into(),
            data: None,
        }
    }

    /// Create a successful response with message and data
    pub fn ok_data(message: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            ok: true,
            message: message.into(),
            data: Some(data),
        }
    }

    /// Create an error response
    pub fn err(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            message: message.into(),
            data: None,
        }
    }
}
