use gtk4::prelude::*;
use gtk4::{ApplicationWindow, Orientation, Widget};
use tracing::debug;

/// A NovaShell overlay window wrapping an ApplicationWindow.
///
/// Holds the root container and manages widget insertion.
pub struct NovaWindow {
    pub window: ApplicationWindow,
    root: gtk4::Box,
    pub screen_name: String,
}

impl NovaWindow {
    /// Create a new NovaWindow for the given ApplicationWindow
    pub fn new(window: ApplicationWindow, screen_name: impl Into<String>) -> Self {
        let root = gtk4::Box::new(Orientation::Vertical, 0);
        root.add_css_class("nova-root");
        window.set_child(Some(&root));

        // Make background transparent by default
        window.add_css_class("nova-window");

        Self {
            window,
            root,
            screen_name: screen_name.into(),
        }
    }

    /// Add a widget to this window's root container
    pub fn add_widget(&self, widget: &Widget) {
        debug!("NovaWindow[{}]: adding widget", self.screen_name);
        self.root.append(widget);
    }

    /// Remove all children from the root container
    pub fn clear(&self) {
        debug!("NovaWindow[{}]: clearing widgets", self.screen_name);
        while let Some(child) = self.root.first_child() {
            self.root.remove(&child);
        }
    }

    /// Show this window
    pub fn show(&self) {
        self.window.present();
    }

    /// Hide this window
    pub fn hide(&self) {
        self.window.hide();
    }

    /// Returns a reference to the underlying ApplicationWindow
    pub fn gtk_window(&self) -> &ApplicationWindow {
        &self.window
    }

    /// Returns the root container box
    pub fn root_container(&self) -> &gtk4::Box {
        &self.root
    }
}
