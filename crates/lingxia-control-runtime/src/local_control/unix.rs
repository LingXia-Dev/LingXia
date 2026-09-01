//! Unix domain socket, in LingXia-owned runtime state.

use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

pub(super) type Stream = UnixStream;

/// Where the endpoint lives, so a client can find it without being told.
/// A string on both platforms: what a client passes to `connect` is a path
/// here and a kernel name on Windows, and no shared type covers both.
pub fn endpoint_name(control_dir: &Path) -> String {
    lingxia_control_protocol::local_control::endpoint(
        control_dir,
        super::EPOCH.load(std::sync::atomic::Ordering::SeqCst),
    )
}

fn socket_path(control_dir: &Path) -> PathBuf {
    PathBuf::from(endpoint_name(control_dir))
}

pub(super) fn split_writer(stream: &Stream) -> std::io::Result<Stream> {
    stream.try_clone()
}

/// Remove a socket file left by a process that was killed. Turning the
/// capability off should leave nothing that looks like it is still on.
pub(super) fn clear_stale(control_dir: &Path) {
    let _ = std::fs::remove_file(socket_path(control_dir));
}

/// Unblock a pending `accept` by connecting to it once.
pub(super) fn poke(endpoint: &str) {
    let _ = UnixStream::connect(endpoint);
}

pub(super) struct Listener {
    inner: UnixListener,
    path: PathBuf,
}

impl Listener {
    pub(super) fn bind(control_dir: &Path, _epoch: u64) -> std::io::Result<Self> {
        let path = socket_path(control_dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
            // The directory carries the real guarantee: a socket's own mode is
            // not honoured on every Unix, but nobody traverses a 0700 parent.
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
        }
        // A socket file outlives the process that made it, so a crash leaves
        // one behind and `bind` would fail forever. Removing first is safe:
        // a live server holds no lock on the path, and a second instance of
        // the same app is a bug the pid guard catches, not this.
        let _ = std::fs::remove_file(&path);
        let inner = UnixListener::bind(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        Ok(Self { inner, path })
    }

    pub(super) fn name(&self) -> String {
        self.path.display().to_string()
    }

    pub(super) fn accept(
        &self,
        _listening: &std::sync::atomic::AtomicBool,
    ) -> std::io::Result<Stream> {
        let (stream, _) = self.inner.accept()?;
        Ok(stream)
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
