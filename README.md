<div align="center">

```
███╗   ██╗ ██████╗ ██╗   ██╗ █████╗
████╗  ██║██╔═══██╗██║   ██║██╔══██╗
██╔██╗ ██║██║   ██║██║   ██║███████║
██║╚██╗██║██║   ██║╚██╗ ██╔╝██╔══██║
██║ ╚████║╚██████╔╝ ╚████╔╝ ██║  ██║
╚═╝  ╚═══╝ ╚═════╝   ╚═══╝  ╚═╝  ╚═╝
```

**Modular desktop ricing engine for Wayland**

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](#)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-orange.svg)](#)
[![GTK4](https://img.shields.io/badge/GTK-4-green.svg)](#)
[![Wayland](https://img.shields.io/badge/Wayland-layer%20shell-blue.svg)](#)

</div>

---

Nova is a Wayland desktop shell written in Rust. It renders transparent overlay widgets directly onto your desktop — clock, system monitor, media player, audio visualizer, volume — all fully customizable via YAML config and CSS. Hot-reload means changes apply instantly without restarting.

## Features

- **Ghost UI** — fully transparent widgets that feel like part of the wallpaper
- **Hot-reload** — edit config or CSS and see changes immediately
- **Built-in widgets** — clock, sysmon, cava, volume, media, battery, launcher
- **Custom widgets** — write shell scripts or YAML widget files, drop them in
- **Themes** — swap the entire look with one CSS file
- **Plugin API** — extend with native Rust plugins (C-ABI `.so`)
- **IPC** — control the daemon at runtime via `nova toggle`, `nova show`, etc.
- **Multi-monitor** — independent widget layouts per screen
- **Cava integration** — auto-spawns cava, renders bars with gradient + mirror support

## Requirements

- Wayland compositor with wlr-layer-shell support (Hyprland, Sway, river, niri, etc.)
- GTK4 + gtk4-layer-shell
- Rust toolchain (for building)
- `playerctl` — media widget
- `wpctl` or `amixer` — volume widget
- `cava` — audio visualizer (optional)

## Install

```bash
git clone https://github.com/Frqmelikescheese/novashell
cd novashell
cargo build --release
cp target/release/nova ~/.local/bin/nova
cp -r default_config/* ~/.config/novashell/
```

Or use the install script:

```bash
bash install.sh
```

## Usage

```bash
nova daemon          # start the shell
nova reload          # hot-reload config
nova reload-css      # hot-reload CSS only
nova toggle <id>     # toggle widget visibility
nova show <id>       # show widget
nova hide <id>       # hide widget
nova quit            # stop the daemon
nova list            # list running widgets
```

## Config

Config lives at `~/.config/novashell/config.yaml`. Widgets are placed per screen with anchor positioning.

```yaml
novashell:
  hot_reload: true

screens:
  main:
    monitor: HDMI-A-1
    layer: bottom
    widgets:
      - widget: clock
        id: clock
        vars:
          time_format: "%H:%M"
          date_format: "%A, %b %d"
        position:
          anchor: top        # top-left, top, top-right, left, center, right, bottom-left, bottom, bottom-right
          margin: [24, 0, 0, 0]
```

## Widgets

### clock
| var | default | description |
|-----|---------|-------------|
| `time_format` | `%H:%M:%S` | strftime format |
| `date_format` | `%B %-d` | strftime format |
| `day_format` | `%A` | strftime format, empty = hide |

### sysmon
| var | default | description |
|-----|---------|-------------|
| `show_net` | `true` | show network RX/TX rows |
| `show_swap` | `false` | show swap usage row |
| `show_title` | `true` | show title label |
| `title` | `System` | title text |
| `cpu_label` | `CPU` | CPU row label |
| `ram_label` | `RAM` | RAM row label |
| `swap_label` | `SWP` | swap row label |
| `net_rx_label` | `NET↓` | RX row label |
| `net_tx_label` | `NET↑` | TX row label |
| `update_interval` | `2` | seconds between updates |

### cava
| var | default | description |
|-----|---------|-------------|
| `bar_count` | `20` | number of frequency bars |
| `color_active` | `#89b4fa` | bar color (hex) |
| `gradient_end` | _(none)_ | second color for top→bottom gradient |
| `gap` | `2` | pixels between bars |
| `width` | `280` | canvas width px |
| `height` | `80` | canvas height px |
| `pipe` | `/tmp/cava_input` | path to cava raw pipe |
| `mirror` | `false` | mirror bars symmetrically |
| `bar_style` | `sharp` | `sharp` or `rounded` |
| `opacity` | `1.0` | global opacity 0.0–1.0 |

### volume
| var | default | description |
|-----|---------|-------------|
| `show_icon` | `true` | show icon/button |
| `show_label` | `true` | show percentage label |
| `show_slider` | `true` | show slider |
| `text_icon` | `false` | use Nerd Font glyph instead of GTK icon |
| `mute_icon` | `󰝟` | glyph shown when muted (text_icon mode) |
| `max_volume` | `1.0` | slider max (set to `1.5` for 150%) |
| `update_interval` | `1000` | milliseconds between polls |

### media
| var | default | description |
|-----|---------|-------------|
| `show_art` | `true` | show album art |
| `show_album` | `true` | show album name |
| `show_player` | `true` | show player name label |
| `show_time` | `true` | show position / duration |
| `show_shuffle` | `false` | show shuffle button |
| `hide_stopped` | `true` | hide widget when nothing is playing |
| `art_size` | `48` | album art size px |
| `max_chars` | `24` | truncate title/artist at N chars |

### battery
| var | default | description |
|-----|---------|-------------|
| `show_icon` | `true` | show battery icon |
| `show_bar` | `true` | show level bar |
| `show_status` | `true` | show status text |
| `threshold_warning` | `20` | % to add `.warning` CSS class |
| `threshold_critical` | `10` | % to add `.critical` CSS class |
| `update_interval` | `30` | seconds between updates |

### exec (custom command widget)
| var | default | description |
|-----|---------|-------------|
| `command` | _(required)_ | shell command to run |
| `interval` | `5` | seconds between runs |
| `format` | `{output}` | template, `{output}` = stdout |
| `markup` | `false` | enable Pango markup |
| `align` | `left` | `left`, `center`, `right` |
| `on_click` | _(none)_ | command on left-click |

## Themes

Themes are single CSS files in `~/.config/novashell/themes/`. Apply with:

```bash
cp ~/.config/novashell/themes/catppuccin-glass.css ~/.config/novashell/style.css
nova reload-css
```

Included themes:
- `ghost.css` — fully transparent, widgets float as text/glows
- `catppuccin-glass.css` — frosted glass cards, Catppuccin Mocha palette
- `tokyo-night.css` — deep blue/purple Tokyo Night
- `rose-pine.css` — warm rose/gold/iris
- `dracula-neon.css` — high-contrast neon Dracula
- `nord-ice.css` — cool grey-blue Nord
- `gruvbox-warm.css` — warm retro Gruvbox

## Custom Widgets

Drop a `.widget` file into `~/.config/novashell/widgets/` and reference it by name in config:

```yaml
# ~/.config/novashell/widgets/uptime.widget
name: uptime
template: |
  <box class="nova-exec">
    <label class="nova-exec__label" text="{uptime}" />
  </box>
sources:
  uptime:
    type: shell
    command: "awk '{d=int($1/86400);h=int($1%86400/3600);m=int($1%3600/60); printf \"%dd %dh %dm\",d,h,m}' /proc/uptime"
    interval: 60
```

Included custom widgets: `workspace`, `weather`, `network_ssid`, `pacman_updates`, `gpu_usage`, `uptime`, `kernel`.

## CSS Classes

Every widget exposes CSS classes for styling:

| widget | classes |
|--------|---------|
| clock | `.nova-clock`, `__time`, `__date`, `__day` |
| sysmon | `.nova-sysmon`, `__title`, `__row`, `__label`, `__bar`, `__bar--cpu`, `__bar--ram`, `__bar--swap`, `__value` |
| cava | `.nova-cava`, `__canvas` |
| volume | `.nova-volume`, `__icon`, `__icon-btn`, `__slider`, `__label` |
| media | `.nova-media`, `__player`, `__art`, `__title`, `__artist`, `__album`, `__time`, `__duration`, `__btn`, `__btn--playpause`, `__btn--active`, `__progress` |
| battery | `.nova-battery`, `__icon`, `__percent`, `__status`, `__bar` + `.warning`, `.critical` |
| launcher | `.nova-launcher`, `__btn` |
| exec | `.nova-exec`, `__label` |

## Cava Setup

Nova auto-spawns cava if `~/.config/novashell/cava.conf` exists. Example config:

```ini
[general]
bars = 32
framerate = 60

[output]
method = raw
raw_target = /tmp/nova_cava
data_format = binary
bit_format = 8bit
```

Set `pipe` in the cava widget vars to match `raw_target`.

## Autostart

```ini
# ~/.config/hypr/hyprland.conf
exec-once = nova daemon
```

Or with systemd:
```bash
cp nova.service ~/.config/systemd/user/
systemctl --user enable --now nova
```

## License

MIT
