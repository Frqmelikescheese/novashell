# NovaShell — Implementation Milestones

## Phase 0 — Foundation (complete)
- [x] Project structure and Cargo workspace
- [x] Config schema (YAML → typed structs)
- [x] CLI parsing with all subcommands
- [x] Error types
- [x] Shared state (Arc<RwLock<AppState>>)

## Phase 1 — Rendering Core (complete)
- [x] GTK4 Application bootstrap
- [x] Wayland layer shell window creation
- [x] Anchor point → layer shell edge/margin mapping
- [x] XML template parser → GTK widget tree
- [x] CSS provider loading
- [x] Template variable interpolation {var_name}

## Phase 2 — Built-in Widgets (complete)
- [x] Clock (chrono + glib timer)
- [x] System monitor (sysinfo: CPU, RAM, net)
- [x] Cava visualizer (raw pipe + DrawingArea + Cairo)
- [x] MPRIS2 media player (zbus D-Bus)
- [x] Battery meter (/sys/class/power_supply/)
- [x] Volume (wpctl/amixer subprocess)
- [x] App launcher (.desktop file reader)

## Phase 3 — Hot Reload (complete)
- [x] notify file watcher on config dir
- [x] CSS hot-reload
- [x] Full config hot-reload
- [x] Widget file (.widget) hot-reload

## Phase 4 — IPC (complete)
- [x] Unix socket server (tokio)
- [x] JSON protocol
- [x] IpcClient for CLI → daemon communication
- [x] All commands: reload, reload-css, toggle, show, hide, move, set-profile, list, quit

## Phase 5 — Plugin System (complete)
- [x] C ABI plugin interface (libloading)
- [x] Plugin .so scanner
- [x] Plugin callback context (get_var, log)
- [x] Example plugin (weather via wttr.in)

## Phase 6 — Polish (in progress)
- [ ] Dynamic widget move via layer shell margin update
- [ ] Per-profile screen switching
- [ ] Startup revealer animations
- [ ] Widget drag-to-reposition
- [ ] novashell init TUI wizard
- [ ] Shell completions (clap_complete)
- [ ] Man page (clap_mangen)
- [ ] X11 support via XWayland

## Phase 7 — Community Widgets (planned)
- [ ] Weather (wttr.in / OpenWeatherMap)
- [ ] Disk usage
- [ ] GPU monitor (NVML / ROCm sysfs)
- [ ] Workspace indicator (Hyprland IPC / sway IPC)
- [ ] Notification counter (dunst / mako D-Bus)
- [ ] Network speed sparkline
- [ ] Pomodoro timer
- [ ] GitHub notifications

## Phase 8 — Theme Ecosystem (planned)
- [ ] Theme marketplace CLI (novashell theme install catppuccin)
- [ ] Waybar JSON theme import helper
- [ ] Eww widget import converter
