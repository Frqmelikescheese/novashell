use crate::config::schema::WidgetInstance;
use crate::error::{NovaError, Result};
use crate::widgets::WidgetDefinition;
use gtk4::prelude::*;
use gtk4::{
    Button, DrawingArea, Grid, Image, Label, LevelBar, Orientation, Overlay,
    ProgressBar, Revealer, Scale, ScrolledWindow, Separator, Widget,
};
use once_cell::sync::Lazy;
use quick_xml::events::Event as XmlEvent;
use quick_xml::Reader;
use regex::Regex;
use std::collections::HashMap;
use tracing::{debug, warn};

/// Regex for `{var_name}` template substitution
static VAR_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\{([a-zA-Z_][a-zA-Z0-9_]*)\}").unwrap());

/// Interpolate `{var}` placeholders in a string using the provided variable map
pub fn interpolate(template: &str, vars: &HashMap<String, String>) -> String {
    VAR_REGEX
        .replace_all(template, |caps: &regex::Captures| {
            let key = &caps[1];
            vars.get(key).cloned().unwrap_or_default()
        })
        .into_owned()
}

/// Factory for building GTK widget trees from `WidgetDefinition` XML templates
pub struct WidgetFactory;

impl WidgetFactory {
    /// Build a GTK4 Widget from a definition + instance config.
    ///
    /// Parses the XML template, instantiates GTK widgets recursively, applies
    /// CSS classes, and sets up variable-driven update timers.
    pub fn build(
        definition: &WidgetDefinition,
        instance: &WidgetInstance,
    ) -> Result<Widget> {
        // Merge instance vars over definition defaults
        let mut vars: HashMap<String, String> = definition
            .vars
            .iter()
            .map(|(k, v)| (k.clone(), v.default.clone()))
            .collect();

        if let Some(instance_vars) = &instance.vars {
            for (k, v) in instance_vars {
                vars.insert(k.clone(), v.clone());
            }
        }

        // Parse the XML template (vars already cloned above for initial render)
        let (root_widget, var_bindings) = parse_xml_template(&definition.template, &vars)?;

        // Apply style override if any
        if let Some(style) = &instance.style_override {
            apply_inline_css(&root_widget, style);
        }

        // Set up update timers for variables with builtins or scripts
        setup_var_timers(definition, instance, var_bindings, vars);

        debug!(
            "WidgetFactory: built '{}' (id={:?})",
            definition.name, instance.id
        );

        Ok(root_widget)
    }
}

/// A binding from a widget to a text template that may contain `{var}` placeholders.
/// When any var in the template updates, the widget is re-rendered from the template.
#[derive(Clone)]
struct VarBinding {
    widget: Widget,
    /// The raw attribute template, e.g. `"{percent}%"` or `"{title}"`
    template: String,
}

/// Recursively parse XML template and create GTK widgets.
/// Returns the root widget and a map of var_name → Vec<VarBinding>.
///
/// Uses separate arms for Start vs Empty to avoid the lookahead-consumption bug
/// that was present in the original code.
fn parse_xml_template(
    template: &str,
    vars: &HashMap<String, String>,
) -> Result<(Widget, HashMap<String, Vec<VarBinding>>)> {
    let mut reader = Reader::from_str(template);
    reader.trim_text(true);

    let mut stack: Vec<gtk4::Box> = Vec::new();
    let mut root: Option<Widget> = None;
    let mut var_bindings: HashMap<String, Vec<VarBinding>> = HashMap::new();

    loop {
        match reader.read_event() {
            // Container / open tag — children follow until the matching End
            Ok(XmlEvent::Start(e)) => {
                let tag = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_lowercase();
                let (raw_attrs, attrs) = parse_element_attrs(&e, vars);
                let widget = build_gtk_widget(&tag, &attrs)?;
                register_var_bindings(&raw_attrs, &widget, &mut var_bindings);
                attach_to_parent(&widget, &stack, &mut root);

                // Containers push themselves so their children attach to them
                let is_container = matches!(
                    tag.as_str(),
                    "box" | "grid" | "scrolled" | "revealer" | "overlay"
                );
                if is_container {
                    if let Ok(b) = widget.clone().downcast::<gtk4::Box>() {
                        stack.push(b);
                    } else {
                        stack.push(gtk4::Box::new(Orientation::Vertical, 0));
                    }
                }
            }
            // Self-closing tag — no children, no stack push
            Ok(XmlEvent::Empty(e)) => {
                let tag = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_lowercase();
                let (raw_attrs, attrs) = parse_element_attrs(&e, vars);
                let widget = build_gtk_widget(&tag, &attrs)?;
                register_var_bindings(&raw_attrs, &widget, &mut var_bindings);
                attach_to_parent(&widget, &stack, &mut root);
                // No stack push — self-contained element
            }
            Ok(XmlEvent::End(_)) => {
                stack.pop();
            }
            Ok(XmlEvent::Eof) => break,
            Err(e) => {
                return Err(NovaError::Xml(format!("XML parse error: {e}")));
            }
            _ => {}
        }
    }

    let root = root.ok_or_else(|| NovaError::Widget("Empty widget template".to_string()))?;
    Ok((root, var_bindings))
}

