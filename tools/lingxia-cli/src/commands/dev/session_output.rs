//! A dev session's own build output and published store, which no other session reads or
//! writes. The project's `dist/` is left to `lingxia build`.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

const SESSIONS_DIR: &str = "sessions";
const OWNER_FILE: &str = "owner";
const BUILD_DIR: &str = "build";
const STORE_DIR: &str = "lxapp";

/// `<project>/.lingxia/dev/sessions/<session id>/`.
#[derive(Debug, Clone)]
pub(crate) struct SessionOutput {
    root: PathBuf,
}

impl SessionOutput {
    /// Claim this session's directory, after removing those of sessions proven gone.
    /// Returns the directories left because nothing proves their owner gone.
    pub(crate) fn create(project_root: &Path, session_id: &str) -> Result<(Self, Vec<PathBuf>)> {
        let sessions = sessions_dir(project_root);
        let kept = sweep(&sessions);
        let root = sessions.join(session_id);
        fs::create_dir_all(&root)
            .with_context(|| format!("Failed to create {}", root.display()))?;
        let pid = std::process::id();
        let owner = format!("{pid} {}", start_time(pid).unwrap_or(0));
        let temporary = root.join(format!("{OWNER_FILE}.tmp"));
        fs::write(&temporary, owner)
            .and_then(|()| fs::rename(&temporary, root.join(OWNER_FILE)))
            .with_context(|| format!("Failed to write the owner of {}", root.display()))?;
        Ok((Self { root }, kept))
    }

    #[cfg(test)]
    pub(crate) fn at(root: PathBuf) -> Self {
        Self { root }
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// Where this session builds `app_id`.
    pub(crate) fn build_dir(&self, app_id: &str) -> PathBuf {
        self.root.join(BUILD_DIR).join(file_safe(app_id))
    }

    /// The manifests, snapshots and blobs this session serves.
    pub(crate) fn store(&self) -> PathBuf {
        self.root.join(STORE_DIR)
    }

    /// Start `app_id`'s build from a copy of `dist`, which the host app embedded.
    pub(crate) fn seed(&self, app_id: &str, dist: &Path) -> Result<PathBuf> {
        let target = self.build_dir(app_id);
        let staging = target.with_file_name(format!(".{}.seeding", file_safe(app_id)));
        let _ = fs::remove_dir_all(&staging);
        copy_tree(dist, &staging)
            .with_context(|| format!("Failed to copy {} for this session", dist.display()))?;
        let _ = fs::remove_dir_all(&target);
        fs::rename(&staging, &target)
            .with_context(|| format!("Failed to publish {}", target.display()))?;
        Ok(target)
    }

    pub(crate) fn remove_app(&self, app_id: &str) {
        let _ = fs::remove_dir_all(self.build_dir(app_id));
    }
}

/// Removes the session's directory when the session ends.
pub(crate) struct RemoveOnDrop(pub(crate) SessionOutput);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0.root);
    }
}

fn sessions_dir(project_root: &Path) -> PathBuf {
    super::log_store::dev_dir(project_root)
        .join("dev")
        .join(SESSIONS_DIR)
}

#[derive(Debug, PartialEq, Eq)]
enum Owner {
    Alive,
    Gone,
    /// No readable owner record, or a live pid whose start time cannot be read.
    Unknown,
}

/// Removes each session directory whose owner is proven gone; returns those left unproven.
fn sweep(sessions: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(sessions) else {
        return Vec::new();
    };
    let mut kept = Vec::new();
    for entry in entries.filter_map(|entry| entry.ok()) {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        match owner(&dir) {
            Owner::Gone => {
                let _ = fs::remove_dir_all(&dir);
            }
            Owner::Alive => {}
            Owner::Unknown => kept.push(dir),
        }
    }
    kept.sort();
    kept
}

fn owner(dir: &Path) -> Owner {
    let Ok(text) = fs::read_to_string(dir.join(OWNER_FILE)) else {
        return Owner::Unknown;
    };
    let mut parts = text.split_whitespace();
    let (Some(Ok(pid)), Some(Ok(started))) = (
        parts.next().map(str::parse::<u32>),
        parts.next().map(str::parse::<u64>),
    ) else {
        return Owner::Unknown;
    };
    owner_state(pid, started)
}

