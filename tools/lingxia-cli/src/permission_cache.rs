use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const CACHE_VERSION: u32 = 1;
pub const DEFAULT_MAX_AGE_SECONDS: i64 = 24 * 60 * 60;

#[derive(Debug, Clone, Copy)]
pub enum PermissionPlatform {
    Ios,
    Macos,
    Harmony,
}

impl PermissionPlatform {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ios => "ios",
            Self::Macos => "macos",
            Self::Harmony => "harmony",
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PermissionCacheFile {
    version: u32,
    entries: BTreeMap<String, PermissionCacheEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PermissionCacheEntry {
    updated_at: i64,
    granted_permissions: Vec<String>,
}

pub struct PermissionCache {
    path: PathBuf,
    data: PermissionCacheFile,
}

impl PermissionCache {
    pub fn load() -> Result<Self> {
        let path = cache_path()?;
        if !path.exists() {
            return Ok(Self {
                path,
                data: PermissionCacheFile {
                    version: CACHE_VERSION,
                    ..PermissionCacheFile::default()
                },
            });
        }

        let raw = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let parsed: PermissionCacheFile = serde_json::from_str(&raw)
            .with_context(|| format!("Failed to parse {}", path.display()))?;

        if parsed.version != CACHE_VERSION {
            return Ok(Self {
                path,
                data: PermissionCacheFile {
                    version: CACHE_VERSION,
                    ..PermissionCacheFile::default()
                },
            });
        }

        Ok(Self { path, data: parsed })
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(&self.data).context("Failed to serialize cache")?;
        fs::write(&self.path, json)
            .with_context(|| format!("Failed to write {}", self.path.display()))?;
        Ok(())
    }

    pub fn get(
        &self,
        platform: PermissionPlatform,
        app_id: &str,
        max_age_seconds: Option<i64>,
    ) -> Option<Vec<String>> {
        let key = cache_key(platform, app_id);
        let entry = self.data.entries.get(&key)?;
        let max_age = max_age_seconds.unwrap_or(DEFAULT_MAX_AGE_SECONDS);
        if max_age >= 0 && now_unix().saturating_sub(entry.updated_at) > max_age {
            return None;
        }
        Some(normalize_permissions(&entry.granted_permissions))
    }

    pub fn set(&mut self, platform: PermissionPlatform, app_id: &str, permissions: &[String]) {
        let key = cache_key(platform, app_id);
        self.data.entries.insert(
            key,
            PermissionCacheEntry {
                updated_at: now_unix(),
                granted_permissions: normalize_permissions(permissions),
            },
        );
    }
}

fn cache_key(platform: PermissionPlatform, app_id: &str) -> String {
    format!("{}:{}", platform.as_str(), app_id.trim())
}

fn cache_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
    Ok(home.join(".lingxia").join("permissions").join("cache.json"))
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn normalize_permissions(values: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        let normalized = trimmed.to_string();
        if seen.insert(normalized.clone()) {
            out.push(normalized);
        }
    }
    out
}
