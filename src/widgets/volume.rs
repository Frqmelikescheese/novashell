use crate::widgets::traits::{NovaWidget, WidgetContext, WidgetEvent};
use gtk4::prelude::*;
use gtk4::{Button, Label, Orientation, Scale, Widget};
use tracing::{debug, warn};

/// Read volume via wpctl (PipeWire). Returns (fraction 0.0–1.0, is_muted).
pub fn read_volume_wpctl() -> Option<(f64, bool)> {
    let output = std::process::Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Format: "Volume: 0.75" or "Volume: 0.75 [MUTED]"
    let line = stdout.trim();
    let muted = line.contains("[MUTED]");

    let fraction: f64 = line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);

    Some((fraction.clamp(0.0, 1.5), muted))
}

/// Fallback: read volume via amixer
pub fn read_volume_amixer() -> Option<(f64, bool)> {
    let output = std::process::Command::new("amixer")
        .args(["get", "Master"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse lines like: Playback 0 - 65536 [100%] [on]
    for line in stdout.lines() {
        if line.contains('%') {
            let muted = line.contains("[off]");
            // Extract percentage between [ and %]
            if let Some(start) = line.find('[') {
                if let Some(end) = line.find('%') {
                    if end > start {
                        let pct_str = &line[start + 1..end];
                        if let Ok(pct) = pct_str.parse::<f64>() {
                            return Some((pct / 100.0, muted));
                        }
                    }
                }
            }
        }
    }

    None
}

/// Read volume from best available source
pub fn read_volume() -> (f64, bool) {
    if let Some(v) = read_volume_wpctl() {
        return v;
    }
    if let Some(v) = read_volume_amixer() {
        return v;
    }
    warn!("VolumeWidget: cannot read volume (no wpctl or amixer)");
    (1.0, false)
}

/// Returns volume fraction 0.0–1.0
pub fn get_fraction() -> f64 {
    read_volume().0.min(1.0)
}

/// Returns volume as integer percentage string
pub fn get_percent() -> String {
    let (frac, _) = read_volume();
    format!("{:.0}", (frac * 100.0).min(100.0))
}

/// Returns the appropriate volume icon based on level and mute state
pub fn volume_icon_text(frac: f64, muted: bool) -> &'static str {
    if muted || frac == 0.0 {
        return "󰝟";
    }
    match (frac * 100.0) as u32 {
        1..=33 => "󰕿",
        34..=66 => "󰖀",
        _ => "󰕾",
    }
}

/// Returns the appropriate volume icon name (for GTK image)
pub fn get_icon() -> String {
    let (frac, muted) = read_volume();
    if muted {
        return "audio-volume-muted".to_string();
    }
    match (frac * 100.0) as u32 {
        0 => "audio-volume-muted".to_string(),
        1..=33 => "audio-volume-low".to_string(),
        34..=66 => "audio-volume-medium".to_string(),
        _ => "audio-volume-high".to_string(),
    }
}

/// Toggle mute using wpctl
fn toggle_mute() {
    let _ = std::process::Command::new("wpctl")
        .args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"])
        .output();
}

/// Set volume using wpctl (fraction 0.0–1.5)
fn set_volume_wpctl(fraction: f64) {
    let pct = format!("{:.2}", fraction.clamp(0.0, 1.5));
    let _ = std::process::Command::new("wpctl")
        .args(["set-volume", "@DEFAULT_AUDIO_SINK@", &pct])
        .output();
}

/// Built-in volume control widget
pub struct VolumeWidget;

impl VolumeWidget {
    pub fn new() -> Self {
        Self
    }
}

impl Default for VolumeWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl NovaWidget for VolumeWidget {
    fn name(&self) -> &str {
        "volume"
    }

    fn build(&self, ctx: &WidgetContext) -> Widget {
        // --- vars ---
        let show_icon: bool = ctx.var("show_icon", "true").parse().unwrap_or(true);
        let show_label: bool = ctx.var("show_label", "true").parse().unwrap_or(true);
        let show_slider: bool = ctx.var("show_slider", "true").parse().unwrap_or(true);
        let use_text_icon: bool = ctx.var("text_icon", "false").parse().unwrap_or(false);
        let max_volume: f64 = ctx.var("max_volume", "1.0").parse().unwrap_or(1.0);
        let mute_icon = ctx.var("mute_icon", "󰝟");
        let update_ms: u64 = ctx.var("update_interval", "1000").parse().unwrap_or(1000);

        let container = gtk4::Box::new(Orientation::Horizontal, 8);
        container.add_css_class("nova-volume");

        let (frac, muted) = read_volume();

        // Icon: either a text label (Nerd Font glyph) or GTK symbolic icon button
        let icon_btn = if use_text_icon {
            let icon_text = if muted { mute_icon.clone() } else { volume_icon_text(frac, muted).to_string() };
            let lbl = Label::new(Some(&icon_text));
            lbl.add_css_class("nova-volume__icon");
            // Wrap in a button so clicks still toggle mute
            let btn = Button::new();
            btn.add_css_class("nova-volume__icon-btn");
            btn.set_child(Some(&lbl));
            btn
        } else {
            let btn = Button::from_icon_name(&get_icon());
            btn.add_css_class("nova-volume__icon");
            btn
        };
        icon_btn.set_visible(show_icon);

        let slider = Scale::with_range(Orientation::Horizontal, 0.0, max_volume.max(1.0), 0.01);
        slider.add_css_class("nova-volume__slider");
        slider.set_draw_value(false);
        slider.set_value(frac.min(max_volume));
        slider.set_hexpand(true);
        slider.set_visible(show_slider);

        let pct_label = Label::new(Some(&format!("{:.0}%", frac * 100.0)));
        pct_label.add_css_class("nova-volume__label");
        pct_label.set_visible(show_label);

        // Mute toggle on icon click
        {
            let icon_btn_clone = icon_btn.clone();
            let pct_clone = pct_label.clone();
            let slider_clone = slider.clone();
            let mute_icon_c = mute_icon.clone();
            icon_btn.connect_clicked(move |_| {
                toggle_mute();
                let (f, muted) = read_volume();
                if use_text_icon {
                    if let Some(child) = icon_btn_clone.child() {
                        if let Some(lbl) = child.downcast_ref::<Label>() {
                            let txt = if muted { mute_icon_c.clone() } else { volume_icon_text(f, muted).to_string() };
                            lbl.set_text(&txt);
                        }
                    }
                } else {
                    icon_btn_clone.set_icon_name(&get_icon());
                }
                slider_clone.set_value(f.min(max_volume));
                pct_clone.set_text(&format!("{:.0}%", f * 100.0));
            });
        }

        // Slider change: set volume
        {
            let pct_clone2 = pct_label.clone();
            let icon_btn_clone2 = icon_btn.clone();
            let mute_icon_c2 = mute_icon.clone();
            slider.connect_value_changed(move |s| {
                let v = s.value();
                set_volume_wpctl(v);
                pct_clone2.set_text(&format!("{:.0}%", v * 100.0));
                let (f, muted) = read_volume();
                if use_text_icon {
                    if let Some(child) = icon_btn_clone2.child() {
                        if let Some(lbl) = child.downcast_ref::<Label>() {
                            let txt = if muted { mute_icon_c2.clone() } else { volume_icon_text(f, muted).to_string() };
                            lbl.set_text(&txt);
                        }
                    }
                } else {
                    icon_btn_clone2.set_icon_name(&get_icon());
                }
            });
        }

        container.append(&icon_btn);
        container.append(&slider);
        container.append(&pct_label);

        // Periodic update
        let icon_btn_upd = icon_btn.clone();
        let slider_upd = slider.clone();
        let pct_upd = pct_label.clone();
        let mute_icon_upd = mute_icon.clone();

        glib::timeout_add_local(std::time::Duration::from_millis(update_ms), move || {
            let (f, muted) = read_volume();
            if use_text_icon {
                if let Some(child) = icon_btn_upd.child() {
                    if let Some(lbl) = child.downcast_ref::<Label>() {
                        let txt = if muted { mute_icon_upd.clone() } else { volume_icon_text(f, muted).to_string() };
                        lbl.set_text(&txt);
                    }
                }
            } else {
                icon_btn_upd.set_icon_name(&get_icon());
            }
            slider_upd.set_value(f.min(max_volume));
            pct_upd.set_text(&format!("{:.0}%", f * 100.0));
            glib::ControlFlow::Continue
        });

        debug!("VolumeWidget: built");
        container.upcast()
    }

    fn update(&self, _widget: &Widget, _ctx: &WidgetContext) {
        // Timer handles updates
    }

    fn on_event(&self, event: &WidgetEvent, _widget: &Widget) {
        match event {
            WidgetEvent::ButtonClick { action } => {
                if action == "volume::mute_toggle" {
                    toggle_mute();
                }
            }
            WidgetEvent::SliderChange { action, value } => {
                if action == "volume::set" {
                    set_volume_wpctl(*value);
                }
            }
            _ => {}
        }
    }
}
