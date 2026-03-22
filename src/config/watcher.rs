use crate::error::{NovaError, Result};
use crossbeam_channel::{bounded, Receiver, Sender};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{debug, error, warn};

/// Classification of a file change event
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeEvent {
    /// A CSS file was modified (.css)
    CssChange(PathBuf),
    /// A widget definition was modified (.widget)
    WidgetChange(PathBuf),
    /// The main config was modified (.yaml, .yml)
    ConfigChange(PathBuf),
    /// Some other file changed
    Other(PathBuf),
}

/// File watcher for the NovaShell config directory.
///
/// Watches recursively for file changes and classifies them into
/// `ChangeEvent` variants sent over a crossbeam channel.
pub struct ConfigWatcher {
    _watcher: RecommendedWatcher,
    pub receiver: Receiver<ChangeEvent>,
}

impl ConfigWatcher {
    /// Start watching the given directory. Returns a `ConfigWatcher` whose
    /// `receiver` channel yields `ChangeEvent` values as files change.
    pub fn new(watch_dir: &Path) -> Result<Self> {
        let (tx, rx) = bounded::<ChangeEvent>(64);

        let mut watcher = RecommendedWatcher::new(
            move |result: std::result::Result<Event, notify::Error>| {
                match result {
                    Ok(event) => {
                        if let Some(change) = classify_event(&event) {
                            if let Err(e) = tx.send(change) {
                                error!("ConfigWatcher: failed to send event: {e}");
                            }
                        }
                    }
                    Err(e) => {
                        warn!("ConfigWatcher: watch error: {e}");
                    }
                }
            },
            Config::default().with_poll_interval(Duration::from_millis(500)),
        )
        .map_err(|e| NovaError::Watch(e.to_string()))?;

        watcher
            .watch(watch_dir, RecursiveMode::Recursive)
            .map_err(|e| NovaError::Watch(format!("Failed to watch {}: {e}", watch_dir.display())))?;

        debug!("ConfigWatcher: watching {}", watch_dir.display());

        Ok(Self {
            _watcher: watcher,
            receiver: rx,
        })
    }
}

/// Given a notify event, attempt to classify it. Returns None for irrelevant
/// event kinds (access, metadata, etc.).
fn classify_event(event: &Event) -> Option<ChangeEvent> {
    use EventKind::*;

    // Only care about modify and create events
    let relevant = matches!(
        event.kind,
        Modify(_) | Create(_) | Remove(_)
    );

    if !relevant {
        return None;
    }

    // Use the first affected path for classification
    let path = event.paths.first()?.clone();

    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    let change = match extension {
        "css" => ChangeEvent::CssChange(path),
        "widget" => ChangeEvent::WidgetChange(path),
        "yaml" | "yml" => ChangeEvent::ConfigChange(path),
        _ => ChangeEvent::Other(path),
    };

    Some(change)
}

/// Returns the canonical path to the sender end of an existing channel,
/// suitable for constructing a standalone sender for testing.
pub fn make_test_channel() -> (Sender<ChangeEvent>, Receiver<ChangeEvent>) {
    bounded::<ChangeEvent>(64)
}
