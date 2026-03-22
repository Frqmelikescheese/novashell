use crate::widgets::traits::{NovaWidget, WidgetContext, WidgetEvent};
use gtk4::prelude::*;
use gtk4::{Button, Image, Label, Orientation, Scale, Widget};
use parking_lot::Mutex;
use std::sync::Arc;
use tracing::{debug, warn};

/// Playback state
#[derive(Default, Clone, PartialEq)]
pub enum PlaybackStatus {
    Playing,
    Paused,
    #[default]
    Stopped,
}

impl PlaybackStatus {
    fn from_str(s: &str) -> Self {
        match s.trim() {
            "Playing" => Self::Playing,
            "Paused"  => Self::Paused,
            _         => Self::Stopped,
        }
    }

    fn css_class(&self) -> &'static str {
        match self {
            Self::Playing => "nova-media--playing",
            Self::Paused  => "nova-media--paused",
            Self::Stopped => "nova-media--stopped",
        }
    }
}

/// Full state snapshot from MPRIS
#[derive(Default, Clone)]
struct MediaState {
    title:             String,
    artist:            String,
    album:             String,
    art_path:          String,   // local file path (may be temp file for http art)
    status:            PlaybackStatus,
    position_fraction: f64,
    duration_secs:     u64,
    position_secs:     u64,
    player_name:       String,   // e.g. "spotify", "firefox", "mpd"
    shuffle:           bool,
}

impl MediaState {
    fn has_content(&self) -> bool {
        !self.title.is_empty()
    }
}

/// MPRIS2 media control widget
pub struct MediaWidget {
    state: Arc<Mutex<MediaState>>,
}

impl MediaWidget {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(MediaState::default())),
        }
    }
}

impl Default for MediaWidget {
    fn default() -> Self { Self::new() }
}

impl NovaWidget for MediaWidget {
    fn name(&self) -> &str { "media" }

