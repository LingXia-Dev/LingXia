//! Manual control over an update the CLI otherwise performs on its own.
//!
//! The automatic path checks once a day and only when the CLI sits where
//! `install.sh` puts it. That leaves no way to update on demand, to see what a
//! newer release would be without taking it, to move to a specific version, or
//! to learn why nothing is happening when the install does not qualify.

use crate::update::{self, UpdateStatus};
use anyhow::{Context, Result};
use colored::Colorize;
use semver::Version;

/// Exit code for `--check` when a newer release exists, so a script can branch
/// on it without parsing output.
const EXIT_UPDATE_AVAILABLE: i32 = 10;

/// The installer the README points at, so the fallback this prints is the same
/// command a user would have run to install in the first place.
const INSTALL_SCRIPT_URL: &str =
    "https://raw.githubusercontent.com/LingXia-Dev/LingXia/main/install.sh";

pub fn execute(check: bool, version: Option<String>, cli_only: bool) -> Result<i32> {
    // Inside a project (unless --cli or an explicit CLI version is asked for),
    // `upgrade` means: bring the project's pinned LingXia versions to this
    // CLI's line. The CLI itself already self-updates daily.
    if !cli_only
        && version.is_none()
        && let Some(root) = crate::commands::project_upgrade::find_project_root()
    {
        return crate::commands::project_upgrade::execute(&root, check);
    }

    let exe_path = update::current_exe_path()?
        .canonicalize()
        .context("Failed to resolve the running executable")?;

    let status = match &version {
        Some(target) => pinned_status(target)?,
        None => {
            update::load_update_status(true).context("Failed to check for a newer CLI release")?
        }
    };

    report(&status, &version);

    if check {
        return Ok(if status.update_available {
            EXIT_UPDATE_AVAILABLE
        } else {
            0
        });
    }
    if !status.update_available {
        return Ok(0);
    }

    // An install this CLI cannot replace in place still deserves an answer.
    if !update::is_install_sh_install(&exe_path) {
        println!();
        println!(
            "{} This CLI is not where install.sh puts it ({}), so it cannot replace itself.",
            "!".yellow(),
            exe_path.display()
        );
        println!("  Re-run the installer, pinning the version you want:");
        println!(
            "    {}",
            format!(
                "curl -fsSL {INSTALL_SCRIPT_URL} | LINGXIA_VERSION={} sh",
                status.latest_version
            )
            .cyan()
        );
        return Ok(1);
    }

    println!();
    println!(
        "Updating LingXia CLI {} -> {}...",
        status.current_version, status.latest_version
    );
    update::install_update(&exe_path, &status)?;
    println!(
        "{} lingxia, lxdev and the Runner are now {}.",
        "✓".green(),
        status.latest_version
    );
    println!("  The installed agent skill refreshes on the next run.");
    Ok(0)
}

/// Build a status for an explicitly requested version, which may be older than
/// the running one -- pinning a known-good release is the reason to ask.
fn pinned_status(target: &str) -> Result<UpdateStatus> {
    let requested = Version::parse(target)
        .with_context(|| format!("Not a version: {target} (expected x.y.z)"))?;
    let current = Version::parse(env!("CARGO_PKG_VERSION"))
        .context("Failed to parse the current CLI version")?;
    if requested == current {
        return Ok(UpdateStatus {
            current_version: current.clone(),
            latest_version: requested,
            latest_tag: String::new(),
            release_repo: update::release_repo_for_current_install(),
            update_available: false,
        });
    }
    Ok(UpdateStatus {
        current_version: current,
        latest_version: requested.clone(),
        latest_tag: format!("lingxia-cli-v{requested}"),
        release_repo: update::release_repo_for_current_install(),
        update_available: true,
    })
}

fn report(status: &UpdateStatus, pinned: &Option<String>) {
    println!("{}", "LingXia CLI".bold());
    println!("  installed  {}", status.current_version);
    match pinned {
        Some(_) if !status.update_available => {
            println!("  requested  {} (already installed)", status.latest_version);
        }
        Some(_) => {
            let direction = if status.latest_version < status.current_version {
                " (downgrade)"
            } else {
                ""
            };
            println!("  requested  {}{direction}", status.latest_version);
        }
        None if status.update_available => {
            println!("  available  {}", status.latest_version.to_string().green());
        }
        None => println!("  {}", "up to date".green()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinning_the_running_version_is_not_an_update() {
        let status = pinned_status(env!("CARGO_PKG_VERSION")).expect("current version parses");
        assert!(!status.update_available);
    }

    #[test]
    fn pinning_an_older_version_is_an_update_to_perform() {
        // Downgrading to a known-good release is a deliberate request, not a
        // no-op: the automatic path only ever moves forward.
        let status = pinned_status("0.1.0").expect("0.1.0 parses");
        assert!(status.update_available);
        assert_eq!(status.latest_tag, "lingxia-cli-v0.1.0");
    }

    #[test]
    fn a_malformed_version_is_rejected_before_any_download() {
        assert!(pinned_status("latest").is_err());
    }

    /// `--check` is meant for scripts, so the two outcomes must stay
    /// distinguishable without reading the printed text.
    #[test]
    fn check_reports_availability_through_the_exit_code() {
        assert_ne!(EXIT_UPDATE_AVAILABLE, 0);
        let current = pinned_status(env!("CARGO_PKG_VERSION")).expect("current parses");
        let code = if current.update_available {
            EXIT_UPDATE_AVAILABLE
        } else {
            0
        };
        assert_eq!(code, 0, "the running version must not report an update");
    }
}