/// Collect raw + interpolated attributes from an XML element.
fn parse_element_attrs(
    e: &quick_xml::events::BytesStart,
    vars: &HashMap<String, String>,
) -> (HashMap<String, String>, HashMap<String, String>) {
    let mut raw_attrs: HashMap<String, String> = HashMap::new();
    for attr in e.attributes().flatten() {
        let k = std::str::from_utf8(attr.key.as_ref()).unwrap_or("").to_string();
        let v = std::str::from_utf8(&attr.value).unwrap_or("").to_string();
        raw_attrs.insert(k, v);
    }
    let attrs: HashMap<String, String> = raw_attrs
        .iter()
        .map(|(k, v)| (k.clone(), interpolate(v, vars)))
        .collect();
    (raw_attrs, attrs)
}

/// Register var bindings for dynamic attributes on a widget.
fn register_var_bindings(
    raw_attrs: &HashMap<String, String>,
    widget: &Widget,
    var_bindings: &mut HashMap<String, Vec<VarBinding>>,
) {
    for key in &["text", "markup", "value", "icon", "file"] {
        if let Some(raw_template) = raw_attrs.get(*key) {
            if VAR_REGEX.is_match(raw_template) {
                let binding = VarBinding {
                    widget: widget.clone(),
                    template: raw_template.clone(),
                };
                for cap in VAR_REGEX.captures_iter(raw_template) {
                    let var_name = cap[1].to_string();
                    var_bindings.entry(var_name).or_default().push(binding.clone());
                }
            }
        }
    }
}

/// Attach a widget to the top of the parent stack, or set as root.
fn attach_to_parent(widget: &Widget, stack: &[gtk4::Box], root: &mut Option<Widget>) {
    if let Some(parent) = stack.last() {
        parent.append(widget);
    } else {
        *root = Some(widget.clone());
    }
}


