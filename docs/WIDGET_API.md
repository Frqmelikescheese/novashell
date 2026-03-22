# NovaShell Widget API

## Built-in Data Sources

These are the `builtin:` values you can reference in any `.widget` YAML file.
They are evaluated by the engine on the specified `interval_ms`.

### Clock
| Source | Description | Example Output |
|--------|-------------|----------------|
| `clock::time` | Current time (HH:MM:SS) | `14:35:22` |
| `clock::date` | Current date (Month Day) | `March 21` |
| `clock::day`  | Day of week | `Friday` |

### System Monitor
| Source | Description | Example Output |
|--------|-------------|----------------|
| `sysmon::cpu_percent` | CPU usage % (0–100) | `42` |
| `sysmon::cpu_fraction` | CPU usage 0.0–1.0 | `0.420` |
| `sysmon::ram_used` | RAM used (human-readable) | `5.2 GB` |
| `sysmon::ram_fraction` | RAM fraction 0.0–1.0 | `0.650` |
| `sysmon::net_rx` | Network receive rate | `1.2 MB/s` |
| `sysmon::net_tx` | Network transmit rate | `256 KB/s` |

### Battery
| Source | Description | Example Output |
|--------|-------------|----------------|
| `battery::percent` | Battery % | `78` |
| `battery::fraction` | Battery 0.0–1.0 | `0.780` |
| `battery::status` | Charging / Discharging / Full | `Charging` |
| `battery::icon` | UTF-8 icon for charge level | `` |

### Volume
| Source | Description | Example Output |
|--------|-------------|----------------|
| `volume::fraction` | Volume 0.0–1.0 | `0.650` |
| `volume::percent` | Volume % | `65` |
| `volume::icon` | GTK icon name | `audio-volume-medium` |

### Media (MPRIS2)
| Source | Description | Example Output |
|--------|-------------|----------------|
| `media::title` | Track title | `Midnight City` |
| `media::artist` | Artist name | `M83` |
| `media::play_icon` | GTK icon name for play/pause | `media-playback-pause` |
| `media::position_fraction` | Playback position 0.0–1.0 | `0.340` |
| `media::art_path` | Path to album art file | `/tmp/art.jpg` |

## Widget YAML Format

```yaml
name: my_widget          # required — used in config.yaml as `widget: my_widget`
description: "..."       # optional

template: |              # required — XML widget tree
  <box class="my-widget" orientation="v" spacing="8">
    <label class="my-widget__value" text="{my_var}"/>
    <levelbar class="my-widget__bar" value="{my_fraction}"/>
  </box>

vars:                    # optional — dynamic variable definitions
  my_var:
    builtin: "clock::time"   # use a built-in source…
    # — or —
    script: "date +%H:%M"    # …or a shell command
    interval_ms: 1000        # refresh every N milliseconds
    default: "00:00"         # shown before first update

default_style: |         # optional — CSS applied for this widget type
  .my-widget { padding: 12px; border-radius: 10px; }
```

## Scripted Variables

Use `script:` to run any shell command. Stdout is used as the value.

```yaml
vars:
  kernel:
    script: "uname -r"
    interval_ms: 86400000   # once per day
    default: "unknown"
  uptime:
    script: "uptime -p | sed 's/up //'"
    interval_ms: 60000
    default: ""
  ip:
    script: "ip route get 1 | awk '{print $7; exit}'"
    interval_ms: 30000
    default: "0.0.0.0"
```

## Template XML Reference

All GTK attributes are passed as XML attributes. CSS classes use `class="..."`.
Multiple classes: `class="foo bar baz"`.

Variable interpolation: `text="{my_var}"` — any attribute value can use `{var}`.

### Supported elements

```xml
<!-- Container -->
<box orientation="h|v" spacing="N" halign="start|center|end|fill" valign="..." homogeneous="true">

<!-- Text -->
<label text="{var}" ellipsize="none|start|middle|end" max-width-chars="N" wrap="true" selectable="true"/>

<!-- Button (can contain children) -->
<button icon="icon-name" action="source::action">
  <label text="Click me"/>
</button>

<!-- Image -->
<image src="{art_path}" icon="icon-name" width="48" height="48" fallback-icon="audio-x-generic"/>

<!-- Progress / level -->
<levelbar value="{fraction}" min="0.0" max="1.0" orientation="h|v"/>
<progressbar value="{fraction}" show-text="true"/>
<scale value="{fraction}" min="0.0" max="1.0" orientation="h|v" action="vol::set"/>

<!-- Layout -->
<separator orientation="h|v"/>
<overlay/>          <!-- gtk4::Overlay — first child is background, rest are overlaid -->
<grid row-spacing="N" column-spacing="N" row-homogeneous="true"/>

<!-- Animation -->
<revealer transition="none|crossfade|slide-right|slide-left|slide-up|slide-down" duration="200">

<!-- Containers -->
<scrolled hscrollbar="auto|always|never" vscrollbar="auto|always|never"/>

<!-- Custom drawing (Cairo) -->
<drawing width="200" height="80"/>
```

## Plugin Widget API (C)

```c
#include <stdint.h>

typedef struct {
    void* engine_ptr;
    char* (*get_var)(void* engine, const char* var_name);
    void  (*log_info)(void* engine, const char* message);
    void  (*log_warn)(void* engine, const char* message);
    void  (*free_string)(char* ptr);
} NovaPluginCtx;

typedef struct {
    const char*  name;
    const char*  version;
    const char*  description;
    uint32_t     widget_count;
    const char** widget_names;
} NovaPluginInfo;

// Entry point — must be exported
NovaPluginInfo* nova_plugin_init(void);

// Variable provider — export to handle {var} in templates
char* nova_plugin_get_var(NovaPluginCtx* ctx, const char* var_name);
void  nova_plugin_free_string(char* ptr);
```

Install compiled `.so` to `~/.config/novashell/plugins/`.
