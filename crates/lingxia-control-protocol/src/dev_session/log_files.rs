//! A dev session's log on disk: `.lingxia/logs/{session}.jsonl`, rotated.
//!
//! The writer (`lingxia dev`) caps the file at [`MAX_LOG_BYTES`]: past it the
//! file becomes `{session}.1.jsonl`, the previous `.1` becomes `.2`, and at
//! most [`ROTATED_LOGS`] rotated files are kept. Readers (`lxdev logs`) read
//! [`session_log_files`] oldest first.

use std::path::{Path, PathBuf};

/// Largest a session's current log file grows before it rotates.
pub const MAX_LOG_BYTES: u64 = 8 * 1024 * 1024;
/// Rotated files kept per session, besides the current one.
pub const ROTATED_LOGS: usize = 2;

/// `{session}.{n}.jsonl` beside `log_file` (`{session}.jsonl`); `n` from 1,
/// the most recent rotation.
pub fn rotated_log_path(log_file: &Path, n: usize) -> PathBuf {
    let stem = log_file
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    log_file.with_file_name(format!("{stem}.{n}.jsonl"))
}

/// The session's log files that exist, oldest first: the rotated ones, then
/// the current one.
pub fn session_log_files(log_file: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = (1..=ROTATED_LOGS)
        .rev()
        .map(|n| rotated_log_path(log_file, n))
        .filter(|path| path.is_file())
        .collect();
    if log_file.is_file() {
        files.push(log_file.to_path_buf());
    }
    files
}

/// The session a file under `.lingxia/logs/` belongs to: `abc123` for
/// `abc123.jsonl` and `abc123.2.jsonl`; `None` for anything else.
pub fn log_session_id(file_name: &str) -> Option<&str> {
    let stem = file_name.strip_suffix(".jsonl")?;
    let session = match stem.split_once('.') {
        Some((session, n)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => session,
        Some(_) => return None,
        None => stem,
    };
    (!session.is_empty()).then_some(session)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotated_files_sit_beside_the_current_one_and_read_oldest_first() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("abc123.jsonl");
        assert_eq!(rotated_log_path(&log, 2), dir.path().join("abc123.2.jsonl"));
        assert!(session_log_files(&log).is_empty());
        for name in [
            "abc123.jsonl",
            "abc123.1.jsonl",
            "abc123.2.jsonl",
            "other.jsonl",
        ] {
            std::fs::write(dir.path().join(name), "").unwrap();
        }
        assert_eq!(
            session_log_files(&log),
            vec![
                dir.path().join("abc123.2.jsonl"),
                dir.path().join("abc123.1.jsonl"),
                log.clone(),
            ]
        );
    }

    #[test]
    fn a_log_file_names_its_session() {
        assert_eq!(log_session_id("abc123.jsonl"), Some("abc123"));
        assert_eq!(log_session_id("abc123.2.jsonl"), Some("abc123"));
        assert_eq!(log_session_id("abc123.x.jsonl"), None);
        assert_eq!(log_session_id("abc123.log"), None);
        assert_eq!(log_session_id(".jsonl"), None);
    }
}
