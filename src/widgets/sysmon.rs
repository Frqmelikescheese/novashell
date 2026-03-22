use crate::widgets::traits::{NovaWidget, WidgetContext, WidgetEvent};
use gtk4::prelude::*;
use gtk4::{Label, LevelBar, Orientation, Widget};
use parking_lot::Mutex;
use std::sync::Arc;
use sysinfo::{Networks, System};
use tracing::debug;

/// Shared system info state
struct SysState {
    system: System,
    networks: Networks,
    prev_rx: u64,
    prev_tx: u64,
}

impl SysState {
    fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        let networks = Networks::new_with_refreshed_list();
        Self {
            system,
            networks,
            prev_rx: 0,
            prev_tx: 0,
        }
    }

    fn refresh(&mut self) {
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.networks.refresh();
    }

    fn cpu_percent(&self) -> f32 {
        let cpus = self.system.cpus();
        if cpus.is_empty() {
            return 0.0;
        }
        cpus.iter().map(|c| c.cpu_usage()).sum::<f32>() / cpus.len() as f32
    }

    fn ram_used_bytes(&self) -> u64 {
        self.system.used_memory()
    }

    fn ram_total_bytes(&self) -> u64 {
        self.system.total_memory()
    }

    fn net_rx_bytes(&self) -> u64 {
        self.networks.iter().map(|(_, n)| n.received()).sum()
    }

    fn net_tx_bytes(&self) -> u64 {
        self.networks.iter().map(|(_, n)| n.transmitted()).sum()
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B/s")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB/s", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB/s", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB/s", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn format_memory(bytes: u64) -> String {
    if bytes < 1024 * 1024 * 1024 {
        format!("{:.0} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Built-in system monitor widget showing CPU, RAM, and network stats
pub struct SysmonWidget {
    state: Arc<Mutex<SysState>>,
}

impl SysmonWidget {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(SysState::new())),
        }
    }
}

impl Default for SysmonWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl NovaWidget for SysmonWidget {
    fn name(&self) -> &str {
        "sysmon"
    }

    fn build(&self, _ctx: &WidgetContext) -> Widget {
        let container = gtk4::Box::new(Orientation::Vertical, 6);
        container.add_css_class("nova-sysmon");

        let title = Label::new(Some("System"));
        title.add_css_class("nova-sysmon__title");
        title.set_halign(gtk4::Align::Start);
        container.append(&title);

        // CPU row
        let cpu_row = gtk4::Box::new(Orientation::Horizontal, 8);
        cpu_row.add_css_class("nova-sysmon__row");
        let cpu_label = Label::new(Some("CPU"));
        cpu_label.add_css_class("nova-sysmon__label");
        let cpu_bar = LevelBar::new();
        cpu_bar.add_css_class("nova-sysmon__bar");
        cpu_bar.add_css_class("nova-sysmon__bar--cpu");
        cpu_bar.set_hexpand(true);
        cpu_bar.set_min_value(0.0);
        cpu_bar.set_max_value(1.0);
        let cpu_val = Label::new(Some("0%"));
        cpu_val.add_css_class("nova-sysmon__value");
        cpu_row.append(&cpu_label);
        cpu_row.append(&cpu_bar);
        cpu_row.append(&cpu_val);
        container.append(&cpu_row);

        // RAM row
        let ram_row = gtk4::Box::new(Orientation::Horizontal, 8);
        ram_row.add_css_class("nova-sysmon__row");
        let ram_label = Label::new(Some("RAM"));
        ram_label.add_css_class("nova-sysmon__label");
        let ram_bar = LevelBar::new();
        ram_bar.add_css_class("nova-sysmon__bar");
        ram_bar.add_css_class("nova-sysmon__bar--ram");
        ram_bar.set_hexpand(true);
        ram_bar.set_min_value(0.0);
        ram_bar.set_max_value(1.0);
        let ram_val = Label::new(Some("0 MB"));
        ram_val.add_css_class("nova-sysmon__value");
        ram_row.append(&ram_label);
        ram_row.append(&ram_bar);
        ram_row.append(&ram_val);
        container.append(&ram_row);

        // Net TX row
        let net_tx_row = gtk4::Box::new(Orientation::Horizontal, 8);
        net_tx_row.add_css_class("nova-sysmon__row");
        let net_tx_label = Label::new(Some("NET↑"));
        net_tx_label.add_css_class("nova-sysmon__label");
        let net_tx_val = Label::new(Some("0 B/s"));
        net_tx_val.add_css_class("nova-sysmon__value");
        net_tx_row.append(&net_tx_label);
        net_tx_row.append(&net_tx_val);
        container.append(&net_tx_row);

        // Net RX row
        let net_rx_row = gtk4::Box::new(Orientation::Horizontal, 8);
        net_rx_row.add_css_class("nova-sysmon__row");
        let net_rx_label = Label::new(Some("NET↓"));
        net_rx_label.add_css_class("nova-sysmon__label");
        let net_rx_val = Label::new(Some("0 B/s"));
        net_rx_val.add_css_class("nova-sysmon__value");
        net_rx_row.append(&net_rx_label);
        net_rx_row.append(&net_rx_val);
        container.append(&net_rx_row);

        let state = self.state.clone();

        // Update timer every 2 seconds
        glib::timeout_add_local(std::time::Duration::from_secs(2), move || {
            let mut s = state.lock();
            let prev_rx = s.net_rx_bytes();
            let prev_tx = s.net_tx_bytes();
            s.refresh();

            let cpu = s.cpu_percent();
            let ram_used = s.ram_used_bytes();
            let ram_total = s.ram_total_bytes();
            let ram_frac = if ram_total > 0 {
                ram_used as f64 / ram_total as f64
            } else {
                0.0
            };
            let rx_delta = s.net_rx_bytes().saturating_sub(prev_rx) / 2;
            let tx_delta = s.net_tx_bytes().saturating_sub(prev_tx) / 2;

            cpu_bar.set_value(cpu as f64 / 100.0);
            cpu_val.set_text(&format!("{:.0}%", cpu));
            ram_bar.set_value(ram_frac);
            ram_val.set_text(&format_memory(ram_used));
            net_tx_val.set_text(&format_bytes(tx_delta));
            net_rx_val.set_text(&format_bytes(rx_delta));

            glib::ControlFlow::Continue
        });

        debug!("SysmonWidget: built");
        container.upcast()
    }

    fn update(&self, _widget: &Widget, _ctx: &WidgetContext) {
        // Timer handles live updates
    }

    fn on_event(&self, _event: &WidgetEvent, _widget: &Widget) {
        // No interactive elements
    }
}

/// Collect CPU usage percentage from the system
pub fn get_cpu_percent() -> f32 {
    let mut sys = System::new();
    sys.refresh_cpu_usage();
    // Need a small delay for meaningful measurement
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    sys.refresh_cpu_usage();
    let cpus = sys.cpus();
    if cpus.is_empty() {
        return 0.0;
    }
    cpus.iter().map(|c| c.cpu_usage()).sum::<f32>() / cpus.len() as f32
}

/// Collect RAM usage fraction (0.0–1.0)
pub fn get_ram_fraction() -> f64 {
    let mut sys = System::new();
    sys.refresh_memory();
    let total = sys.total_memory();
    if total == 0 {
        return 0.0;
    }
    sys.used_memory() as f64 / total as f64
}

/// Collect RAM used as human-readable string
pub fn get_ram_used() -> String {
    let mut sys = System::new();
    sys.refresh_memory();
    format_memory(sys.used_memory())
}

/// Collect network receive rate (rough instantaneous value)
pub fn get_net_rx() -> String {
    let networks = Networks::new_with_refreshed_list();
    let total: u64 = networks.iter().map(|(_, n)| n.received()).sum();
    format_bytes(total)
}

/// Collect network transmit rate (rough instantaneous value)
pub fn get_net_tx() -> String {
    let networks = Networks::new_with_refreshed_list();
    let total: u64 = networks.iter().map(|(_, n)| n.transmitted()).sum();
    format_bytes(total)
}
