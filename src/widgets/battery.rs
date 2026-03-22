use crate::widgets::traits::{NovaWidget, WidgetContext, WidgetEvent};
use gtk4::prelude::*;
use gtk4::{Label, LevelBar, Orientation, Widget};
use std::fs;
use tracing::{debug, warn};

const BAT_PATH: &str = "/sys/class/power_supply/BAT0";
const BAT1_PATH: &str = "/sys/class/power_supply/BAT1";

/// Read a sysfs attribute file, trimming whitespace
fn read_sysfs(path: &str) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
}

/// Find the first available battery path
fn find_battery_path() -> Option<&'static str> {
    if std::path::Path::new(BAT_PATH).exists() {
        Some(BAT_PATH)
    } else if std::path::Path::new(BAT1_PATH).exists() {
        Some(BAT1_PATH)
    } else {
        None
    }
}

/// Returns (capacity_percent, status_string)
pub fn read_battery() -> (u32, String) {
    let bat_path = match find_battery_path() {
        Some(p) => p,
        None => {
            warn!("BatteryWidget: no battery found in /sys/class/power_supply/");
            return (100, "Unknown".to_string());
        }
    };

    let capacity: u32 = read_sysfs(&format!("{bat_path}/capacity"))
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

    let status = read_sysfs(&format!("{bat_path}/status"))
        .unwrap_or_else(|| "Unknown".to_string());

    (capacity, status)
}

/// Returns battery fraction 0.0–1.0
pub fn get_fraction() -> f64 {
    let (cap, _) = read_battery();
    cap as f64 / 100.0
}

/// Returns battery percentage as string
pub fn get_percent() -> String {
    let (cap, _) = read_battery();
    cap.to_string()
}

/// Returns battery status string
pub fn get_status() -> String {
    let (_, status) = read_battery();
    status
}

/// Returns a Unicode battery icon based on charge level and status
pub fn get_icon() -> String {
    let (cap, status) = read_battery();
    if status == "Charging" || status == "Full" {
        return "⚡".to_string();
    }
    match cap {
        90..=100 => "󰁹".to_string(),
        70..=89 => "󰂁".to_string(),
        50..=69 => "󰁾".to_string(),
        20..=49 => "󰁼".to_string(),
        10..=19 => "󰁻".to_string(),
        _ => "󰂎".to_string(),
    }
}

/// Determine CSS alert class based on thresholds
fn alert_class(cap: u32, warn_threshold: u32, crit_threshold: u32) -> Option<&'static str> {
    if cap <= crit_threshold {
        Some("critical")
    } else if cap <= warn_threshold {
        Some("warning")
    } else {
        None
    }
}

/// Built-in battery status widget
pub struct BatteryWidget;

impl BatteryWidget {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BatteryWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl NovaWidget for BatteryWidget {
    fn name(&self) -> &str {
        "battery"
    }

    fn build(&self, ctx: &WidgetContext) -> Widget {
        // --- vars ---
        let show_bar: bool = ctx.var("show_bar", "true").parse().unwrap_or(true);
        let show_status: bool = ctx.var("show_status", "true").parse().unwrap_or(true);
        let show_icon: bool = ctx.var("show_icon", "true").parse().unwrap_or(true);
        let warn_threshold: u32 = ctx.var("threshold_warning", "20").parse().unwrap_or(20);
        let crit_threshold: u32 = ctx.var("threshold_critical", "10").parse().unwrap_or(10);
        let update_secs: u64 = ctx.var("update_interval", "30").parse().unwrap_or(30);

        let container = gtk4::Box::new(Orientation::Horizontal, 8);
        container.add_css_class("nova-battery");

        let (cap, status) = read_battery();
        let frac = cap as f64 / 100.0;

        // Apply initial alert class
        if let Some(cls) = alert_class(cap, warn_threshold, crit_threshold) {
            container.add_css_class(cls);
        }

        let icon_label = Label::new(Some(&get_icon()));
        icon_label.add_css_class("nova-battery__icon");
        icon_label.set_visible(show_icon);

        let detail_box = gtk4::Box::new(Orientation::Vertical, 2);
        detail_box.add_css_class("nova-battery__detail");

        let percent_label = Label::new(Some(&format!("{cap}%")));
        percent_label.add_css_class("nova-battery__percent");
        percent_label.set_halign(gtk4::Align::Start);

        let status_label = Label::new(Some(&status));
        status_label.add_css_class("nova-battery__status");
        status_label.set_halign(gtk4::Align::Start);
        status_label.set_visible(show_status);

        detail_box.append(&percent_label);
        detail_box.append(&status_label);

        let bar = LevelBar::new();
        bar.add_css_class("nova-battery__bar");
        bar.set_orientation(Orientation::Vertical);
        bar.set_min_value(0.0);
        bar.set_max_value(1.0);
        bar.set_value(frac);
        bar.set_visible(show_bar);

        container.append(&icon_label);
        container.append(&detail_box);
        container.append(&bar);

        // Update timer
        let container_weak = container.downgrade();
        glib::timeout_add_local(std::time::Duration::from_secs(update_secs), move || {
            let (cap, status) = read_battery();
            let frac = cap as f64 / 100.0;
            icon_label.set_text(&get_icon());
            percent_label.set_text(&format!("{cap}%"));
            status_label.set_text(&status);
            bar.set_value(frac);

            // Update alert classes
            if let Some(cont) = container_weak.upgrade() {
                cont.remove_css_class("warning");
                cont.remove_css_class("critical");
                if let Some(cls) = alert_class(cap, warn_threshold, crit_threshold) {
                    cont.add_css_class(cls);
                }
            }

            glib::ControlFlow::Continue
        });

        debug!("BatteryWidget: built");
        container.upcast()
    }

    fn update(&self, _widget: &Widget, _ctx: &WidgetContext) {
        // Timer handles updates
    }

    fn on_event(&self, _event: &WidgetEvent, _widget: &Widget) {
        // No interactive elements
    }
}