/// Create a GTK widget from a tag name and attributes
fn build_gtk_widget(
    tag: &str,
    attrs: &HashMap<String, String>,
) -> Result<Widget> {
    let widget: Widget = match tag {
        "box" => {
            let orientation = parse_orientation(attrs.get("orientation").map(|s| s.as_str()));
            let spacing: i32 = attrs
                .get("spacing")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let b = gtk4::Box::new(orientation, spacing);
            if let Some(halign) = attrs.get("halign") {
                b.set_halign(parse_align(halign));
            }
            if let Some(valign) = attrs.get("valign") {
                b.set_valign(parse_align(valign));
            }
            if let Some("true") = attrs.get("hexpand").map(|s| s.as_str()) {
                b.set_hexpand(true);
            }
            if let Some("true") = attrs.get("vexpand").map(|s| s.as_str()) {
                b.set_vexpand(true);
            }
            b.upcast()
        }
        "label" => {
            let text = attrs.get("text").map(|s| s.as_str()).unwrap_or("");
            let label = Label::new(Some(text));
            if let Some(align) = attrs.get("halign") {
                label.set_halign(parse_align(align));
            }
            if let Some(ellipsize) = attrs.get("ellipsize") {
                label.set_ellipsize(parse_ellipsize(ellipsize));
            }
            if let Some(max_w) = attrs.get("max-width-chars").and_then(|s| s.parse::<i32>().ok()) {
                label.set_max_width_chars(max_w);
            }
            if let Some("true") = attrs.get("wrap").map(|s| s.as_str()) {
                label.set_wrap(true);
            }
            if let Some(markup) = attrs.get("markup") {
                label.set_markup(markup);
            }
            label.upcast()
        }
        "button" => {
            let btn = if let Some(icon) = attrs.get("icon") {
                Button::from_icon_name(icon)
            } else if let Some(label_text) = attrs.get("label") {
                Button::with_label(label_text)
            } else {
                Button::new()
            };

            if let Some(action) = attrs.get("action").cloned() {
                btn.connect_clicked(move |_| {
                    dispatch_action(&action);
                });
            }

            if let Some(tooltip) = attrs.get("tooltip") {
                btn.set_tooltip_text(Some(tooltip));
            }
            btn.upcast()
        }
        "image" => {
            let img = if let Some(file) = attrs.get("file") {
                Image::from_file(file)
            } else if let Some(icon) = attrs.get("icon").or_else(|| attrs.get("fallback-icon")) {
                Image::from_icon_name(icon)
            } else {
                Image::new()
            };

            if let Some(size) = attrs.get("pixel-size").and_then(|s| s.parse::<i32>().ok()) {
                img.set_pixel_size(size);
            }
            if let Some(w) = attrs.get("width").and_then(|s| s.parse::<i32>().ok()) {
                img.set_size_request(
                    w,
                    attrs
                        .get("height")
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(-1),
                );
            }
            img.upcast()
        }
        "levelbar" => {
            let bar = LevelBar::new();
            bar.set_min_value(0.0);
            bar.set_max_value(1.0);
            let val: f64 = attrs
                .get("value")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            bar.set_value(val);
            if let Some(orient) = attrs.get("orientation") {
                bar.set_orientation(parse_orientation(Some(orient)));
            }
            if let Some("true") = attrs.get("hexpand").map(|s| s.as_str()) {
                bar.set_hexpand(true);
            }
            bar.upcast()
        }
        "progressbar" => {
            let pb = ProgressBar::new();
            let val: f64 = attrs
                .get("value")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            pb.set_fraction(val.clamp(0.0, 1.0));
            if let Some(text) = attrs.get("text") {
                pb.set_show_text(true);
                pb.set_text(Some(text));
            }
            pb.upcast()
        }
        "scale" => {
            let orientation = parse_orientation(attrs.get("orientation").map(|s| s.as_str()));
            let min: f64 = attrs.get("min").and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let max: f64 = attrs.get("max").and_then(|s| s.parse().ok()).unwrap_or(1.0);
            let step: f64 = attrs.get("step").and_then(|s| s.parse().ok()).unwrap_or(0.01);
            let scale = Scale::with_range(orientation, min, max, step);
            let val: f64 = attrs.get("value").and_then(|s| s.parse().ok()).unwrap_or(0.0);
            scale.set_value(val);
            scale.set_draw_value(
                attrs
                    .get("draw-value")
                    .map(|s| s == "true")
                    .unwrap_or(false),
            );
            if let Some("true") = attrs.get("hexpand").map(|s| s.as_str()) {
                scale.set_hexpand(true);
            }
            // Wire action (e.g. volume::set) on value-changed
            if let Some(action) = attrs.get("action").cloned() {
                scale.connect_value_changed(move |s| {
                    let v = s.value();
                    let cmd = match action.as_str() {
                        "volume::set" => format!("wpctl set-volume @DEFAULT_AUDIO_SINK@ {:.2}", v),
                        _ => return,
                    };
                    std::process::Command::new("sh").arg("-c").arg(&cmd).spawn().ok();
                });
            }
            scale.upcast()
        }
        "separator" => {
            let orientation = parse_orientation(attrs.get("orientation").map(|s| s.as_str()));
            Separator::new(orientation).upcast()
        }
        "drawing" => {
            let area = DrawingArea::new();
            if let Some(w) = attrs.get("width").and_then(|s| s.parse::<i32>().ok()) {
                area.set_content_width(w);
            }
            if let Some(h) = attrs.get("height").and_then(|s| s.parse::<i32>().ok()) {
                area.set_content_height(h);
            }
            if let Some("true") = attrs.get("hexpand").map(|s| s.as_str()) {
                area.set_hexpand(true);
            }
            area.upcast()
        }
        "revealer" => {
            let r = Revealer::new();
            if let Some(transition) = attrs.get("transition") {
                r.set_transition_type(parse_revealer_transition(transition));
            }
            if let Some(dur) = attrs.get("duration").and_then(|s| s.parse::<u32>().ok()) {
                r.set_transition_duration(dur);
            }
            r.upcast()
        }
        "overlay" => Overlay::new().upcast(),
        "grid" => {
            let g = Grid::new();
            if let Some(row_spacing) = attrs.get("row-spacing").and_then(|s| s.parse::<i32>().ok()) {
                g.set_row_spacing(row_spacing as u32);
            }
            if let Some(col_spacing) = attrs.get("col-spacing").and_then(|s| s.parse::<i32>().ok()) {
                g.set_column_spacing(col_spacing as u32);
            }
            g.upcast()
        }
        "scrolled" => {
            let sw = ScrolledWindow::new();
            if let Some("true") = attrs.get("vexpand").map(|s| s.as_str()) {
                sw.set_vexpand(true);
            }
            sw.upcast()
        }
        unknown => {
            warn!("WidgetFactory: unknown element <{unknown}>, using Box");
            gtk4::Box::new(Orientation::Vertical, 0).upcast()
        }
    };

    // Apply common attributes
    if let Some(class) = attrs.get("class") {
        for cls in class.split_whitespace() {
            widget.add_css_class(cls);
        }
    }

    if let Some(id) = attrs.get("id") {
        widget.set_widget_name(id);
    }

    if let (Some(w), Some(h)) = (
        attrs.get("min-width").and_then(|s| s.parse::<i32>().ok()),
        attrs.get("min-height").and_then(|s| s.parse::<i32>().ok()),
    ) {
        widget.set_size_request(w, h);
    } else if let Some(w) = attrs.get("min-width").and_then(|s| s.parse::<i32>().ok()) {
        widget.set_size_request(w, -1);
    } else if let Some(h) = attrs.get("min-height").and_then(|s| s.parse::<i32>().ok()) {
        widget.set_size_request(-1, h);
    }

    if attrs.get("hexpand").map(|s| s.as_str()) == Some("true") {
        widget.set_hexpand(true);
    }
    if attrs.get("vexpand").map(|s| s.as_str()) == Some("true") {
        widget.set_vexpand(true);
    }

    // Margin
    if let Some(m) = attrs.get("margin").and_then(|s| s.parse::<i32>().ok()) {
        widget.set_margin_top(m);
        widget.set_margin_bottom(m);
        widget.set_margin_start(m);
        widget.set_margin_end(m);
    }

    Ok(widget)
}

