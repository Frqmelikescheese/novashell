use crate::error::{NovaError, Result};
use crate::ipc::protocol::{IpcCommand, IpcResponse};
use crate::state::SharedState;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tracing::{debug, error, info, warn};

/// Returns the canonical path to the IPC socket.
///
/// Uses `$XDG_RUNTIME_DIR/nova.sock` if available,
/// otherwise falls back to `/tmp/novashell-<UID>.sock`.
pub fn socket_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("nova.sock")
    } else {
        let uid = unsafe { libc::getuid() };
        PathBuf::from(format!("/tmp/nova-{uid}.sock"))
    }
}

/// Async Unix socket IPC server.
///
/// Listens for JSON-encoded `IpcCommand` messages and dispatches them to
/// a handler, writing back a JSON `IpcResponse`.
pub struct IpcServer {
    socket_path: PathBuf,
    state: SharedState,
    /// Sender used to trigger actions on the GTK main thread
    action_tx: crossbeam_channel::Sender<IpcAction>,
}

/// Actions dispatched from the IPC server to the GTK main thread
#[derive(Debug, Clone)]
pub enum IpcAction {
    Reload,
    ReloadCss,
    Toggle(String),
    Show(String),
    Hide(String),
    Move { target: String, x: i32, y: i32 },
    SetProfile(String),
    Quit,
}

impl IpcServer {
    /// Create a new IPC server.
    pub fn new(
        state: SharedState,
        action_tx: crossbeam_channel::Sender<IpcAction>,
    ) -> Self {
        Self {
            socket_path: socket_path(),
            state,
            action_tx,
        }
    }

    /// Create a new IPC server with a custom socket path.
    pub fn with_path(
        path: PathBuf,
        state: SharedState,
        action_tx: crossbeam_channel::Sender<IpcAction>,
    ) -> Self {
        Self {
            socket_path: path,
            state,
            action_tx,
        }
    }

    /// Start listening for connections. This is an async loop that runs indefinitely.
    pub async fn run(&self) -> Result<()> {
        // Remove stale socket if it exists
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path).ok();
        }

        let listener = UnixListener::bind(&self.socket_path).map_err(|e| {
            NovaError::Ipc(format!(
                "Cannot bind to socket {}: {e}",
                self.socket_path.display()
            ))
        })?;

        info!("IPC server listening on {}", self.socket_path.display());

        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    let state = self.state.clone();
                    let tx = self.action_tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, state, tx).await {
                            warn!("IPC: connection handler error: {e}");
                        }
                    });
                }
                Err(e) => {
                    error!("IPC: accept error: {e}");
                }
            }
        }
    }
}

/// Handle a single IPC connection: read command, dispatch, write response
async fn handle_connection(
    stream: UnixStream,
    state: SharedState,
    action_tx: crossbeam_channel::Sender<IpcAction>,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    let bytes = buf_reader.read_line(&mut line).await.map_err(|e| {
        NovaError::Ipc(format!("Read error: {e}"))
    })?;

    if bytes == 0 {
        return Ok(());
    }

    debug!("IPC: received: {}", line.trim());

    let cmd: IpcCommand = match serde_json::from_str(line.trim()) {
        Ok(c) => c,
        Err(e) => {
            let resp = IpcResponse::err(format!("Invalid JSON command: {e}"));
            let json = serde_json::to_string(&resp)? + "\n";
            writer.write_all(json.as_bytes()).await.ok();
            return Ok(());
        }
    };

    let response = dispatch_command(cmd, &state, &action_tx);

    let json = serde_json::to_string(&response)? + "\n";
    writer.write_all(json.as_bytes()).await.map_err(|e| {
        NovaError::Ipc(format!("Write error: {e}"))
    })?;

    Ok(())
}

