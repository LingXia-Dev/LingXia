//! Manual control over an update the CLI otherwise performs on its own.
//!
//! The automatic path checks once a day and only when the CLI sits where
//! `install.sh` puts it. That leaves no way to update on demand, to see what a
//! newer release would be without taking it, to move to a specific version, or
//! to learn why nothing is happening when the install does not qualify.
//!
//! Always upgrades the CLI. Inside a project it then compares the project's
//! LingXia line (npm / crate / SDK pins, major.minor) with this CLI. A newer
//! line is offered as a prompt — never applied silently.

use crate::update::{self, SelfReplace, UpdateStatus};
use anyhow::{Context, Result};
use colored::Colorize;
use semver::Version;
#[cfg(not(target_os = "windows"))]
use std::path::{Path, PathBuf};

/// Exit code for `--check` when a newer release exists, so a script can branch
/// on it without parsing output.
const EXIT_UPDATE_AVAILABLE: i32 = 10;

/// The installer the README points at, so the fallback this prints is the same
/// command a user would have run to install in the first place.
const INSTALL_SCRIPT_URL: &str =
    "https://raw.githubusercontent.com/LingXia-Dev/LingXia/main/install.sh";

/// What the CLI half of `upgrade` did, so the project half can decide whether
/// to re-exec (new binary on disk) or keep going with this process.
enum CliStep {
    UpToDate,
    /// `--check` only: a newer CLI exists.
    Available,
    /// The release service could not be queried, so the CLI state is unknown.
    Unavailable,
    #[cfg(not(target_os = "windows"))]
    Replaced {
        exe: PathBuf,
    },
    #[cfg(target_os = "windows")]
    Staged,
    NotReplaceable,
}

pub fn execute(check: bool, version: Option<String>, yes: bool) -> Result<i32> {
    // CLI first, always. The new CLI's baked compatibility line is what the
    // project half compares against, so a successful self-replace re-execs on
    // Unix. Windows stages the swap until this process exits — project pins
    // are compared to *this* CLI's line, and the user re-runs after the new
    // binary is in place for a newer line.
    let project_root = crate::commands::project_upgrade::find_project_root();

    let cli = run_cli_step(check, version.as_deref(), project_root.is_some())?;

    if should_refresh_skill(&cli, check) {
        crate::update::refresh_installed_skill();
    }

    if let Some(root) = project_root.as_deref() {
        #[cfg(not(target_os = "windows"))]
        if let CliStep::Replaced { exe } = &cli
            && !check
        {
            return reexec_upgraded_cli(exe);
        }
        #[cfg(target_os = "windows")]
        if should_defer_project_upgrade(&cli, check) {
            println!();
            println!(
                "{} CLI update is staged and installs after this command exits.",
                "!".yellow()
            );
            println!(
                "  Re-run `{}` once the new CLI is in place so project pins follow that release.",
                rerun_command(version.as_deref(), yes)
            );
            return Ok(deferred_project_exit());
        }
        if blocks_project_upgrade(&cli) {
            return Ok(cli_exit(&cli));
        }
        let project = crate::commands::project_upgrade::execute(root, check, yes)?;
        return Ok(combine_exit(check, &cli, project));
    }

    Ok(cli_exit(&cli))
}

#[cfg(target_os = "windows")]
fn should_defer_project_upgrade(cli: &CliStep, check: bool) -> bool {
    matches!(cli, CliStep::Staged) && !check
}

#[cfg(any(target_os = "windows", test))]
fn deferred_project_exit() -> i32 {
    EXIT_UPDATE_AVAILABLE
}

#[cfg(any(target_os = "windows", test))]
fn rerun_command(version: Option<&str>, yes: bool) -> String {
    let mut command = "lingxia upgrade".to_string();
    if let Some(version) = version {
        command.push_str(" --version ");
        command.push_str(version);
    }
    if yes {
        command.push_str(" --yes");
    }
    command
}

fn should_refresh_skill(cli: &CliStep, check: bool) -> bool {
    !check && matches!(cli, CliStep::UpToDate)
}

fn blocks_project_upgrade(cli: &CliStep) -> bool {
    matches!(cli, CliStep::NotReplaceable)
}