/// Parse orientation from attribute string
fn parse_orientation(s: Option<&str>) -> Orientation {
    match s {
        Some("h") | Some("horizontal") => Orientation::Horizontal,
        _ => Orientation::Vertical,
    }
}

/// Parse Gtk Align from attribute string
fn parse_align(s: &str) -> gtk4::Align {
    match s {
        "start" => gtk4::Align::Start,
        "end" => gtk4::Align::End,
        "center" => gtk4::Align::Center,
        "fill" => gtk4::Align::Fill,
        _ => gtk4::Align::Fill,
    }
}

/// Parse ellipsize mode
fn parse_ellipsize(s: &str) -> pango::EllipsizeMode {
    match s {
        "start" => pango::EllipsizeMode::Start,
        "middle" => pango::EllipsizeMode::Middle,
        "end" => pango::EllipsizeMode::End,
        _ => pango::EllipsizeMode::None,
    }
}

/// Parse revealer transition type
fn parse_revealer_transition(s: &str) -> gtk4::RevealerTransitionType {
    match s {
        "slide-right" => gtk4::RevealerTransitionType::SlideRight,
        "slide-left" => gtk4::RevealerTransitionType::SlideLeft,
        "slide-down" => gtk4::RevealerTransitionType::SlideDown,
        "slide-up" => gtk4::RevealerTransitionType::SlideUp,
        "crossfade" => gtk4::RevealerTransitionType::Crossfade,
        _ => gtk4::RevealerTransitionType::None,
    }
}

/// Apply an inline CSS style string to a widget via a CssProvider
fn apply_inline_css(widget: &Widget, css: &str) {
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(css);
    widget
        .style_context()
        .add_provider(&provider, gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);
}

