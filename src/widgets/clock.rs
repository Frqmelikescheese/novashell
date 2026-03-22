use crate::widgets::traits::{NovaWidget, WidgetContext, WidgetEvent};
use chrono::Local;
use gtk4::prelude::*;
use gtk4::{Label, Orientation, Widget};
use tracing::debug;

/// Built-in clock widget showing time, date, and day-of-week
pub struct ClockWidget;

impl ClockWidget {
    pub fn new() -> Self {
        Self
    }

    /// Format the current time using the provided format string
    fn format_time(fmt: &str) -> String {
        Local::now().format(fmt).to_string()
    }
}

impl Default for ClockWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl NovaWidget for ClockWidget {
    fn name(&self) -> &str {
        "clock"
    }

    fn build(&self, ctx: &WidgetContext) -> Widget {
        let container = gtk4::Box::new(Orientation::Vertical, 4);
        container.add_css_class("nova-clock");

        let time_fmt = ctx.var("time_format", "%H:%M:%S");
        let date_fmt = ctx.var("date_format", "%B %-d");
        let day_fmt = ctx.var("day_format", "%A");

        let time_label = Label::new(Some(&Self::format_time(&time_fmt)));
        time_label.add_css_class("nova-clock__time");
        time_label.set_halign(gtk4::Align::Center);

        let date_label = Label::new(Some(&Self::format_time(&date_fmt)));
        date_label.add_css_class("nova-clock__date");
        date_label.set_halign(gtk4::Align::Center);

        let day_label = Label::new(Some(&Self::format_time(&day_fmt)));
        day_label.add_css_class("nova-clock__day");
        day_label.set_halign(gtk4::Align::Center);

        container.append(&time_label);
        container.append(&date_label);
        container.append(&day_label);

        // Set up 1-second timer to update the clock
        let time_label_clone = time_label.clone();
        let date_label_clone = date_label.clone();
        let day_label_clone = day_label.clone();
        let time_fmt_owned = time_fmt.clone();
        let date_fmt_owned = date_fmt.clone();
        let day_fmt_owned = day_fmt.clone();

        glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
            time_label_clone.set_text(&Self::format_time(&time_fmt_owned));
            date_label_clone.set_text(&Self::format_time(&date_fmt_owned));
            day_label_clone.set_text(&Self::format_time(&day_fmt_owned));
            glib::ControlFlow::Continue
        });

        debug!("ClockWidget: built");
        container.upcast()
    }

    fn update(&self, widget: &Widget, ctx: &WidgetContext) {
        // The timer handles automatic updates; this handles forced refreshes
        if let Some(container) = widget.downcast_ref::<gtk4::Box>() {
            let time_fmt = ctx.var("time_format", "%H:%M:%S");
            let date_fmt = ctx.var("date_format", "%B %-d");
            let day_fmt = ctx.var("day_format", "%A");

            let mut child = container.first_child();
            let mut idx = 0;
            while let Some(c) = child {
                if let Some(label) = c.downcast_ref::<Label>() {
                    match idx {
                        0 => label.set_text(&Self::format_time(&time_fmt)),
                        1 => label.set_text(&Self::format_time(&date_fmt)),
                        2 => label.set_text(&Self::format_time(&day_fmt)),
                        _ => {}
                    }
                }
                child = c.next_sibling();
                idx += 1;
            }
        }
    }

    fn on_event(&self, _event: &WidgetEvent, _widget: &Widget) {
        // Clock widget has no interactive elements
    }
}

/// Returns the current time formatted with the given format string.
/// Used by the builtin data source system.
pub fn get_time(format: &str) -> String {
    Local::now().format(format).to_string()
}

/// Returns the current date formatted with the given format string.
pub fn get_date(format: &str) -> String {
    Local::now().format(format).to_string()
}

/// Returns the current day-of-week formatted with the given format string.
pub fn get_day(format: &str) -> String {
    Local::now().format(format).to_string()
}
