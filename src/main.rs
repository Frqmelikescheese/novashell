use anyhow::Result;
use clap::Parser;
use gtk4::prelude::*;
use gtk4::glib;
use gio::prelude::*;
use novashell_lib::{
    cli::{NovaCli, NovaCommand},
    config::ConfigLoader,
    css::CssManager,
    ipc::{
        protocol::IpcCommand,
        server::{IpcAction, IpcClient, IpcServer},
    },
    plugin::PluginLoader,
    renderer::Renderer,
    state,
    widgets::BuiltinRegistry,
};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

fn main() -> Result<()> {
    let cli = NovaCli::parse();

    // Initialise logging
    let filter = std::env::var("NOVASHELL_LOG")
        .unwrap_or_else(|_| cli.log_level.clone());
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&filter)),
        )
        .with_target(false)
        .compact()
        .init();

    match &cli.command {
        // ----- Daemon -------------------------------------------------------
        None | Some(NovaCommand::Daemon { .. }) => {
            run_daemon(&cli)
        }
        // ----- IPC commands -------------------------------------------------
        Some(cmd) => {
            let socket_override = cli.socket.as_deref().map(std::path::PathBuf::from);
            let client = match socket_override {
                Some(p) => IpcClient::with_path(p),
                None => IpcClient::new(),
            };

            let ipc_cmd = cli_to_ipc(cmd);
            let resp = client.send(&ipc_cmd)?;

            if resp.ok {
                println!("{}", resp.message);
                if let Some(data) = resp.data {
                    println!("{}", serde_json::to_string_pretty(&data)?);
                }
                Ok(())
            } else {
                eprintln!("Error: {}", resp.message);
                std::process::exit(1);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Daemon startup
// ---------------------------------------------------------------------------

fn run_daemon(cli: &NovaCli) -> Result<()> {
    info!("Nova v{} starting up", env!("CARGO_PKG_VERSION"));

    // Load config
    let mut config_loader = match &cli.config {
        Some(path) => ConfigLoader::load(path)?,
        None => ConfigLoader::load_default()?,
    };

    let config_dir = config_loader.config_dir();
    info!("Config dir: {}", config_dir.display());

    // Build shared state
    let shared_state = state::new_shared(config_loader.config.clone(), config_loader.path.clone());

    // Load plugins from ~/.config/novashell/plugins/
    {
        let plugin_dir = config_dir.join("plugins");
        let mut loader = PluginLoader::new();
        loader.scan_and_load(&plugin_dir);

        if loader.plugin_count() > 0 {
            info!(
                "Loaded {} plugin(s): {}",
                loader.plugin_count(),
                loader.plugin_names().join(", ")
            );
        }

        // Register plugin-contributed widget definitions into state
        let mut state_write = shared_state.write();
        for (_, def) in loader.contributed_definitions() {
            state_write.widget_registry.register(def.clone());
        }
    }

    // Register built-in widget definitions into state (pre-load from widgets dir)
    {
        let builtin = BuiltinRegistry::new();
        let widgets_dir = config_dir.join("widgets");
        if widgets_dir.exists() {
            let entries = std::fs::read_dir(&widgets_dir)
                .unwrap_or_else(|_| std::fs::read_dir("/dev/null").unwrap());
            let mut state_write = shared_state.write();
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("widget") {
                    match novashell_lib::widgets::WidgetDefinition::load_from_file(&path) {
                        Ok(def) => {
                            state_write.widget_registry.register(def);
                        }
                        Err(e) => {
                            warn!("Failed to load widget file {}: {e}", path.display());
                        }
                    }
                }
            }
        }
    }

    // Set up IPC action channel
    let (action_tx, action_rx) = crossbeam_channel::unbounded::<IpcAction>();

    // Spawn async IPC server on a background tokio thread
    let ipc_state = shared_state.clone();
    let ipc_tx = action_tx.clone();
    let ipc_socket = cli.socket.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");

        rt.block_on(async move {
            let server = match ipc_socket {
                Some(p) => IpcServer::with_path(
                    std::path::PathBuf::from(p),
                    ipc_state,
                    ipc_tx,
                ),
                None => IpcServer::new(ipc_state, ipc_tx),
            };

            if let Err(e) = server.run().await {
                error!("IPC server error: {e}");
            }
        });
    });

    // Start GTK4 application on the main thread
    let app = gtk4::Application::builder()
        .application_id("io.nova.Nova")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();

    let shared_state_clone = shared_state.clone();
    let config_dir_clone = config_dir.clone();
    let action_tx_gtk = action_tx.clone();

    app.connect_activate(move |app: &gtk4::Application| {
        info!("GTK application activated");

        // Apply CSS
        let css_mgr = CssManager::new(&config_dir_clone);
        if let Err(e) = css_mgr.load() {
            warn!("CSS load warning: {e}");
        }

        // Create renderer and render all screens
        let mut renderer = Renderer::new(app.clone(), shared_state_clone.clone());
        renderer.render_all();

        // Store renderer and CSS manager in a RefCell so we can mutate in callbacks
        let renderer_cell = std::cell::RefCell::new(renderer);
        let css_cell = std::cell::RefCell::new(css_mgr);

        // Set up a glib idle handler that drains the IPC action channel.
        // Clone action_rx because connect_activate is Fn (may be called again).
        let action_rx = action_rx.clone();
        let shared_state_idle = shared_state_clone.clone();
        let config_dir_idle = config_dir_clone.clone();
        let app_clone = app.clone();

        glib::timeout_add_local(
            std::time::Duration::from_millis(50),
            move || {
                // Drain actions sent from the IPC server
                while let Ok(action) = action_rx.try_recv() {
                    match action {
                        IpcAction::Reload => {
                            info!("IPC: full reload");
                            let config_path = shared_state_idle.read().config_path.clone();
                            match ConfigLoader::load(&config_path) {
                                Ok(loader) => {
                                    shared_state_idle.write().config = loader.config;
                                    info!("Config reloaded from {}", config_path.display());
                                }
                                Err(e) => {
                                    warn!("Config reload failed: {e}");
                                }
                            }
                            renderer_cell.borrow_mut().reload();
                        }
                        IpcAction::ReloadCss => {
                            info!("IPC: CSS reload");
                            if let Err(e) = css_cell.borrow().reload() {
                                warn!("CSS reload error: {e}");
                            }
                        }
                        IpcAction::Toggle(target) => {
                            renderer_cell.borrow_mut().toggle_target(&target);
                        }
                        IpcAction::Show(target) => {
                            renderer_cell.borrow_mut().show_target(&target);
                        }
                        IpcAction::Hide(target) => {
                            renderer_cell.borrow_mut().hide_target(&target);
                        }
                        IpcAction::Move { target, x, y } => {
                            warn!("IPC: move '{target}' to ({x},{y}) — dynamic move not yet implemented");
                        }
                        IpcAction::SetProfile(name) => {
                            info!("IPC: set profile '{name}'");
                            shared_state_idle.write().active_profile = name;
                        }
                        IpcAction::Quit => {
                            info!("IPC: quit requested");
                            app_clone.quit();
                            return glib::ControlFlow::Break;
                        }
                    }
                }

                glib::ControlFlow::Continue
            },
        );

        // Set up file watcher for hot-reload
        if shared_state_clone.read().config.novashell.hot_reload {
            setup_file_watcher(
                &config_dir_clone,
                action_tx_gtk.clone(),
            );
        }
    });

    // Spawn cava with the novashell cava config if available
    let cava_conf = config_dir.join("cava.conf");
    let mut cava_child: Option<std::process::Child> = None;
    if cava_conf.exists() {
        match std::process::Command::new("cava")
            .arg("-p")
            .arg(&cava_conf)
            .spawn()
        {
            Ok(child) => {
                info!("Spawned cava (pid {})", child.id());
                cava_child = Some(child);
            }
            Err(e) => warn!("Could not spawn cava: {e}"),
        }
    }

    let _exit_code = app.run_with_args::<String>(&[]);

    // Kill cava when the daemon exits
    if let Some(mut child) = cava_child {
        child.kill().ok();
    }
    info!("GTK application exited");
    Ok(())
}

