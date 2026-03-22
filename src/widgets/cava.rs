use crate::widgets::traits::{NovaWidget, WidgetContext, WidgetEvent};
use cairo::Context;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{DrawingArea, Orientation, Widget};
use parking_lot::Mutex;
use std::io::Read;
use std::sync::Arc;
use tracing::{debug, warn};

/// Shared bar data updated from cava pipe
#[derive(Default)]
struct CavaState {
    bars: Vec<f64>,
    color_r: f64,
    color_g: f64,
    color_b: f64,
    bar_count: usize,
    gap: f64,
}

impl CavaState {
    fn new(bar_count: usize, color: &str, gap: f64) -> Self {
        let (r, g, b) = parse_color(color);
        Self {
            bars: vec![0.0; bar_count],
            color_r: r,
            color_g: g,
            color_b: b,
            bar_count,
            gap,
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
/// Reads space-separated bar values (0–1000) from cava's raw output pipe at
/// `/tmp/cava_input` (or the path specified in the `pipe` var). Renders them
/// as a bar chart using Cairo.
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
        let container = gtk4::Box::new(Orientation::Vertical, 0);
        container.add_css_class("nova-cava");

        let bar_count: usize = ctx.var("bar_count", "20").parse().unwrap_or(20);
        // Accept both "color" and "color_active" (for Catppuccin config compat)
        let color = if ctx.vars.contains_key("color_active") {
            ctx.var("color_active", "#89b4fa")
        } else {
            ctx.var("color", "#89b4fa")
        };
        let gap: f64 = ctx.var("gap", "2").parse().unwrap_or(2.0);
        let width: i32 = ctx.var("width", "280").parse().unwrap_or(280);
        let height: i32 = ctx.var("height", "80").parse().unwrap_or(80);
        let pipe_path = ctx.var("pipe", "/tmp/cava_input");

        {
            let mut s = self.state.lock();
            *s = CavaState::new(bar_count, &color, gap);
        }

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

            let n = s.bars.len() as f64;
            let bar_w = (w - s.gap * (n - 1.0)) / n;
            if bar_w <= 0.0 {
                return;
            }

            for (i, &val) in s.bars.iter().enumerate() {
                let norm = (val / 1000.0).clamp(0.0, 1.0);
                let bar_h = norm * h;
                let x = i as f64 * (bar_w + s.gap);
                let y = h - bar_h;

                // Gradient: brighter at top
                let alpha = 0.6 + 0.4 * norm;
                cr.set_source_rgba(s.color_r, s.color_g, s.color_b, alpha);
                cr.rectangle(x, y, bar_w, bar_h);
                let _ = cr.fill();
            }
        });

        container.append(&drawing);

        // Start background thread to read from cava pipe.
        // Use SendWeakRef so we can cross the thread boundary safely.
        let state_clone = self.state.clone();
        let weak_drawing = glib::SendWeakRef::from(drawing.downgrade());

        std::thread::spawn(move || {
            read_cava_pipe(&pipe_path, state_clone, weak_drawing);
        });

        debug!("CavaWidget: built");
        container.upcast()
    }

    fn update(&self, widget: &Widget, ctx: &WidgetContext) {
        // Update color/gap from vars
        let color = ctx.var("color", "#89b4fa");
        let gap: f64 = ctx.var("gap", "2").parse().unwrap_or(2.0);
        let (r, g, b) = parse_color(&color);
        {
            let mut s = self.state.lock();
            s.color_r = r;
            s.color_g = g;
            s.color_b = b;
            s.gap = gap;
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

/// Background function: opens the cava pipe, reads raw 8-bit frames, updates state.
///
/// cava raw output with `bit_format = 8bit`: each frame is exactly `bar_count` bytes,
/// values in range 0–255. Frames are written continuously with no separator.
fn read_cava_pipe(
    pipe_path: &str,
    state: Arc<Mutex<CavaState>>,
    drawing: glib::SendWeakRef<DrawingArea>,
) {
    use std::io::Read;

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
