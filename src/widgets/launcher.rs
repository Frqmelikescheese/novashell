use crate::widgets::traits::{NovaWidget, WidgetContext, WidgetEvent};
use gtk4::prelude::*;
use gtk4::{Button, Entry, Label, ListBox, Orientation, ScrolledWindow, Widget, Window};
use std::path::PathBuf;
use tracing::{debug, warn};

/// A parsed .desktop file entry
#[derive(Debug, Clone)]
pub struct DesktopEntry {
    pub name: String,
    pub exec: String,
    pub icon: String,
    pub comment: String,
    pub categories: Vec<String>,
    pub no_display: bool,
}

/// Read and parse .desktop files from system and user application dirs
pub fn load_desktop_entries() -> Vec<DesktopEntry> {
    let mut dirs: Vec<PathBuf> = vec![PathBuf::from("/usr/share/applications")];

    if let Some(local_data) = dirs::data_local_dir() {
        dirs.push(local_data.join("applications"));
    }

    let mut entries = Vec::new();

    for dir in &dirs {
        if !dir.exists() {
            continue;
        }

        let read_dir = match std::fs::read_dir(dir) {
            Ok(r) => r,
            Err(e) => {
                warn!("Launcher: cannot read {}: {e}", dir.display());
                continue;
            }
        };

        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("desktop") {
                if let Some(de) = parse_desktop_file(&path) {
                    if !de.no_display && !de.exec.is_empty() {
                        entries.push(de);
                    }
                }
            }
        }
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries.dedup_by(|a, b| a.name == b.name);
    entries
}

/// Parse a single .desktop file
fn parse_desktop_file(path: &std::path::Path) -> Option<DesktopEntry> {
    let content = std::fs::read_to_string(path).ok()?;

    let mut in_desktop_entry = false;
    let mut name = String::new();
    let mut exec = String::new();
    let mut icon = String::new();
    let mut comment = String::new();
    let mut categories: Vec<String> = Vec::new();
    let mut no_display = false;

    for line in content.lines() {
        let line = line.trim();
        if line == "[Desktop Entry]" {
            in_desktop_entry = true;
            continue;
        }
        if line.starts_with('[') && line != "[Desktop Entry]" {
            in_desktop_entry = false;
            continue;
        }
        if !in_desktop_entry {
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            let k = key.trim();
            let v = value.trim();
            match k {
                "Name" => name = v.to_string(),
                "Exec" => exec = clean_exec(v),
                "Icon" => icon = v.to_string(),
                "Comment" => comment = v.to_string(),
                "Categories" => {
                    categories = v.split(';').map(|s| s.to_string()).collect()
                }
                "NoDisplay" => no_display = v.eq_ignore_ascii_case("true"),
                "Type" => {
                    if v != "Application" {
                        return None;
                    }
                }
                _ => {}
            }
        }
    }

    if name.is_empty() {
        return None;
    }

    Some(DesktopEntry {
        name,
        exec,
        icon,
        comment,
        categories,
        no_display,
    })
}

/// Remove desktop file field codes (%f, %u, %F, %U, %i, %c, %k) from Exec
fn clean_exec(exec: &str) -> String {
    let mut result = exec.to_string();
    for code in &["%f", "%u", "%F", "%U", "%i", "%c", "%k", "%d", "%D", "%n", "%N", "%v", "%m"] {
        result = result.replace(code, "");
    }
    result.trim().to_string()
}

/// Launch an application from its Exec string
pub fn launch_app(exec: &str) {
    let parts: Vec<&str> = exec.split_whitespace().collect();
    if parts.is_empty() {
        return;
    }

    debug!("Launcher: launching '{exec}'");

    let mut cmd = std::process::Command::new(parts[0]);
    if parts.len() > 1 {
        cmd.args(&parts[1..]);
    }

    if let Err(e) = cmd.spawn() {
        warn!("Launcher: failed to launch '{exec}': {e}");
    }
}

/// App launcher widget with search overlay
pub struct LauncherWidget;

impl LauncherWidget {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LauncherWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl NovaWidget for LauncherWidget {
    fn name(&self) -> &str {
        "launcher"
    }

    fn build(&self, ctx: &WidgetContext) -> Widget {
        let container = gtk4::Box::new(Orientation::Vertical, 0);
        container.add_css_class("nova-launcher");

        let icon = ctx.var("icon", "system-search");
        let tooltip = ctx.var("tooltip", "App Launcher");

        let launch_btn = Button::from_icon_name(&icon);
        launch_btn.add_css_class("nova-launcher__button");
        launch_btn.set_tooltip_text(Some(&tooltip));

        container.append(&launch_btn);

        // Load desktop entries in a background thread so startup isn't blocked
        let entries: std::sync::Arc<parking_lot::RwLock<Vec<DesktopEntry>>> =
            std::sync::Arc::new(parking_lot::RwLock::new(Vec::new()));

        let entries_clone = entries.clone();
        std::thread::spawn(move || {
            let loaded = load_desktop_entries();
            debug!("Launcher: loaded {} desktop entries", loaded.len());
            let mut w = entries_clone.write();
            *w = loaded;
        });

        // Open search overlay when button clicked
        launch_btn.connect_clicked(move |btn| {
            let entries_snap = {
                let r = entries.read();
                r.clone()
            };
            open_launcher_overlay(btn, entries_snap);
        });

        debug!("LauncherWidget: built");
        container.upcast()
    }

    fn update(&self, _widget: &Widget, _ctx: &WidgetContext) {}