/// Gone only when the pid is not running, or runs with another start time (a reused pid).
fn owner_state(pid: u32, started: u64) -> Owner {
    let id = sysinfo::Pid::from_u32(pid);
    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[id]), true);
    let Some(process) = system.process(id) else {
        return Owner::Gone;
    };
    match process.start_time() {
        0 => Owner::Unknown,
        now if started != 0 && now != started => Owner::Gone,
        _ if started == 0 => Owner::Unknown,
        _ => Owner::Alive,
    }
}

fn start_time(pid: u32) -> Option<u64> {
    let id = sysinfo::Pid::from_u32(pid);
    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[id]), true);
    system
        .process(id)
        .map(|process| process.start_time())
        .filter(|time| *time > 0)
}

fn file_safe(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => ch,
            _ => '_',
        })
        .collect()
}

fn copy_tree(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let to = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_dir(project: &Path, id: &str, owner: Option<String>) -> PathBuf {
        let dir = sessions_dir(project).join(id);
        fs::create_dir_all(dir.join("build/app")).unwrap();
        if let Some(owner) = owner {
            fs::write(dir.join(OWNER_FILE), owner).unwrap();
        }
        dir
    }

    /// The pid of a process that has exited and been reaped.
    fn exited_pid() -> u32 {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--list")
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let pid = child.id();
        child.wait().unwrap();
        pid
    }

    #[test]
    fn a_session_whose_owner_is_gone_is_removed() {
        let project = tempfile::tempdir().unwrap();
        let exited = session_dir(
            project.path(),
            "aaaaaa",
            Some(format!("{} 1", exited_pid())),
        );
        // This test's own pid with another start time: a reused pid.
        let reused = session_dir(
            project.path(),
            "bbbbbb",
            Some(format!("{} 1", std::process::id())),
        );
        let (output, kept) = SessionOutput::create(project.path(), "cccccc").unwrap();
        assert!(!exited.exists());
        assert!(!reused.exists());
        assert!(kept.is_empty());
        assert!(output.root().join(OWNER_FILE).is_file());
    }

    #[test]
    fn a_session_whose_owner_lives_is_kept() {
        let project = tempfile::tempdir().unwrap();
        let pid = std::process::id();
        let live = session_dir(
            project.path(),
            "aaaaaa",
            Some(format!("{pid} {}", start_time(pid).unwrap())),
        );
        let (_, kept) = SessionOutput::create(project.path(), "bbbbbb").unwrap();
        assert!(live.join("build/app").is_dir());
        assert!(kept.is_empty());
    }

    #[test]
    fn a_session_without_an_owner_record_is_kept_and_reported() {
        let project = tempfile::tempdir().unwrap();
        let unowned = session_dir(project.path(), "aaaaaa", None);
        let unreadable = session_dir(project.path(), "bbbbbb", Some("not an owner".into()));
        let (_, kept) = SessionOutput::create(project.path(), "cccccc").unwrap();
        assert!(unowned.join("build/app").is_dir());
        assert!(unreadable.join("build/app").is_dir());
        assert_eq!(kept, vec![unowned, unreadable]);
    }

    #[test]
    fn a_session_removes_its_directory_when_it_ends() {
        let project = tempfile::tempdir().unwrap();
        let (output, _) = SessionOutput::create(project.path(), "aaaaaa").unwrap();
        let root = output.root().to_path_buf();
        drop(RemoveOnDrop(output));
        assert!(!root.exists());
        assert!(sessions_dir(project.path()).is_dir());
    }

    #[test]
    fn a_seed_copies_dist_and_leaves_it() {
        let project = tempfile::tempdir().unwrap();
        let dist = project.path().join("lxapp/dist");
        fs::create_dir_all(dist.join("pages")).unwrap();
        fs::write(dist.join("pages/a.js"), "embedded").unwrap();
        let (output, _) = SessionOutput::create(project.path(), "aaaaaa").unwrap();
        let built = output.seed("app.one", &dist).unwrap();
        assert_eq!(built, output.build_dir("app.one"));
        assert_eq!(
            fs::read_to_string(built.join("pages/a.js")).unwrap(),
            "embedded"
        );
        assert_eq!(
            fs::read_to_string(dist.join("pages/a.js")).unwrap(),
            "embedded"
        );
    }
}
