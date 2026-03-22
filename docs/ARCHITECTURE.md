# NovaShell Architecture

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│                      novashell daemon                        │
│                                                             │
│  ┌──────────┐   ┌──────────────┐   ┌────────────────────┐  │
│  │  Config  │   │  CSS Manager │   │   Plugin Loader    │  │
│  │  Loader  │   │  (GTK CSS    │   │  (libloading .so)  │  │
│  │  (YAML)  │   │   Provider)  │   │                    │  │
│  └────┬─────┘   └──────┬───────┘   └────────┬───────────┘  │
│       │                │                     │              │
│  ┌────▼─────────────────▼─────────────────────▼──────────┐  │
│  │                  App State (Arc<RwLock<AppState>>)     │  │
│  │  config | css_loaded | widget_registry | profiles      │  │
│  └────────────────────────┬──────────────────────────────┘  │
│                           │                                  │
│  ┌────────────────────────▼──────────────────────────────┐  │
│  │                     Renderer                           │  │
│  │                                                        │  │
│  │   ┌──────────────┐     ┌────────────────────────────┐ │  │
│  │   │  Layer Shell │     │    Widget Factory           │ │  │
│  │   │  Window Mgr  │     │  (XML template → GTK tree) │ │  │
│  │   │  (GTK4 +     │ ──► │                            │ │  │
│  │   │  layer-shell)│     │  template var interpolation│ │  │
│  │   └──────────────┘     │  glib timers / data sources│ │  │
│  │                        └────────────────────────────┘ │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                             │
│  ┌──────────────────────┐   ┌───────────────────────────┐  │
│  │    IPC Server        │   │    File Watcher            │  │
│  │  (Unix socket)       │   │  (notify crate)            │  │
│  │  tokio async         │   │  → CSS/Config hot-reload   │  │
│  └──────────────────────┘   └───────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
            ▲
            │  JSON over Unix socket
            ▼
┌─────────────────┐
│  novashell CLI  │
│  (IPC client)   │
└─────────────────┘
```

## Module Map

| Module | Path | Responsibility |
|--------|------|----------------|
| CLI | `src/cli/` | Argument parsing (clap) |
| Config | `src/config/` | YAML loading, hot-reload watcher |
| State | `src/state/` | Shared `Arc<RwLock<AppState>>` |
| Renderer | `src/renderer/` | GTK4 app + layer shell windows |
| WidgetFactory | `src/renderer/widget_factory.rs` | XML template → GTK widget tree |
| Widgets | `src/widgets/` | Built-in widget implementations + data sources |
| CSS | `src/css/` | `GtkCssProvider` loading |
| IPC | `src/ipc/` | Unix socket server + client |
| Plugin | `src/plugin/` | `.so` loader + C ABI |

## Rendering Pipeline

```
config.yaml (YAML)
    │
    ▼
NovaConfig (serde structs)
    │
    ├── screens[name].widgets[n].widget  →  WidgetDefinition
    │                                         (from builtin or .widget YAML)
    │
    ▼
WidgetFactory::build(def, instance_config)
    │
    ├── Parse XML template  ──► GTK widget tree
    ├── Apply CSS classes
    ├── Interpolate {vars} using builtin sources / shell scripts
    ├── Set up glib::timeout timers for live updates
    └── Return gtk4::Widget
         │
         ▼
    ApplicationWindow (gtk4-layer-shell)
    • set_layer(Top/Bottom/Overlay/Background)
    • set_anchor(Top, Right, ...)
    • set_margin(top, right, bottom, left)
    • add_widget(gtk_widget)
    • show()
```

## Widget Definition Format (`.widget` YAML)

```yaml
name: my_widget
description: "Short description"
template: |
  <box class="my-widget" orientation="v">
    <label class="my-widget__value" text="{my_var}"/>
  </box>
vars:
  my_var:
    builtin: "clock::time"  # or use `script: "date +%H:%M"`
    interval_ms: 1000
    default: "00:00"
default_style: |
  .my-widget { padding: 10px; border-radius: 8px; }
```

### Template XML elements

| Element | GTK4 Widget | Key attributes |
|---------|-------------|----------------|
| `<box>` | `gtk4::Box` | `orientation`, `spacing`, `halign`, `valign` |
| `<label>` | `gtk4::Label` | `text`, `ellipsize`, `max-width-chars`, `wrap` |
| `<button>` | `gtk4::Button` | `icon`, `label`, `action` |
| `<image>` | `gtk4::Image` | `src`, `icon`, `width`, `height` |
| `<levelbar>` | `gtk4::LevelBar` | `value`, `min`, `max`, `orientation` |
| `<progressbar>` | `gtk4::ProgressBar` | `value` |
| `<scale>` | `gtk4::Scale` | `value`, `min`, `max`, `orientation`, `action` |
| `<separator>` | `gtk4::Separator` | `orientation` |
| `<drawing>` | `gtk4::DrawingArea` | `width`, `height` |
| `<revealer>` | `gtk4::Revealer` | `transition`, `duration` |
| `<overlay>` | `gtk4::Overlay` | — |
| `<scrolled>` | `gtk4::ScrolledWindow` | `hscrollbar`, `vscrollbar` |

## IPC Protocol

Commands are JSON sent over `$XDG_RUNTIME_DIR/novashell.sock`:

```json
{"cmd":"reload"}
{"cmd":"reload_css"}
{"cmd":"toggle","target":"clock-main"}
{"cmd":"show","target":"screen:primary"}
{"cmd":"hide","target":"sysmon-main"}
{"cmd":"move","target":"clock-main","x":100,"y":50}
{"cmd":"set_profile","name":"laptop"}
{"cmd":"list_widgets"}
{"cmd":"quit"}
```

Response:
```json
{"ok":true,"message":"Reload triggered"}
{"ok":true,"message":"3 widgets","data":[{"id":"clock-main","visible":true},...]}
{"ok":false,"message":"Widget 'foo' not found"}
```

## Plugin API (C ABI)

```c
// Your plugin must export this symbol:
NovaPluginInfo* nova_plugin_init();

// Optionally export for variable queries:
char* nova_plugin_get_var(NovaPluginCtx* ctx, const char* var_name);
void  nova_plugin_free_string(char* ptr);

typedef struct {
    const char* name;
    const char* version;
    const char* description;
    uint32_t    widget_count;
    const char** widget_names;
} NovaPluginInfo;
```

See `plugin_example/` for a full Rust plugin template.