    fn on_event(&self, _event: &WidgetEvent, _widget: &Widget) {}
}

/// Open a transient search overlay window attached to the parent button
fn open_launcher_overlay(btn: &Button, entries: Vec<DesktopEntry>) {
    // Find the root window
    let root = btn.root();
    let parent_win = root.and_then(|r| r.downcast::<gtk4::Window>().ok());

    let overlay = Window::builder()
        .title("Launch Application")
        .default_width(400)
        .default_height(500)
        .resizable(false)
        .build();

    overlay.add_css_class("nova-launcher__overlay");

    if let Some(parent) = parent_win {
        overlay.set_transient_for(Some(&parent));
    }
    overlay.set_modal(true);

    let vbox = gtk4::Box::new(Orientation::Vertical, 8);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);

    let search = Entry::new();
    search.add_css_class("nova-launcher__search");
    search.set_placeholder_text(Some("Search applications..."));
    vbox.append(&search);

    let scrolled = ScrolledWindow::new();
    scrolled.set_vexpand(true);
    let list = ListBox::new();
    list.add_css_class("nova-launcher__list");

    // Populate list
    for entry in &entries {
        let row = gtk4::ListBoxRow::new();
        let row_box = gtk4::Box::new(Orientation::Horizontal, 8);
        row_box.set_margin_top(4);
        row_box.set_margin_bottom(4);
        row_box.set_margin_start(8);
        row_box.set_margin_end(8);

        if !entry.icon.is_empty() {
            let icon_img = gtk4::Image::from_icon_name(&entry.icon);
            icon_img.set_pixel_size(24);
            row_box.append(&icon_img);
        }

        let label_box = gtk4::Box::new(Orientation::Vertical, 2);
        let name_lbl = Label::new(Some(&entry.name));
        name_lbl.set_halign(gtk4::Align::Start);
        name_lbl.add_css_class("nova-launcher__app-name");

        if !entry.comment.is_empty() {
            let desc_lbl = Label::new(Some(&entry.comment));
            desc_lbl.set_halign(gtk4::Align::Start);
            desc_lbl.add_css_class("nova-launcher__app-desc");
        }

        label_box.append(&name_lbl);
        row_box.append(&label_box);
        row.set_child(Some(&row_box));
        list.append(&row);
    }

    scrolled.set_child(Some(&list));
    vbox.append(&scrolled);
    overlay.set_child(Some(&vbox));

    // Wire up search filtering
    {
        let list_clone = list.clone();
        let entries_clone = entries.clone();
        search.connect_changed(move |entry| {
            let query = entry.text().to_lowercase();
            // Remove all rows and re-add matching ones
            while let Some(child) = list_clone.first_child() {
                list_clone.remove(&child);
            }
            for de in &entries_clone {
                if query.is_empty()
                    || de.name.to_lowercase().contains(&query)
                    || de.comment.to_lowercase().contains(&query)
                {
                    let row = gtk4::ListBoxRow::new();
                    let row_box = gtk4::Box::new(Orientation::Horizontal, 8);
                    row_box.set_margin_top(4);
                    row_box.set_margin_bottom(4);
                    row_box.set_margin_start(8);
                    row_box.set_margin_end(8);

                    if !de.icon.is_empty() {
                        let icon_img = gtk4::Image::from_icon_name(&de.icon);
                        icon_img.set_pixel_size(24);
                        row_box.append(&icon_img);
                    }

                    let name_lbl = Label::new(Some(&de.name));
                    name_lbl.set_halign(gtk4::Align::Start);
                    row_box.append(&name_lbl);
                    row.set_child(Some(&row_box));
                    list_clone.append(&row);
                }
            }
        });
    }

    // Launch on row activation
    {
        let entries_launch = entries.clone();
        let overlay_clone = overlay.clone();
        list.connect_row_activated(move |l, row| {
            let idx = row.index() as usize;
            // Map visible rows
            let query_widget = l
                .first_child()
                .and_then(|c| c.downcast::<gtk4::ListBoxRow>().ok());
            let mut current = query_widget;
            let mut found_idx = 0;
            let mut exec_opt: Option<String> = None;

            // Walk list to find exec at this row index
            let mut walker = l.first_child();
            let mut wi = 0;
            while let Some(child) = walker {
                if let Ok(lbr) = child.clone().downcast::<gtk4::ListBoxRow>() {
                    if wi == idx {
                        // Try to get name from label
                        if let Some(rb) = lbr.child() {
                            if let Ok(hbox) = rb.downcast::<gtk4::Box>() {
                                let mut hchild = hbox.first_child();
                                while let Some(c) = hchild {
                                    if let Ok(lbl) = c.clone().downcast::<Label>() {
                                        let name = lbl.text().to_string();
                                        exec_opt = entries_launch
                                            .iter()
                                            .find(|e| e.name == name)
                                            .map(|e| e.exec.clone());
                                        break;
                                    }
                                    hchild = c.next_sibling();
                                }
                            }
                        }
                        break;
                    }
                }
                walker = child.next_sibling();
                wi += 1;
            }

            if let Some(exec) = exec_opt {
                launch_app(&exec);
                overlay_clone.close();
            }
        });
    }

    // Close on Escape
    let key_ctrl = gtk4::EventControllerKey::new();
    {
        let overlay_esc = overlay.clone();
        key_ctrl.connect_key_pressed(move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                overlay_esc.close();
                return gtk4::glib::Propagation::Stop;
            }
            gtk4::glib::Propagation::Proceed
        });
    }
    overlay.add_controller(key_ctrl);

    overlay.present();
}
