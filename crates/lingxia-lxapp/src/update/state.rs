use super::*;
use dashmap::DashMap;
use std::sync::OnceLock;
use tokio::sync::watch;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ForceUpdateDownloadState {
    Downloading { version: String },
    Completed,
    Failed(String),
}

pub(super) struct ForceUpdateDownloadTracker {
    downloads: DashMap<String, watch::Sender<ForceUpdateDownloadState>>,
}

impl ForceUpdateDownloadTracker {
    fn new() -> Self {
        Self {
            downloads: DashMap::new(),
        }
    }

    pub(super) fn try_start_download(
        &self,
        key: &str,
        version: &str,
    ) -> Option<watch::Receiver<ForceUpdateDownloadState>> {
        use dashmap::mapref::entry::Entry;

        match self.downloads.entry(key.to_string()) {
            Entry::Occupied(_) => None,
            Entry::Vacant(entry) => {
                let initial = ForceUpdateDownloadState::Downloading {
                    version: version.to_string(),
                };
                let (tx, rx) = watch::channel(initial);
                entry.insert(tx);
                Some(rx)
            }
        }
    }

    pub(super) fn mark_completed(&self, key: &str) {
        if let Some(entry) = self.downloads.get(key) {
            let _ = entry.send(ForceUpdateDownloadState::Completed);
        }
        self.downloads.remove(key);
    }

    pub(super) fn mark_failed(&self, key: &str, error: String) {
        if let Some(entry) = self.downloads.get(key) {
            let _ = entry.send(ForceUpdateDownloadState::Failed(error));
        }
        self.downloads.remove(key);
    }

    pub(super) fn wait_for_download(
        &self,
        key: &str,
    ) -> Option<watch::Receiver<ForceUpdateDownloadState>> {
        self.downloads.get(key).map(|entry| entry.subscribe())
    }

    fn state(&self, key: &str) -> Option<ForceUpdateDownloadState> {
        self.downloads.get(key).map(|entry| entry.borrow().clone())
    }
}

static FORCE_UPDATE_DOWNLOAD_TRACKER: OnceLock<ForceUpdateDownloadTracker> = OnceLock::new();

pub(super) fn force_update_tracker() -> &'static ForceUpdateDownloadTracker {
    FORCE_UPDATE_DOWNLOAD_TRACKER.get_or_init(ForceUpdateDownloadTracker::new)
}

pub(super) fn force_update_download_key(lxappid: &str, release_type: ReleaseType) -> String {
    UpdateTarget::lxapp(
        lxappid,
        release_type,
        LxAppUpdateQuery::latest(None::<String>),
    )
    .scope_key()
}

pub fn is_force_update_downloading(lxappid: &str, release_type: ReleaseType) -> bool {
    matches!(
        force_update_tracker().state(&force_update_download_key(lxappid, release_type)),
        Some(ForceUpdateDownloadState::Downloading { .. })
    )
}
