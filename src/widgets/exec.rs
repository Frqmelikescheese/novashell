use crate::widgets::traits::{NovaWidget, WidgetContext, WidgetEvent};
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Label, Orientation, Widget};
use tracing::{debug, warn};

/// A widget that runs a shell command and displays its output.
///
/// Vars:
///   command      — shell command to run (required)
///   interval     — seconds between refreshes (default: 5)
///   format       — output template, use {output} for command stdout (default: "{output}")
///   markup       — set to "true" to interpret Pango markup in output (default: false)
///   on_click     — shell command to run on left-click
///   on_right_click — shell command to run on right-click
///   align        — text alignment: left | center | right (default: left)
///
/// Example config:
///   - widget: exec
///     vars:
///       command: "~/.config/novashell/scripts/weather.sh"
///       interval: "300"
///       format: "🌤 {output}"
///       on_click: "xdg-open https://weather.com"
pub struct ExecWidget;

impl ExecWidget {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ExecWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl NovaWidget for ExecWidget {
    fn name(&self) -> &str {
        "exec"
    }

    fn build(&self, ctx: &WidgetContext) -> Widget {
        let command = ctx.var("command", "echo 'set command var'");
        let interval_secs: u64 = ctx.var("interval", "5").parse().unwrap_or(5);
        let format_str = ctx.var("format", "{output}");
        let use_markup = ctx.var("markup", "false") == "true";
        let on_click = ctx.vars.get("on_click").cloned();
        let on_right = ctx.vars.get("on_right_click").cloned();
        let align_str = ctx.var("align", "left");

        let label = Label::new(Some("…"));
        label.add_css_class("nova-exec__label");
        label.set_xalign(match align_str.as_str() {
            "center" => 0.5,
            "right" => 1.0,
            _ => 0.0,
        });
        label.set_wrap(false);
        label.set_ellipsize(gtk4::pango::EllipsizeMode::End);

        let container = gtk4::Box::new(Orientation::Vertical, 0);
        container.add_css_class("nova-exec");

        // Add click handler if configured
        if on_click.is_some() || on_right.is_some() {
            let gesture = gtk4::GestureClick::new();
            gesture.set_button(0);
            let lc = on_click.clone();
            let rc = on_right.clone();
            gesture.connect_released(move |g, _, _, _| {
                let cmd = match g.current_button() {
                    1 => lc.as_deref(),
                    3 => rc.as_deref(),
                    _ => None,
                };
                if let Some(cmd) = cmd {
                    std::process::Command::new("sh").arg("-c").arg(cmd).spawn().ok();
                }
            });
            container.add_controller(gesture);
        }

        container.append(&label);

        // Background thread: run command, push result to GTK main thread
        let weak_label = glib::SendWeakRef::from(label.downgrade());
        let fmt = format_str.clone();
        let cmd = command.clone();

        // Run immediately, then on interval
        run_and_update(cmd.clone(), fmt.clone(), use_markup, weak_label.clone());

        glib::timeout_add_seconds_local(interval_secs as u32, move || {
            let wl = weak_label.clone();
            if wl.upgrade().is_none() {
                return glib::ControlFlow::Break;
            }
            run_and_update(cmd.clone(), fmt.clone(), use_markup, wl);
            glib::ControlFlow::Continue
        });

        debug!("ExecWidget: built, command='{command}' interval={interval_secs}s");
        container.upcast()
    }

    fn update(&self, _widget: &Widget, _ctx: &WidgetContext) {}
    fn on_event(&self, _event: &WidgetEvent, _widget: &Widget) {}
}

fn run_and_update(
    command: String,
    format_str: String,
    use_markup: bool,
    weak_label: glib::SendWeakRef<Label>,
) {
    std::thread::spawn(move || {
        let raw = match std::process::Command::new("sh")
            .arg("-c")
            .arg(&command)
            .output()
        {
            Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
            Err(e) => {
                warn!("ExecWidget: command failed: {e}");
                String::from("error")
            }
        };

        let text = format_str.replace("{output}", &raw);

        glib::idle_add_once(move || {
            if let Some(label) = weak_label.upgrade() {
                if use_markup {
                    label.set_markup(&text);
                } else {
                    label.set_text(&text);
                }
            }
        });
    });
}