// ---------------------------------------------------------------------------
// File watcher setup
// ---------------------------------------------------------------------------

fn setup_file_watcher(
    config_dir: &std::path::Path,
    action_tx: crossbeam_channel::Sender<IpcAction>,
) {
    use notify::{RecursiveMode, Watcher};

    let config_dir = config_dir.to_path_buf();

    std::thread::spawn(move || {
        let (tx, rx) = crossbeam_channel::unbounded();

        let mut watcher = match notify::recommended_watcher(move |res| {
            tx.send(res).ok();
        }) {
            Ok(w) => w,
            Err(e) => {
                warn!("File watcher setup failed: {e}");
                return;
            }
        };

        if let Err(e) = watcher.watch(&config_dir, RecursiveMode::Recursive) {
            warn!("Cannot watch config dir: {e}");
            return;
        }

        info!("Hot-reload watcher active on {}", config_dir.display());

        let mut last_reload = std::time::Instant::now();
        let mut last_css_reload = std::time::Instant::now();
        let debounce = std::time::Duration::from_millis(300);

        for res in rx {
            match res {
                Ok(event) => {
                    use notify::EventKind::*;
                    match event.kind {
                        Create(_) | Modify(_) | Remove(_) => {
                            for path in &event.paths {
                                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                                    match ext {
                                        "css" => {
                                            if last_css_reload.elapsed() >= debounce {
                                                debug!("Watcher: CSS changed: {}", path.display());
                                                action_tx.send(IpcAction::ReloadCss).ok();
                                                last_css_reload = std::time::Instant::now();
                                            }
                                        }
                                        "yaml" | "toml" | "json" | "widget" => {
                                            if last_reload.elapsed() >= debounce {
                                                debug!("Watcher: Config changed: {}", path.display());
                                                action_tx.send(IpcAction::Reload).ok();
                                                last_reload = std::time::Instant::now();
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    warn!("File watcher error: {e}");
                }
            }
        }
    });
}

// ---------------------------------------------------------------------------
// CLI → IPC translation
// ---------------------------------------------------------------------------

fn cli_to_ipc(cmd: &NovaCommand) -> IpcCommand {
    match cmd {
        NovaCommand::Daemon { .. } => IpcCommand::Reload, // unreachable, handled above
        NovaCommand::Reload => IpcCommand::Reload,
        NovaCommand::ReloadCss => IpcCommand::ReloadCss,
        NovaCommand::Toggle { target } => IpcCommand::Toggle { target: target.clone() },
        NovaCommand::Show { target } => IpcCommand::Show { target: target.clone() },
        NovaCommand::Hide { target } => IpcCommand::Hide { target: target.clone() },
        NovaCommand::Move { target, x, y } => IpcCommand::Move {
            target: target.clone(),
            x: *x,
            y: *y,
        },
        NovaCommand::SetProfile { name } => IpcCommand::SetProfile { name: name.clone() },
        NovaCommand::List => IpcCommand::ListWidgets,
        NovaCommand::Quit => IpcCommand::Quit,
    }
}