/// Dispatch an IpcCommand and return a response
fn dispatch_command(
    cmd: IpcCommand,
    state: &SharedState,
    action_tx: &crossbeam_channel::Sender<IpcAction>,
) -> IpcResponse {
    match cmd {
        IpcCommand::Reload => {
            action_tx.send(IpcAction::Reload).ok();
            IpcResponse::ok("Reload triggered")
        }
        IpcCommand::ReloadCss => {
            action_tx.send(IpcAction::ReloadCss).ok();
            IpcResponse::ok("CSS reload triggered")
        }
        IpcCommand::Toggle { target } => {
            action_tx.send(IpcAction::Toggle(target.clone())).ok();
            IpcResponse::ok(format!("Toggle '{target}'"))
        }
        IpcCommand::Show { target } => {
            action_tx.send(IpcAction::Show(target.clone())).ok();
            IpcResponse::ok(format!("Show '{target}'"))
        }
        IpcCommand::Hide { target } => {
            action_tx.send(IpcAction::Hide(target.clone())).ok();
            IpcResponse::ok(format!("Hide '{target}'"))
        }
        IpcCommand::Move { target, x, y } => {
            action_tx
                .send(IpcAction::Move {
                    target: target.clone(),
                    x,
                    y,
                })
                .ok();
            IpcResponse::ok(format!("Move '{target}' to ({x},{y})"))
        }
        IpcCommand::SetProfile { name } => {
            action_tx.send(IpcAction::SetProfile(name.clone())).ok();
            IpcResponse::ok(format!("Profile set to '{name}'"))
        }
        IpcCommand::ListWidgets => {
            let state_read = state.read();
            let widgets: Vec<serde_json::Value> = state_read
                .widget_registry
                .names()
                .into_iter()
                .map(|name| {
                    let visible = state_read.is_visible(&name);
                    serde_json::json!({
                        "id": name,
                        "visible": visible,
                    })
                })
                .collect();

            IpcResponse::ok_data(
                format!("{} widgets", widgets.len()),
                serde_json::Value::Array(widgets),
            )
        }
        IpcCommand::Quit => {
            action_tx.send(IpcAction::Quit).ok();
            IpcResponse::ok("Shutting down")
        }
    }
}

/// IPC client for sending commands from the CLI to the daemon
pub struct IpcClient {
    socket_path: PathBuf,
}

impl IpcClient {
    /// Create a client using the default socket path
    pub fn new() -> Self {
        Self {
            socket_path: socket_path(),
        }
    }

    /// Create a client with a custom socket path
    pub fn with_path(path: PathBuf) -> Self {
        Self { socket_path: path }
    }

    /// Send a command to the daemon and return the response.
    ///
    /// Uses a blocking std Unix socket for simplicity in the CLI context.
    pub fn send(&self, command: &IpcCommand) -> Result<IpcResponse> {
        use std::io::{BufRead, Write};
        use std::os::unix::net::UnixStream;
        use std::time::Duration;

        let stream = UnixStream::connect(&self.socket_path).map_err(|e| {
            NovaError::Ipc(format!(
                "Cannot connect to socket {}: {e}\nIs the nova daemon running?",
                self.socket_path.display()
            ))
        })?;

        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .ok();
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .ok();

        let mut writer = stream.try_clone().map_err(|e| NovaError::Ipc(e.to_string()))?;
        let reader = std::io::BufReader::new(stream);

        let json = serde_json::to_string(command)? + "\n";
        writer.write_all(json.as_bytes())?;

        let mut response_line = String::new();
        reader
            .lines()
            .next()
            .ok_or_else(|| NovaError::Ipc("Empty response from daemon".to_string()))?
            .map_err(|e| NovaError::Ipc(e.to_string()))
            .and_then(|line| {
                serde_json::from_str(&line).map_err(|e| {
                    NovaError::Ipc(format!("Invalid JSON response: {e}"))
                })
            })
    }
}

impl Default for IpcClient {
    fn default() -> Self {
        Self::new()
    }
}