    fn build(&self, ctx: &WidgetContext) -> Widget {
        // ── Configurable vars ──────────────────────────────────────────────
        let show_art      = ctx.var("show_art",      "true")  != "false";
        let show_album    = ctx.var("show_album",    "true")  != "false";
        let show_player   = ctx.var("show_player",   "true")  != "false";
        let show_progress = ctx.var("show_progress", "true")  != "false";
        let show_time     = ctx.var("show_time",     "false") == "true";
        let show_shuffle  = ctx.var("show_shuffle",  "false") == "true";
        let hide_stopped  = ctx.var("hide_stopped",  "true")  != "false";
        let art_size: i32 = ctx.var("art_size", "48").parse().unwrap_or(48);
        let max_chars: i32 = ctx.var("max_chars", "26").parse().unwrap_or(26);

        // ── Root container ─────────────────────────────────────────────────
        let container = gtk4::Box::new(Orientation::Vertical, 8);
        container.add_css_class("nova-media");
        container.add_css_class("nova-media--stopped");

        if hide_stopped {
            container.set_visible(false);
        }

        // ── Info row: art + text ───────────────────────────────────────────
        let info_row = gtk4::Box::new(Orientation::Horizontal, 10);
        info_row.add_css_class("nova-media__info");

        let art_image = Image::from_icon_name("audio-x-generic-symbolic");
        art_image.add_css_class("nova-media__art");
        art_image.set_pixel_size(art_size);
        art_image.set_visible(show_art);

        let text_box = gtk4::Box::new(Orientation::Vertical, 2);
        text_box.add_css_class("nova-media__text");
        text_box.set_hexpand(true);

        // Player name badge
        let player_label = Label::new(None);
        player_label.add_css_class("nova-media__player");
        player_label.set_halign(gtk4::Align::Start);
        player_label.set_visible(show_player);

        let title_label = Label::new(Some("Nothing Playing"));
        title_label.add_css_class("nova-media__title");
        title_label.set_halign(gtk4::Align::Start);
        title_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        title_label.set_max_width_chars(max_chars);

        let artist_label = Label::new(None);
        artist_label.add_css_class("nova-media__artist");
        artist_label.set_halign(gtk4::Align::Start);
        artist_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        artist_label.set_max_width_chars(max_chars);

        let album_label = Label::new(None);
        album_label.add_css_class("nova-media__album");
        album_label.set_halign(gtk4::Align::Start);
        album_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        album_label.set_max_width_chars(max_chars);
        album_label.set_visible(show_album);

        text_box.append(&player_label);
        text_box.append(&title_label);
        text_box.append(&artist_label);
        text_box.append(&album_label);

        info_row.append(&art_image);
        info_row.append(&text_box);
        container.append(&info_row);

        // ── Controls row ───────────────────────────────────────────────────
        let controls = gtk4::Box::new(Orientation::Horizontal, 4);
        controls.add_css_class("nova-media__controls");
        controls.set_halign(gtk4::Align::Center);

        let shuffle_btn = Button::from_icon_name("media-playlist-shuffle-symbolic");
        shuffle_btn.add_css_class("nova-media__btn");
        shuffle_btn.add_css_class("nova-media__btn--shuffle");
        shuffle_btn.set_visible(show_shuffle);

        let prev_btn = Button::from_icon_name("media-skip-backward-symbolic");
        prev_btn.add_css_class("nova-media__btn");
        prev_btn.add_css_class("nova-media__btn--prev");

        let play_btn = Button::from_icon_name("media-playback-start-symbolic");
        play_btn.add_css_class("nova-media__btn");
        play_btn.add_css_class("nova-media__btn--playpause");

        let next_btn = Button::from_icon_name("media-skip-forward-symbolic");
        next_btn.add_css_class("nova-media__btn");
        next_btn.add_css_class("nova-media__btn--next");

        controls.append(&shuffle_btn);
        controls.append(&prev_btn);
        controls.append(&play_btn);
        controls.append(&next_btn);
        container.append(&controls);

        // ── Progress row ───────────────────────────────────────────────────
        let progress_row = gtk4::Box::new(Orientation::Horizontal, 6);
        progress_row.add_css_class("nova-media__progress-row");
        progress_row.set_visible(show_progress);

        let time_label = Label::new(Some("0:00"));
        time_label.add_css_class("nova-media__time");
        time_label.set_visible(show_time);

        let progress = Scale::with_range(Orientation::Horizontal, 0.0, 1.0, 0.001);
        progress.add_css_class("nova-media__progress");
        progress.set_draw_value(false);
        progress.set_value(0.0);
        progress.set_hexpand(true);

        let duration_label = Label::new(Some("0:00"));
        duration_label.add_css_class("nova-media__duration");
        duration_label.set_visible(show_time);

        progress_row.append(&time_label);
        progress_row.append(&progress);
        progress_row.append(&duration_label);
        container.append(&progress_row);

        // ── Button click handlers ──────────────────────────────────────────
        prev_btn.connect_clicked(|_| {
            std::process::Command::new("playerctl").arg("previous").spawn().ok();
        });

        next_btn.connect_clicked(|_| {
            std::process::Command::new("playerctl").arg("next").spawn().ok();
        });

        shuffle_btn.connect_clicked(|_| {
            std::process::Command::new("playerctl")
                .args(["shuffle", "Toggle"]).spawn().ok();
        });

        {
            let play_btn_c = play_btn.clone();
            play_btn.connect_clicked(move |_| {
                std::process::Command::new("playerctl").arg("play-pause").spawn().ok();
                // Optimistic icon flip while we wait for the poller
                let cur = play_btn_c.icon_name().unwrap_or_default();
                if cur.contains("start") {
                    play_btn_c.set_icon_name("media-playback-pause-symbolic");
                } else {
                    play_btn_c.set_icon_name("media-playback-start-symbolic");
                }
            });
        }

        // Seek on progress click
        {
            let progress_c = progress.clone();
            progress.connect_value_changed(move |scale| {
                let pos = scale.value();
                // Only seek on user interaction, not programmatic updates
                // We use change-value signal instead to avoid feedback loops
                let _ = pos;
            });
        }

        // ── Background poller ──────────────────────────────────────────────
        let (tx, rx) = crossbeam_channel::bounded::<MediaState>(4);

        std::thread::spawn(move || {
            let mut last_art_url = String::new();
            let mut cached_art_path = String::new();

            loop {
                let mut info = query_mpris_once();

                // Resolve album art: cache the download between polls
                let art_url = info.art_path.clone();
                if art_url != last_art_url {
                    last_art_url = art_url.clone();
                    cached_art_path = resolve_art_url(&art_url);
                }
                info.art_path = cached_art_path.clone();

                tx.send(info).ok();
                std::thread::sleep(std::time::Duration::from_millis(800));
            }
        });

        // ── GTK update timer ───────────────────────────────────────────────
        let title_c    = title_label.clone();
        let artist_c   = artist_label.clone();
        let album_c    = album_label.clone();
        let player_c   = player_label.clone();
        let play_btn_c = play_btn.clone();
        let progress_c = progress.clone();
        let art_c      = art_image.clone();
        let time_c     = time_label.clone();
        let dur_c      = duration_label.clone();
        let shuffle_c  = shuffle_btn.clone();
        let cont_c     = container.clone();
        let state_c    = self.state.clone();

        // Track current status css class so we can remove it on change
        let prev_status_class = Arc::new(Mutex::new("nova-media--stopped".to_string()));

        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            while let Ok(info) = rx.try_recv() {
                // Update shared state
                *state_c.lock() = info.clone();

                let has_content = info.has_content();

                // Show/hide the whole widget
                if hide_stopped {
                    cont_c.set_visible(has_content);
                }

                // Swap playback status CSS class
                {
                    let new_class = info.status.css_class();
                    let mut prev = prev_status_class.lock();
                    if *prev != new_class {
                        cont_c.remove_css_class(&prev);
                        cont_c.add_css_class(new_class);
                        *prev = new_class.to_string();
                    }
                }

                // Labels
                title_c.set_text(if info.title.is_empty() { "Nothing Playing" } else { &info.title });
                artist_c.set_text(&info.artist);
                album_c.set_text(&info.album);
                album_c.set_visible(show_album && !info.album.is_empty());

                // Player badge — capitalize first char
                let player_display = if info.player_name.is_empty() {
                    String::new()
                } else {
                    let mut c = info.player_name.chars();
                    match c.next() {
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                        None => String::new(),
                    }
                };
                player_c.set_text(&player_display);
                player_c.set_visible(show_player && !player_display.is_empty());

                // Play/pause icon
                let icon = match info.status {
                    PlaybackStatus::Playing => "media-playback-pause-symbolic",
                    _                       => "media-playback-start-symbolic",
                };
                play_btn_c.set_icon_name(icon);

                // Shuffle active state
                if info.shuffle {
                    shuffle_c.add_css_class("nova-media__btn--active");
                } else {
                    shuffle_c.remove_css_class("nova-media__btn--active");
                }

                // Progress
                progress_c.set_value(info.position_fraction);

                // Time labels
                if show_time {
                    time_c.set_text(&format_duration(info.position_secs));
                    dur_c.set_text(&format_duration(info.duration_secs));
                }

                // Album art
                if !info.art_path.is_empty() {
                    art_c.set_from_file(Some(&info.art_path));
                } else {
                    art_c.set_icon_name(Some("audio-x-generic-symbolic"));
                }
            }

            glib::ControlFlow::Continue
        });

        debug!("MediaWidget: built");
        container.upcast()
    }

    fn update(&self, _widget: &Widget, _ctx: &WidgetContext) {}

    fn on_event(&self, event: &WidgetEvent, _widget: &Widget) {
        if let WidgetEvent::ButtonClick { action } = event {
            let s = self.state.lock();
            match action.as_str() {
                "media::prev"       => { std::process::Command::new("playerctl").arg("previous").spawn().ok(); }
                "media::play_pause" => { std::process::Command::new("playerctl").arg("play-pause").spawn().ok(); }
                "media::next"       => { std::process::Command::new("playerctl").arg("next").spawn().ok(); }
                "media::shuffle"    => { std::process::Command::new("playerctl").args(["shuffle", "Toggle"]).spawn().ok(); }
                _ => {}
            }
            drop(s);
        }
    }
}