/// Set up glib timers for variables that have builtin sources or scripts.
/// Uses the var_bindings map built during parsing; each binding holds the widget and
/// the full text template (e.g. "{percent}%") so format strings are preserved.
fn setup_var_timers(
    definition: &WidgetDefinition,
    _instance: &WidgetInstance,
    var_bindings: HashMap<String, Vec<VarBinding>>,
    initial_vars: HashMap<String, String>,
) {
    use crate::widgets::{eval_builtin, eval_script};
    use indexmap::IndexMap;
    use std::sync::{Arc, Mutex};

    // Shared current values — all timers update their slot here so that templates
    // containing multiple vars (e.g. "{a} / {b}") can still render correctly.
    let current_values: Arc<Mutex<HashMap<String, String>>> =
        Arc::new(Mutex::new(initial_vars));

    for (var_name, var_def) in &definition.vars {
        if var_def.builtin.is_none() && var_def.script.is_none() {
            continue;
        }

        let builtin = var_def.builtin.clone();
        let script = var_def.script.clone();
        let interval = var_def.interval_ms;
        let bindings = var_bindings.get(var_name).cloned().unwrap_or_default();
        let var_name = var_name.clone();
        let values_arc = Arc::clone(&current_values);
        let empty_map: IndexMap<String, String> = IndexMap::new();

        if bindings.is_empty() {
            debug!("setup_var_timers: no widgets bound to var '{var_name}', skipping timer");
            continue;
        }

        glib::timeout_add_local(std::time::Duration::from_millis(interval), move || {
            let value = if let Some(ref b) = builtin {
                eval_builtin(b, &empty_map)
            } else if let Some(ref s) = script {
                eval_script(s)
            } else {
                return glib::ControlFlow::Continue;
            };

            // Update shared state
            {
                let mut vals = values_arc.lock().unwrap();
                vals.insert(var_name.clone(), value);
            }

            // Re-render every binding's template with the full current-values snapshot
            let vals = values_arc.lock().unwrap().clone();
            for binding in &bindings {
                let rendered = interpolate(&binding.template, &vals);
                apply_rendered_to_widget(&binding.widget, &rendered);
            }
            glib::ControlFlow::Continue
        });
    }
}

/// Apply a fully-rendered string to a widget.
/// Dispatches based on widget type: Label → set_text, LevelBar/Scale → set_value (f64),
/// Button → set_icon_name, Image → set_icon_name.
fn apply_rendered_to_widget(widget: &Widget, rendered: &str) {
    if let Some(label) = widget.downcast_ref::<Label>() {
        label.set_text(rendered);
    } else if let Some(bar) = widget.downcast_ref::<LevelBar>() {
        if let Ok(fval) = rendered.parse::<f64>() {
            bar.set_value(fval.clamp(0.0, 1.0));
        }
    } else if let Some(scale) = widget.downcast_ref::<Scale>() {
        if let Ok(fval) = rendered.parse::<f64>() {
            scale.set_value(fval);
        }
    } else if let Some(btn) = widget.downcast_ref::<Button>() {
        // Update button icon (for play/pause icon toggling)
        btn.set_icon_name(rendered);
    } else if let Some(img) = widget.downcast_ref::<Image>() {
        // Update image from file path or icon name
        if rendered.starts_with('/') || rendered.starts_with("~/") {
            img.set_from_file(Some(rendered));
        } else if !rendered.is_empty() {
            img.set_icon_name(Some(rendered));
        }
    }
}

/// Dispatch a widget button action by running the appropriate shell command.
///
/// Actions follow the `namespace::command` convention, e.g.:
/// - `media::play_pause`, `media::prev`, `media::next`
/// - `volume::mute_toggle`
/// - `launcher::open`
pub fn dispatch_action(action: &str) {
    let cmd: Option<&str> = match action {
        // ── Media (playerctl) ─────────────────────────────────────────────
        "media::play_pause" => Some("playerctl play-pause"),
        "media::prev"       => Some("playerctl previous"),
        "media::next"       => Some("playerctl next"),
        "media::stop"       => Some("playerctl stop"),

        // ── Volume (wpctl / PipeWire) ─────────────────────────────────────
        "volume::mute_toggle"  => Some("wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle"),
        "volume::up"           => Some("wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%+"),
        "volume::down"         => Some("wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%-"),

        // ── Launcher ─────────────────────────────────────────────────────
        "launcher::open" => Some("rofi -show drun || wofi --show drun || fuzzel"),

        _ => None,
    };

    if let Some(shell_cmd) = cmd {
        debug!("dispatch_action: {action} → {shell_cmd}");
        std::process::Command::new("sh")
            .arg("-c")
            .arg(shell_cmd)
            .spawn()
            .ok();
    } else {
        warn!("dispatch_action: unknown action '{action}'");
    }
}
