use crate::widgets::traits::{NovaWidget, WidgetContext, WidgetEvent};
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{DrawingArea, Orientation, Widget};
use parking_lot::Mutex;
use std::io::Read;
use std::sync::Arc;
use tracing::{debug, warn};

/// Shared bar data updated from cava pipe
#[derive(Default, Clone)]
struct CavaState {
    bars: Vec<f64>,
    color_r: f64,
    color_g: f64,
    color_b: f64,
    color2_r: f64,
    color2_g: f64,
    color2_b: f64,
    has_gradient: bool,
    bar_count: usize,
    gap: f64,
    mirror: bool,
    rounded: bool,
}

impl CavaState {
    fn new(bar_count: usize, color: &str, gap: f64, gradient_end: Option<&str>, mirror: bool, rounded: bool) -> Self {
        let (r, g, b) = parse_color(color);
        let (r2, g2, b2, has_gradient) = if let Some(c2) = gradient_end {
            let (r2, g2, b2) = parse_color(c2);
            (r2, g2, b2, true)
        } else {
            (r, g, b, false)
        };
        Self {
            bars: vec![0.0; bar_count],
            color_r: r,
            color_g: g,
            color_b: b,
            color2_r: r2,
            color2_g: g2,
            color2_b: b2,
            has_gradient,
            bar_count,
            gap,
            mirror,
            rounded,
        }
    }
}

/// Parse a hex color string like "#89b4fa" into (r, g, b) in 0.0–1.0 range
fn parse_color(color: &str) -> (f64, f64, f64) {
    let hex = color.trim_start_matches('#');
    if hex.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        ) {
            return (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
        }
    }
    // fallback: cornflower blue
    (0.537, 0.706, 0.980)
}

/// Cava audio visualizer widget.
///
/// Reads raw 8-bit binary frames from cava's output pipe.
///
/// Vars:
///   bar_count      – number of bars (default 20)
///   color_active   – bar color hex (default #89b4fa)
///   color          – alias for color_active
///   gradient_end   – second color for bottom-to-top gradient (optional)
///   gap            – pixels between bars (default 2)
///   width          – canvas width px (default 280)
///   height         – canvas height px (default 80)
///   pipe           – path to cava raw pipe (default /tmp/cava_input)
///   mirror         – true/false, reflect bars symmetrically (default false)
///   bar_style      – "sharp" or "rounded" (default sharp)
///   opacity        – base opacity 0.0–1.0 (default 1.0)
pub struct CavaWidget {
    state: Arc<Mutex<CavaState>>,
}

impl CavaWidget {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(CavaState::default())),
        }
    }
}

impl Default for CavaWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl NovaWidget for CavaWidget {
    fn name(&self) -> &str {
        "cava"
    }

