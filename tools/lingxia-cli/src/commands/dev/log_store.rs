use anyhow::{Context, Result, anyhow};
use lingxia_observability::now_timestamp_ms;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use uuid::Uuid;

pub const DEFAULT_LOG_RETENTION_DAYS: u64 = 7;
pub const DEV_DIR_NAME: &str = ".lingxia";
pub const DEV_INFO_FILE_NAME: &str = "dev.json";

#[derive(Debug, Clone)]
pub struct DevLogSession {
    pub session_id: String,
    pub dev_dir: PathBuf,
    pub log_file: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevInfo {
    pub version: u32,
    pub session_id: String,
    pub ws_url: String,
    pub log_file: String,
}

pub fn dev_dir(project_root: &Path) -> PathBuf {
    project_root.join(DEV_DIR_NAME)
}

pub fn dev_info_path(project_root: &Path) -> PathBuf {
    dev_dir(project_root).join(DEV_INFO_FILE_NAME)
}

pub fn create_session(project_root: &Path) -> Result<DevLogSession> {
    let dev_dir = dev_dir(project_root);
    let logs_dir = dev_dir.join("logs");
    cleanup_old_logs(&logs_dir, DEFAULT_LOG_RETENTION_DAYS)?;
    fs::create_dir_all(&logs_dir)
        .with_context(|| format!("Failed to create {}", logs_dir.display()))?;

    let session_id = format!("{}-{}", now_timestamp_ms(), Uuid::new_v4().simple());
    Ok(DevLogSession {
        session_id: session_id.clone(),
        dev_dir,
        log_file: logs_dir.join(format!("{session_id}.jsonl")),
    })
}

pub fn write_dev_info(project_root: &Path, session: &DevLogSession, ws_url: &str) -> Result<()> {
    fs::create_dir_all(&session.dev_dir)
        .with_context(|| format!("Failed to create {}", session.dev_dir.display()))?;
    let info = DevInfo {
        version: 1,
        session_id: session.session_id.clone(),
        ws_url: ws_url.to_string(),
        log_file: session.log_file.display().to_string(),
    };
    let bytes = serde_json::to_vec_pretty(&info).context("Failed to encode dev info")?;
    let path = dev_info_path(project_root);
    fs::write(&path, bytes).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

pub fn remove_dev_info(project_root: &Path) -> Result<()> {
    let path = dev_info_path(project_root);
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("Failed to remove {}", path.display()))?;
    }
    Ok(())
}

pub fn cleanup_old_logs(logs_dir: &Path, retention_days: u64) -> Result<()> {
    if retention_days == 0 || !logs_dir.exists() {
        return Ok(());
    }

    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(retention_days.saturating_mul(86_400)))
        .ok_or_else(|| anyhow!("Failed to compute log retention cutoff"))?;
    for entry in
        fs::read_dir(logs_dir).with_context(|| format!("Failed to read {}", logs_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;
        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        if modified < cutoff && metadata.is_file() {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn creates_project_local_dev_paths() {
        let temp = tempdir().unwrap();
        let session = create_session(temp.path()).unwrap();
        assert!(session.dev_dir.ends_with(".lingxia"));
        assert!(
            session
                .log_file
                .to_string_lossy()
                .contains("/.lingxia/logs/")
        );
    }

    #[test]
    fn cleanup_old_logs_removes_expired_entries_only() {
        let temp = tempdir().unwrap();
        let logs_dir = temp.path().join("logs");
        fs::create_dir_all(&logs_dir).unwrap();

        let old_log = logs_dir.join("old.jsonl");
        let new_log = logs_dir.join("new.jsonl");
        fs::write(&old_log, "old").unwrap();
        fs::write(&new_log, "new").unwrap();

        filetime::set_file_mtime(
            &old_log,
            filetime::FileTime::from_system_time(
                SystemTime::now() - Duration::from_secs(10 * 86_400),
            ),
        )
        .unwrap();

        cleanup_old_logs(&logs_dir, 7).unwrap();

        assert!(!old_log.exists());
        assert!(new_log.exists());
    }
}
