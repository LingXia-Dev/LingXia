use super::{AgcConnectClient, AgcCredentialStorage};
use crate::permission_cache::{PermissionCache, PermissionPlatform};
use anyhow::{Context, Result};
use colored::Colorize;
use std::collections::HashSet;

const RESTRICTED_HARMONY_ACL_PERMISSIONS: &[&str] = &["ohos.permission.WRITE_IMAGEVIDEO"];

#[derive(Debug, Clone)]
pub struct AclPermissionResolution {
    pub effective_permissions: Vec<String>,
    pub missing_permissions: Vec<String>,
    pub can_sync_managed_permissions: bool,
}

pub fn controlled_harmony_acl_permissions() -> impl Iterator<Item = &'static str> {
    RESTRICTED_HARMONY_ACL_PERMISSIONS.iter().copied()
}

pub fn resolve_effective_acl_permissions(package_name: &str) -> AclPermissionResolution {
    let required_permissions = controlled_harmony_acl_permissions()
        .map(str::to_string)
        .collect::<Vec<_>>();
    if required_permissions.is_empty() {
        return AclPermissionResolution {
            effective_permissions: Vec::new(),
            missing_permissions: Vec::new(),
            can_sync_managed_permissions: true,
        };
    }

    let mut granted_permissions = Vec::new();
    let mut can_sync_managed_permissions = true;

    if let Some(cached_permissions) = load_cached_harmony_acl_permissions(package_name) {
        // By default, trust local cache and avoid AGC lookups.
        granted_permissions = cached_permissions;
    } else {
        match fetch_harmony_acl_permissions_from_agc(package_name) {
            Ok(Some(fetched)) => {
                granted_permissions = fetched.clone();
                persist_harmony_acl_permissions_cache(package_name, &fetched);
            }
            Ok(None) => {
                can_sync_managed_permissions = false;
                eprintln!(
                    "{} Harmony ACL approvals are not verified yet (no AGC credentials and no cached approvals).",
                    "Warning:".yellow()
                );
            }
            Err(err) => {
                can_sync_managed_permissions = false;
                eprintln!(
                    "{} Failed to verify Harmony ACL approvals from AGC: {}",
                    "Warning:".yellow(),
                    err
                );
            }
        }
    }

    let granted_set = granted_permissions
        .iter()
        .map(|entry| entry.trim())
        .filter(|entry| !entry.is_empty())
        .collect::<HashSet<_>>();

    let mut effective_permissions = Vec::new();
    let mut missing_permissions = Vec::new();
    for permission in required_permissions {
        if granted_set.contains(permission.as_str()) {
            effective_permissions.push(permission);
        } else {
            missing_permissions.push(permission);
        }
    }

    AclPermissionResolution {
        effective_permissions,
        missing_permissions,
        can_sync_managed_permissions,
    }
}

fn load_cached_harmony_acl_permissions(package_name: &str) -> Option<Vec<String>> {
    let Ok(cache) = PermissionCache::load() else {
        return None;
    };
    // Negative max age means "never expire" in PermissionCache::get.
    cache.get(PermissionPlatform::Harmony, package_name, Some(-1))
}

fn persist_harmony_acl_permissions_cache(package_name: &str, permissions: &[String]) {
    let Ok(mut cache) = PermissionCache::load() else {
        return;
    };
    cache.set(PermissionPlatform::Harmony, package_name, permissions);
    let _ = cache.save();
}

fn fetch_harmony_acl_permissions_from_agc(package_name: &str) -> Result<Option<Vec<String>>> {
    let storage = AgcCredentialStorage::new()?;
    let Some(mut credentials) = storage.load()? else {
        return Ok(None);
    };

    let client = AgcConnectClient::new();
    let token = client.ensure_valid_token(&credentials)?;
    let changed = credentials.token.as_ref().is_none_or(|old| {
        old.access_token != token.access_token || old.expires_at != token.expires_at
    });
    if changed {
        credentials.token = Some(token.clone());
        storage
            .save(&credentials)
            .context("Failed to persist refreshed AGC token")?;
    }

    let Some(app) = client.find_app_id_by_package_name(&token, package_name)? else {
        return Ok(Some(Vec::new()));
    };

    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for provision_type in [1, 2] {
        let profiles = client.query_profiles(&token, provision_type, Some(&app.app_id))?;
        for profile in profiles {
            for permission in profile.acl_permissions {
                let trimmed = permission.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let normalized = trimmed.to_string();
                if seen.insert(normalized.clone()) {
                    out.push(normalized);
                }
            }
        }
    }

    Ok(Some(out))
}
