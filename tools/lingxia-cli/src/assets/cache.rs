use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

pub(super) const CACHE_VERSION: u32 = 4;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(super) struct DestinationStamp {
    pub(super) app_json_hash: String,
    pub(super) ui_json_hash: Option<String>,
    pub(super) bundle_hashes: BTreeMap<String, String>,
    #[serde(default)]
    pub(super) app_ui_icon_hashes: BTreeMap<String, String>,
    pub(super) runtime_hash: Option<String>,
    #[serde(default)]
    pub(super) polyfills_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct LxAppBuildStamp {
    pub(super) inputs_hash: String,
    pub(super) dist_hash: String,
    pub(super) asset_name: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(super) struct HostAssetsCache {
    version: u32,
    pub(super) lxapp_builds: HashMap<String, LxAppBuildStamp>,
    pub(super) destinations: HashMap<String, DestinationStamp>,
}

impl HostAssetsCache {
    pub(super) fn load(project_root: &Path) -> Self {
        let path = cache_path(project_root);
        let data = match fs::read(&path) {
            Ok(d) => d,
            Err(_) => return Self::default_v1(),
        };
        match serde_json::from_slice::<HostAssetsCache>(&data) {
            Ok(cache) if cache.version == CACHE_VERSION => cache,
            _ => Self::default_v1(),
        }
    }

    pub(super) fn save(&mut self, project_root: &Path) -> Result<()> {
        let path = cache_path(project_root);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        self.version = CACHE_VERSION;
        fs::write(path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    fn default_v1() -> Self {
        Self {
            version: CACHE_VERSION,
            lxapp_builds: HashMap::new(),
            destinations: HashMap::new(),
        }
    }
}

pub(super) fn cache_path(project_root: &Path) -> PathBuf {
    project_root
        .join(".lingxia")
        .join("host-assets")
        .join("cache.json")
}