    fn build(&self, ctx: &WidgetContext) -> Widget {
        let bar_count: usize = ctx.var("bar_count", "20").parse().unwrap_or(20);
        // Accept both "color" and "color_active" for compat
        let color = if ctx.vars.contains_key("color_active") {
            ctx.var("color_active", "#89b4fa")
        } else {
            ctx.var("color", "#89b4fa")
        };
        let gradient_end = if ctx.vars.contains_key("gradient_end") {
            Some(ctx.var("gradient_end", ""))
        } else {
            None
        };
        let gap: f64 = ctx.var("gap", "2").parse().unwrap_or(2.0);
        let width: i32 = ctx.var("width", "280").parse().unwrap_or(280);
        let height: i32 = ctx.var("height", "80").parse().unwrap_or(80);
        let pipe_path = ctx.var("pipe", "/tmp/cava_input");
        let mirror: bool = ctx.var("mirror", "false").parse().unwrap_or(false);
        let bar_style = ctx.var("bar_style", "sharp");
        let rounded = bar_style == "rounded";
        let opacity: f64 = ctx.var("opacity", "1.0").parse().unwrap_or(1.0);

        {
            let mut s = self.state.lock();
            *s = CavaState::new(bar_count, &color, gap, gradient_end.as_deref(), mirror, rounded);
        }

        let container = gtk4::Box::new(Orientation::Vertical, 0);
        container.add_css_class("nova-cava");

        let drawing = DrawingArea::new();
        drawing.add_css_class("nova-cava__canvas");
        drawing.set_content_width(width);
        drawing.set_content_height(height);

        let draw_state = self.state.clone();
        drawing.set_draw_func(move |_area, cr, w, h| {
            let s = draw_state.lock();
            let w = w as f64;
            let h = h as f64;

            // Clear background
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
            let _ = cr.paint();

            if s.bars.is_empty() {
                return;
            }

            let bars = if s.mirror {
                // Mirror: left half is original reversed, right half is original
                let half: Vec<f64> = s.bars.iter().copied().collect();
                let mut mirrored: Vec<f64> = half.iter().rev().cloned().collect();
                mirrored.extend_from_slice(&half);
                mirrored
            } else {
                s.bars.clone()
            };

            let n = bars.len() as f64;
            let bar_w = (w - s.gap * (n - 1.0)) / n;
            if bar_w <= 0.0 {
                return;
            }

            let radius = if s.rounded { (bar_w / 2.0).min(4.0) } else { 0.0 };

            for (i, &val) in bars.iter().enumerate() {
                let norm = (val / 1000.0).clamp(0.0, 1.0);
                let bar_h = norm * h;
                let x = i as f64 * (bar_w + s.gap);
                let y = h - bar_h;

                if bar_h < 1.0 {
                    continue;
                }

                let alpha = (opacity * (0.6 + 0.4 * norm)).clamp(0.0, 1.0);

                if s.has_gradient {
                    // Vertical gradient: color at top, color2 at bottom
                    use cairo::LinearGradient;
                    let grad = LinearGradient::new(x, y, x, h);
                    grad.add_color_stop_rgba(0.0, s.color_r, s.color_g, s.color_b, alpha);
                    grad.add_color_stop_rgba(1.0, s.color2_r, s.color2_g, s.color2_b, alpha * 0.5);
                    let _ = cr.set_source(grad);
                } else {
                    cr.set_source_rgba(s.color_r, s.color_g, s.color_b, alpha);
                }

                if radius > 0.0 {
                    rounded_rect(cr, x, y, bar_w, bar_h, radius);
                } else {
                    cr.rectangle(x, y, bar_w, bar_h);
                }
                let _ = cr.fill();
            }
        });

        container.append(&drawing);

        // Background thread reads from cava pipe
        let state_clone = self.state.clone();
        let weak_drawing = glib::SendWeakRef::from(drawing.downgrade());

        std::thread::spawn(move || {
            read_cava_pipe(&pipe_path, state_clone, weak_drawing);
        });

        debug!("CavaWidget: built");
        container.upcast()
    }

    fn update(&self, widget: &Widget, ctx: &WidgetContext) {
        let color = if ctx.vars.contains_key("color_active") {
            ctx.var("color_active", "#89b4fa")
        } else {
            ctx.var("color", "#89b4fa")
        };
        let gap: f64 = ctx.var("gap", "2").parse().unwrap_or(2.0);
        let mirror: bool = ctx.var("mirror", "false").parse().unwrap_or(false);
        let (r, g, b) = parse_color(&color);
        {
            let mut s = self.state.lock();
            s.color_r = r;
            s.color_g = g;
            s.color_b = b;
            s.gap = gap;
            s.mirror = mirror;
        }
        if let Some(container) = widget.downcast_ref::<gtk4::Box>() {
            if let Some(child) = container.first_child() {
                child.queue_draw();
            }
        }
    }

    fn on_event(&self, _event: &WidgetEvent, _widget: &Widget) {
        // No interactive elements
    }
}

/// Draw a rounded rectangle path
fn rounded_rect(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0);
    cr.new_sub_path();
    cr.arc(x + r, y + r, r, std::f64::consts::PI, 3.0 * std::f64::consts::PI / 2.0);
    cr.arc(x + w - r, y + r, r, 3.0 * std::f64::consts::PI / 2.0, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, std::f64::consts::PI / 2.0);
    cr.arc(x + r, y + h - r, r, std::f64::consts::PI / 2.0, std::f64::consts::PI);
    cr.close_path();
}

/// Background function: opens the cava pipe, reads raw 8-bit frames, updates state.
fn read_cava_pipe(
    pipe_path: &str,
    state: Arc<Mutex<CavaState>>,
    drawing: glib::SendWeakRef<DrawingArea>,
) {
    loop {
        let bar_count = state.lock().bar_count;

        match std::fs::File::open(pipe_path) {
            Ok(mut file) => {
                let mut buf = vec![0u8; bar_count];
                loop {
                    match file.read_exact(&mut buf) {
                        Ok(()) => {
                            let values: Vec<f64> = buf.iter()
                                .map(|&b| b as f64 / 255.0 * 1000.0)
                                .collect();

                            {
                                let mut s = state.lock();
                                s.bars = values;
                            }

                            let d = drawing.clone();
                            glib::idle_add_once(move || {
                                if let Some(widget) = d.upgrade() {
                                    widget.queue_draw();
                                }
                            });
                        }
                        Err(_) => break,
                    }
                }
            }
            Err(_) => {
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        }
    }
}