// ── MPRIS query ──────────────────────────────────────────────────────────────

/// Query all MPRIS data in a single playerctl invocation.
pub fn query_mpris_once() -> MediaState {
    // Single call to playerctl — gets everything we need
    let output = std::process::Command::new("playerctl")
        .args([
            "metadata",
            "--format",
            "{{title}}\n{{artist}}\n{{album}}\n{{mpris:artUrl}}\n{{status}}\n{{position}}\n{{mpris:length}}\n{{playerName}}\n{{shuffle}}",
        ])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        Ok(_) => {
            // playerctl succeeded but no player active
            return MediaState::default();
        }
        Err(e) => {
            debug!("MediaWidget: playerctl not found: {e}");
            return MediaState::default();
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines();

    let title       = lines.next().unwrap_or("").to_string();
    let artist      = lines.next().unwrap_or("").to_string();
    let album       = lines.next().unwrap_or("").to_string();
    let art_url     = lines.next().unwrap_or("").to_string();
    let status_str  = lines.next().unwrap_or("Stopped");
    let pos_str     = lines.next().unwrap_or("0");
    let len_str     = lines.next().unwrap_or("0");
    let player_name = lines.next().unwrap_or("").to_string();
    let shuffle_str = lines.next().unwrap_or("false");

    let status = PlaybackStatus::from_str(status_str);

    // playerctl returns position in microseconds
    let pos_us: u64 = pos_str.trim().parse().unwrap_or(0);
    let len_us: u64 = len_str.trim().parse().unwrap_or(0);
    let position_secs = pos_us / 1_000_000;
    let duration_secs = len_us / 1_000_000;
    let position_fraction = if len_us > 0 {
        (pos_us as f64 / len_us as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let shuffle = shuffle_str.trim() == "true" || shuffle_str.trim() == "True";

    MediaState {
        title,
        artist,
        album,
        art_path: art_url, // resolved later in the thread
        status,
        position_fraction,
        duration_secs,
        position_secs,
        player_name,
        shuffle,
    }
}

/// Resolve an art URL to a local file path.
///
/// - `file:///...` → strip prefix, return path
/// - `https?://...` → download to a temp file and return its path
/// - anything else  → return empty string
fn resolve_art_url(url: &str) -> String {
    if url.is_empty() {
        return String::new();
    }

    if let Some(path) = url.strip_prefix("file://") {
        return path.to_string();
    }

    if url.starts_with("http://") || url.starts_with("https://") {
        // Download to a deterministic temp path keyed by URL hash
        let hash = simple_hash(url);
        let tmp = format!("/tmp/nova-art-{hash}.img");

        // Only re-download if file doesn't exist yet
        if !std::path::Path::new(&tmp).exists() {
            let ok = std::process::Command::new("curl")
                .args(["-sf", "--max-time", "5", "-o", &tmp, url])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);

            if !ok {
                // Try wget as fallback
                let _ = std::process::Command::new("wget")
                    .args(["-q", "-O", &tmp, url])
                    .status();
            }
        }

        if std::path::Path::new(&tmp).exists() {
            return tmp;
        }

        warn!("MediaWidget: could not download art from {url}");
        return String::new();
    }

    String::new()
}

/// Minimal hash for cache key (djb2)
fn simple_hash(s: &str) -> u64 {
    s.bytes().fold(5381u64, |h, b| h.wrapping_mul(33).wrapping_add(b as u64))
}

fn format_duration(secs: u64) -> String {
    let m = secs / 60;
    let s = secs % 60;
    format!("{m}:{s:02}")
}

// ── Standalone accessors (used by eval_builtin) ───────────────────────────────

pub fn get_title()             -> String { query_mpris_once().title }
pub fn get_artist()            -> String { query_mpris_once().artist }
pub fn get_position_fraction() -> f64    { query_mpris_once().position_fraction }
pub fn get_art_path()          -> String {
    let state = query_mpris_once();
    resolve_art_url(&state.art_path)
}
pub fn get_play_icon() -> String {
    match query_mpris_once().status {
        PlaybackStatus::Playing => "media-playback-pause-symbolic".to_string(),
        _                       => "media-playback-start-symbolic".to_string(),
    }
}
