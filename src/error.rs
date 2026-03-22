use thiserror::Error;

#[derive(Error, Debug)]
pub enum NovaError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Render error: {0}")]
    Render(String),

    #[error("IPC error: {0}")]
    Ipc(String),

    #[error("Plugin error: {0}")]
    Plugin(String),

    #[error("Widget error: {0}")]
    Widget(String),

    #[error("CSS error: {0}")]
    Css(String),

    #[error("IO error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },

    #[error("File watch error: {0}")]
    Watch(String),

    #[error("YAML parse error: {source}")]
    Yaml {
        #[from]
        source: serde_yaml::Error,
    },

    #[error("JSON error: {source}")]
    Json {
        #[from]
        source: serde_json::Error,
    },

    #[error("D-Bus error: {0}")]
    DBus(String),

    #[error("XML parse error: {0}")]
    Xml(String),
}

pub type Result<T> = std::result::Result<T, NovaError>;

impl From<notify::Error> for NovaError {
    fn from(e: notify::Error) -> Self {
        NovaError::Watch(e.to_string())
    }
}

impl From<zbus::Error> for NovaError {
    fn from(e: zbus::Error) -> Self {
        NovaError::DBus(e.to_string())
    }
}
