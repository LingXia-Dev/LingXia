use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub name: String,
    pub status: CheckStatus,
    pub detail: String,
    pub hint: Option<String>,
}

impl CheckResult {
    pub fn pass(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Pass,
            detail: detail.into(),
            hint: None,
        }
    }

    pub fn warn(
        name: impl Into<String>,
        detail: impl Into<String>,
        hint: Option<impl Into<String>>,
    ) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Warn,
            detail: detail.into(),
            hint: hint.map(|h| h.into()),
        }
    }

    pub fn fail(
        name: impl Into<String>,
        detail: impl Into<String>,
        hint: Option<impl Into<String>>,
    ) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Fail,
            detail: detail.into(),
            hint: hint.map(|h| h.into()),
        }
    }
}

pub fn command_exists(cmd: &str) -> bool {
    which::which(cmd).is_ok()
}

pub fn command_version_line(cmd: &str, args: &[&str], prefer_stderr: bool) -> Option<String> {
    let output = Command::new(cmd).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let primary = if prefer_stderr {
        String::from_utf8_lossy(&output.stderr).to_string()
    } else {
        String::from_utf8_lossy(&output.stdout).to_string()
    };
    let fallback = if prefer_stderr {
        String::from_utf8_lossy(&output.stdout).to_string()
    } else {
        String::from_utf8_lossy(&output.stderr).to_string()
    };

    primary
        .lines()
        .find(|l| !l.trim().is_empty())
        .or_else(|| fallback.lines().find(|l| !l.trim().is_empty()))
        .map(|l| l.trim().to_string())
}

pub fn command_output_line(cmd: &str, args: &[&str], prefer_stderr: bool) -> Option<String> {
    command_version_line(cmd, args, prefer_stderr)
}