fn run_cli_step(check: bool, version: Option<&str>, in_project: bool) -> Result<CliStep> {
    let exe_path = update::current_exe_path()?
        .canonicalize()
        .context("Failed to resolve the running executable")?;

    let pinned = version.map(str::to_string);
    let status = match version {
        Some(target) => pinned_status(target)?,
        None => match update::load_update_status(true) {
            Ok(status) => status,
            Err(err) if in_project => {
                eprintln!("{} Could not check for a CLI update: {err}", "!".yellow());
                eprintln!("  Continuing; will still check this project's LingXia line.");
                return Ok(CliStep::Unavailable);
            }
            Err(err) => {
                return Err(err).context("Failed to check for a newer CLI release");
            }
        },
    };

    report(&status, &pinned);

    if check {
        return Ok(if status.update_available {
            CliStep::Available
        } else {
            CliStep::UpToDate
        });
    }
    if !status.update_available {
        return Ok(CliStep::UpToDate);
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
        return Ok(CliStep::NotReplaceable);
    }

    println!();
    println!(
        "Updating LingXia CLI {} -> {}...",
        status.current_version, status.latest_version
    );
    let kind = update::install_update(&exe_path, &status)?;
    println!(
        "{} lingxia, lxdev and the Runner are now {}.",
        "✓".green(),
        status.latest_version
    );
    match kind {
        #[cfg(not(target_os = "windows"))]
        SelfReplace::Complete => {
            println!("  The installed agent skill refreshes on the next run of this CLI.");
            Ok(CliStep::Replaced { exe: exe_path })
        }
        #[cfg(target_os = "windows")]
        SelfReplace::Deferred => Ok(CliStep::Staged),
    }
}

#[cfg(not(target_os = "windows"))]
fn reexec_upgraded_cli(exe: &Path) -> Result<i32> {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    println!();
    println!("Re-running the new CLI to upgrade this project…");
    let status = std::process::Command::new(exe)
        .args(&args)
        .status()
        .context("Failed to re-run the upgraded CLI")?;
    Ok(status.code().unwrap_or(1))
}

fn cli_exit(cli: &CliStep) -> i32 {
    match cli {
        CliStep::Available => EXIT_UPDATE_AVAILABLE,
        CliStep::Unavailable => 1,
        CliStep::NotReplaceable => 1,
        CliStep::UpToDate => 0,
        #[cfg(not(target_os = "windows"))]
        CliStep::Replaced { .. } => 0,
        #[cfg(target_os = "windows")]
        CliStep::Staged => 0,
    }
}

fn combine_exit(check: bool, cli: &CliStep, project: i32) -> i32 {
    if matches!(cli, CliStep::Unavailable | CliStep::NotReplaceable) {
        1
    } else if check {
        if matches!(cli, CliStep::Available) || project == EXIT_UPDATE_AVAILABLE {
            EXIT_UPDATE_AVAILABLE
        } else {
            0
        }
    } else if matches!(cli, CliStep::NotReplaceable) {
        1
    } else {
        project
    }
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

    #[test]
    fn check_exit_is_10_if_either_half_is_behind() {
        assert_eq!(
            combine_exit(true, &CliStep::Available, 0),
            EXIT_UPDATE_AVAILABLE
        );
        assert_eq!(
            combine_exit(true, &CliStep::UpToDate, EXIT_UPDATE_AVAILABLE),
            EXIT_UPDATE_AVAILABLE
        );
        assert_eq!(combine_exit(true, &CliStep::UpToDate, 0), 0);
        assert_eq!(combine_exit(true, &CliStep::Unavailable, 0), 1);
        assert_eq!(
            combine_exit(true, &CliStep::Unavailable, EXIT_UPDATE_AVAILABLE),
            1
        );
    }

    #[test]
    fn apply_keeps_not_replaceable_as_failure_after_project() {
        assert_eq!(combine_exit(false, &CliStep::NotReplaceable, 0), 1);
        assert_eq!(combine_exit(false, &CliStep::UpToDate, 0), 0);
        assert!(blocks_project_upgrade(&CliStep::NotReplaceable));
        assert!(!blocks_project_upgrade(&CliStep::UpToDate));
    }

    #[test]
    fn check_never_refreshes_the_installed_skill() {
        assert!(!should_refresh_skill(&CliStep::UpToDate, true));
        assert!(should_refresh_skill(&CliStep::UpToDate, false));
        assert!(!should_refresh_skill(&CliStep::Unavailable, false));
    }

    #[test]
    fn rerun_command_preserves_explicit_upgrade_intent() {
        assert_eq!(
            rerun_command(Some("0.12.3"), true),
            "lingxia upgrade --version 0.12.3 --yes"
        );
        assert_eq!(rerun_command(None, false), "lingxia upgrade");
    }

    #[test]
    fn staged_project_deferral_requires_automation_to_retry() {
        assert_eq!(deferred_project_exit(), EXIT_UPDATE_AVAILABLE);
        assert_ne!(deferred_project_exit(), 0);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn staged_replace_defers_project_upgrade_until_the_new_cli_runs() {
        assert!(should_defer_project_upgrade(&CliStep::Staged, false));
        assert!(!should_defer_project_upgrade(&CliStep::Staged, true));
        assert!(!should_defer_project_upgrade(&CliStep::UpToDate, false));
    }
}
