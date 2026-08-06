//! Noticing that the configuration file changed.
//!
//! Event-driven, and here rather than in each platform SDK. The file changes a
//! handful of times in a product's life, so a timer that wakes forever to look
//! at it is waste no matter how small each wakeup is; and file watching has a
//! good cross-platform abstraction, unlike font enumeration, so writing it
//! once beats writing it per platform.
//!
//! The *directory* is watched, not the file: configuration is written by
//! renaming a temporary file over the target — by this crate and by every
//! editor worth using — so a watch bound to the file's identity would follow
//! the replaced inode and never fire again.
//!
//! Watching the file is also strictly more general than having the CLI
//! announce its own writes: it picks up `vim`, a dotfile manager, or anything
//! else that saves the file.

use crate::TerminalConfig;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Events arrive in bursts — a write, a rename, a permissions change — so a
/// change is read once the burst settles.
const SETTLE: std::time::Duration = std::time::Duration::from_millis(120);

/// Watches one app's configuration file.
pub struct ConfigWatcher {
    pub(crate) app_data_dir: PathBuf,
    product_defaults: serde_json::Value,
    stamp: Option<(SystemTime, u64)>,
    current: TerminalConfig,
}

impl ConfigWatcher {
    /// Start watching, taking the configuration already in effect.
    ///
    /// No timestamp is recorded, so the first poll reads the file and compares
    /// values: if the caller was handed something that does not match what is
    /// on disk, that is caught rather than pinned in place until the next
    /// save. When they agree — the normal case — the comparison reports
    /// nothing.
    pub fn new(
        app_data_dir: PathBuf,
        product_defaults: serde_json::Value,
        current: TerminalConfig,
    ) -> Self {
        Self {
            app_data_dir,
            product_defaults,
            stamp: None,
            current,
        }
    }

    pub fn config(&self) -> &TerminalConfig {
        &self.current
    }

    /// The new configuration, if the file changed and actually says something
    /// different.
    ///
    /// Comparing the parsed result rather than trusting the timestamp means a
    /// touched or rewritten-identical file costs nothing downstream — which
    /// matters because applying a font change reflows the grid and signals
    /// every child process.
    pub fn poll(&mut self) -> Option<TerminalConfig> {
        let path = TerminalConfig::path(&self.app_data_dir);
        let stamp = stamp_of(&path);
        if stamp == self.stamp {
            return None;
        }
        self.stamp = stamp;

        let (config, error) = TerminalConfig::load(&self.app_data_dir, &self.product_defaults);
        if let Some(error) = error {
            log::warn!("{error}; keeping the configuration already in effect");
            return None;
        }
        if config == self.current {
            return None;
        }
        self.current = config.clone();
        Some(config)
    }
}

/// Watch a configuration file, calling `on_change` with what it now says.
///
/// The callback runs on a dedicated thread and only when the parsed
/// configuration actually differs, so a touched or rewritten-identical file
/// costs nothing downstream — which matters because applying a font change
/// reflows the grid and signals every child process.
pub fn watch(
    mut watcher: ConfigWatcher,
    on_change: impl Fn(TerminalConfig) + Send + 'static,
) -> std::io::Result<()> {
    use notify::{RecursiveMode, Watcher as _};

    let directory = TerminalConfig::path(&watcher.app_data_dir)
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| std::io::Error::other("configuration path has no directory"))?;
    std::fs::create_dir_all(&directory)?;

    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("lingxia-terminal-config".to_string())
        .spawn(move || {
            let mut backend = match notify::recommended_watcher(move |event| {
                let _ = sender.send(event);
            }) {
                Ok(backend) => backend,
                Err(error) => {
                    log::warn!("terminal config watch unavailable: {error}");
                    return;
                }
            };
            if let Err(error) = backend.watch(&directory, RecursiveMode::NonRecursive) {
                log::warn!(
                    "terminal config watch failed on {}: {error}",
                    directory.display()
                );
                return;
            }
            // Held for the thread's lifetime; dropping it stops the watch.
            let _backend = backend;
            while receiver.recv().is_ok() {
                // Drain the burst before reading, so one save is one reload.
                while receiver.recv_timeout(SETTLE).is_ok() {}
                if let Some(config) = watcher.poll() {
                    on_change(config);
                }
            }
        })?;
    Ok(())
}

/// Modification time and size together: a file rewritten within a timestamp's
/// resolution usually changes length, and the pair costs the same single call.
fn stamp_of(path: &Path) -> Option<(SystemTime, u64)> {
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.modified().ok()?, metadata.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        // Atomically, the way both this crate and editors write it.
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, contents).expect("write");
        std::fs::rename(&temporary, path).expect("rename");
    }

    #[test]
    fn a_rewritten_file_is_picked_up_across_the_rename() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = TerminalConfig::path(dir.path());
        let mut watcher = ConfigWatcher::new(
            dir.path().to_path_buf(),
            serde_json::Value::Null,
            TerminalConfig::default(),
        );
        assert!(watcher.poll().is_none(), "no file, nothing to report");

        write(&path, r#"{"font":{"size":18.0}}"#);
        let updated = watcher.poll().expect("the new file is noticed");
        assert_eq!(updated.font.size, 18.0);
        assert!(watcher.poll().is_none(), "and only once");
    }

    #[test]
    fn a_file_that_changed_without_saying_anything_new_is_ignored() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = TerminalConfig::path(dir.path());
        write(&path, r#"{"font":{"size":18.0}}"#);
        let mut watcher = ConfigWatcher::new(
            dir.path().to_path_buf(),
            serde_json::Value::Null,
            TerminalConfig::default(),
        );
        assert!(watcher.poll().is_some(), "the initial contents differ");

        // Same values, different bytes and a new timestamp.
        write(&path, "{\n  \"font\": { \"size\": 18.0 }\n}\n");
        assert!(
            watcher.poll().is_none(),
            "applying a font change reflows the grid; do it only when it differs"
        );
    }

    #[test]
    fn a_broken_file_keeps_what_is_already_in_effect() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = TerminalConfig::path(dir.path());
        write(&path, r#"{"font":{"size":18.0}}"#);
        let mut watcher = ConfigWatcher::new(
            dir.path().to_path_buf(),
            serde_json::Value::Null,
            TerminalConfig::default(),
        );
        watcher.poll().expect("first load");

        write(&path, "{ half written");
        assert!(
            watcher.poll().is_none(),
            "a save in progress must not blank the terminal"
        );
        assert_eq!(watcher.config().font.size, 18.0);
    }
}
