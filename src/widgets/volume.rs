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

/// Returns the appropriate volume icon name
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

/// Set volume using wpctl (fraction 0.0–1.0)
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

    fn build(&self, _ctx: &WidgetContext) -> Widget {
        let container = gtk4::Box::new(Orientation::Horizontal, 8);
        container.add_css_class("nova-volume");

        let (frac, _muted) = read_volume();
        let icon_name = get_icon();

        let icon_btn = Button::from_icon_name(&icon_name);
        icon_btn.add_css_class("nova-volume__icon");

        let slider = Scale::with_range(Orientation::Horizontal, 0.0, 1.0, 0.01);
        slider.add_css_class("nova-volume__slider");
        slider.set_draw_value(false);
        slider.set_value(frac.min(1.0));
        slider.set_hexpand(true);

        let pct_label = Label::new(Some(&format!("{}%", get_percent())));
        pct_label.add_css_class("nova-volume__label");

        // Mute toggle
        {
            let icon_btn_clone = icon_btn.clone();
            let pct_clone = pct_label.clone();
            let slider_clone = slider.clone();
            icon_btn.connect_clicked(move |_| {
                toggle_mute();
                let (f, _) = read_volume();
                let ic = get_icon();
                icon_btn_clone.set_icon_name(&ic);
                slider_clone.set_value(f.min(1.0));
                pct_clone.set_text(&format!("{}%", get_percent()));
            });
        }

        // Slider change: set volume
        {
            let pct_clone2 = pct_label.clone();
            let icon_btn_clone2 = icon_btn.clone();
            slider.connect_value_changed(move |s| {
                let v = s.value();
                set_volume_wpctl(v);
                pct_clone2.set_text(&format!("{:.0}%", v * 100.0));
                icon_btn_clone2.set_icon_name(&get_icon());
            });
        }

        container.append(&icon_btn);
        container.append(&slider);
        container.append(&pct_label);

        // Periodic update every second
        let icon_btn_upd = icon_btn.clone();
        let slider_upd = slider.clone();
        let pct_upd = pct_label.clone();

        glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
            let (f, _) = read_volume();
            icon_btn_upd.set_icon_name(&get_icon());
            // Only update slider if it's not being dragged (simple approach)
            slider_upd.set_value(f.min(1.0));
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
