use crate::config::schema::{AnchorPoint, LayerType, ScreenConfig};
use crate::renderer::window::NovaWindow;
use gdk4::prelude::*;
use gtk4::prelude::*;
use gtk4::Application;
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use tracing::{debug, info, warn};

/// Create a layer shell window for a screen configuration.
///
/// Returns a `NovaWindow` with the layer shell protocol configured
/// (anchors, margins, exclusive zone, layer).
pub fn create_layer_window(
    app: &Application,
    screen_name: &str,
    config: &ScreenConfig,
) -> NovaWindow {
    let window = gtk4::ApplicationWindow::new(app);
    window.set_decorated(false);
    window.set_resizable(false);

    // Initialize the layer shell extension
    window.init_layer_shell();

    // Set the Wayland layer
    window.set_layer(map_layer_type(&config.layer));

    // Set anchor edges based on config
    set_anchors_from_config(&window, config);

    // Exclusive zone
    window.set_exclusive_zone(config.exclusive_zone);

    info!(
        "LayerShell: screen '{}' → monitor='{}' layer={:?}",
        screen_name, config.monitor, config.layer
    );

    // Target the correct physical monitor by connector name
    if let Some(monitor) = find_monitor_by_name(&config.monitor) {
        window.set_monitor(Some(&monitor));
        debug!("LayerShell: pinned '{}' to monitor '{}'", screen_name, config.monitor);
    } else {
        warn!("LayerShell: monitor '{}' not found, using default", config.monitor);
    }

    let nova_window = NovaWindow::new(window, screen_name);

    debug!("LayerShell: created window for screen '{screen_name}'");
    nova_window
}

/// Map our `LayerType` to `gtk4_layer_shell::Layer` (public for renderer)
pub fn map_layer_type_pub(layer: &LayerType) -> Layer {
    map_layer_type(layer)
}

/// Map our `LayerType` to `gtk4_layer_shell::Layer`
fn map_layer_type(layer: &LayerType) -> Layer {
    match layer {
        LayerType::Background => Layer::Background,
        LayerType::Bottom => Layer::Bottom,
        LayerType::Top => Layer::Top,
        LayerType::Overlay => Layer::Overlay,
    }
}

/// Configure anchor edges and margins for a window from ScreenConfig.
///
/// Widget instances each define their own anchor; for the window itself we
/// set it to cover the full screen (all four edges anchored) so individual
/// widgets can be positioned freely via the layer shell margins.
fn set_anchors_from_config(window: &gtk4::ApplicationWindow, config: &ScreenConfig) {
    // Determine dominant anchor from the first widget, or default to full-screen
    let anchor = config
        .widgets
        .first()
        .map(|w| w.position.anchor)
        .unwrap_or(AnchorPoint::TopLeft);

    // Anchor all edges for a full-screen overlay window
    window.set_anchor(Edge::Top, true);
    window.set_anchor(Edge::Bottom, true);
    window.set_anchor(Edge::Left, true);
    window.set_anchor(Edge::Right, true);

    // Set margins from the first widget's position as a starting point
    // (individual widget margins handled by their position widgets)
    if let Some(first) = config.widgets.first() {
        let margin = first.position.margin;
        window.set_margin(Edge::Top, margin[0]);
        window.set_margin(Edge::Right, margin[1]);
        window.set_margin(Edge::Bottom, margin[2]);
        window.set_margin(Edge::Left, margin[3]);
    }
}

/// Find a GDK monitor by its connector name (e.g. "DP-1", "HDMI-A-1", "eDP-1").
pub fn find_monitor_by_name(connector: &str) -> Option<gdk4::Monitor> {
    let display = gdk4::Display::default()?;
    let monitors = display.monitors();
    let n = monitors.n_items();
    debug!("find_monitor_by_name: looking for '{}', {} monitor(s) available", connector, n);
    for i in 0..n {
        if let Some(obj) = monitors.item(i) {
            if let Ok(mon) = obj.downcast::<gdk4::Monitor>() {
                let conn = mon.connector().map(|c| c.to_string()).unwrap_or_else(|| "<none>".into());
                let mfr = mon.manufacturer().map(|c| c.to_string()).unwrap_or_default();
                let model = mon.model().map(|c| c.to_string()).unwrap_or_default();
                debug!("  monitor[{}]: connector='{}' manufacturer='{}' model='{}'", i, conn, mfr, model);
                if conn == connector {
                    debug!("find_monitor_by_name: matched '{}' at index {}", connector, i);
                    return Some(mon);
                }
            }
        }
    }
    warn!("find_monitor_by_name: connector '{}' not found in {} monitor(s)", connector, n);
    None
}

/// Apply anchor + margin settings to a layer window for a specific widget.
///
/// This is used when creating per-widget sub-windows (not the default full-screen approach).
pub fn configure_layer_for_anchor(
    window: &gtk4::ApplicationWindow,
    anchor: AnchorPoint,
    margin: [i32; 4],
) {
    // Reset all anchors first
    window.set_anchor(Edge::Top, false);
    window.set_anchor(Edge::Bottom, false);
    window.set_anchor(Edge::Left, false);
    window.set_anchor(Edge::Right, false);

    // Set anchors based on AnchorPoint
    if anchor.has_top() {
        window.set_anchor(Edge::Top, true);
    }
    if anchor.has_bottom() {
        window.set_anchor(Edge::Bottom, true);
    }
    if anchor.has_left() {
        window.set_anchor(Edge::Left, true);
    }
    if anchor.has_right() {
        window.set_anchor(Edge::Right, true);
    }

    // margin: [top, right, bottom, left]
    window.set_margin(Edge::Top, margin[0]);
    window.set_margin(Edge::Right, margin[1]);
    window.set_margin(Edge::Bottom, margin[2]);
    window.set_margin(Edge::Left, margin[3]);
}
