//! The check `lingxia dev`, `lingxia build`, and `lxdev test` run before they
//! start, and `lingxia doctor --project` prints.
//!
//! A project inside `[BUILD_TARGET_FLOOR, this CLI's base line]` builds.
//! Packages follow each lxapp's `minRuntime`, and a host's SDK pins follow
//! that same window. See [`lingxia_control_protocol::dev_session::compat`].

use anyhow::{Result, anyhow};
use colored::Colorize;
use lingxia_control_protocol::dev_session::compat::{self, Component, resolved_packages};
use std::path::{Path, PathBuf};

/// This CLI, with the commit it was built from. Shown by doctor; not a gate.
pub fn cli() -> Component {
    Component::new("CLI", env!("LINGXIA_BUILD_VERSION"))
}

fn tool_line() -> (u64, u64) {
    compat::line(env!("LINGXIA_RUST_CRATE_VERSION"))
        .expect("the CLI's base version is major.minor.patch")
}

/// Directories whose `node_modules/@lingxia` belong to the project: the root,
/// and each local lxapp a host project bundles.
pub fn package_roots(project_root: &Path) -> Vec<PathBuf> {
    let mut roots = vec![project_root.to_path_buf()];
    for dir in compat::host_bundle_dirs(project_root) {
        if !roots.contains(&dir) {
            roots.push(dir);
        }
    }
    roots
}

/// Fail when the project sits outside this CLI's window, or its pieces
/// disagree with each other.
pub fn ensure_project(project_root: &Path) -> Result<()> {
    let errors = compat::project_errors(project_root, tool_line());
    if errors.is_empty() {
        return Ok(());
    }
    if compat::skip_requested() {
        warn_skipped(&errors);
        return Ok(());
    }
    Err(anyhow!("{}", errors.join("\n")))
}

/// What `LINGXIA_SKIP_COMPAT` let through, so a skipped check stays visible.
pub fn warn_skipped(errors: &[String]) {
    for error in errors {
        eprintln!("{} {error} ({}=1)", "warning:".yellow(), compat::SKIP_ENV);
    }
}

/// `lingxia doctor --project`: the window, the project's pins, and the verdict.
pub fn print_project_report(project_root: &Path) -> bool {
    let cli = cli();
    println!("{}", "[project]".bold().cyan());
    let (tool_major, tool_minor) = tool_line();
    let (floor_major, floor_minor) = compat::BUILD_TARGET_FLOOR;
    println!(
        "  CLI       {}  builds {floor_major}.{floor_minor}..{tool_major}.{tool_minor}",
        describe(&cli)
    );
    if crate::config::has_host_config(project_root) {
        let pins = compat::host_sdk_pins(project_root);
        match compat::agreed_host_line(&pins) {
            Ok(Some(((major, minor), _))) => println!("  host SDK  {major}.{minor}"),
            Ok(None) => println!("  host SDK  source checkout"),
            Err(_) => println!("  host SDK  pins differ"),
        }
    }
    for root in package_roots(project_root) {
        if root.join("lxapp.json").is_file()
            && let Ok(min_runtime) = compat::read_min_runtime(&root)
        {
            println!("  {}  minRuntime {min_runtime}", root.display());
        }
        for package in resolved_packages(&root) {
            println!("  {} [{}]", describe(&package), root.display());
        }
    }
    let errors = compat::project_errors(project_root, tool_line());
    let mut ok = if errors.is_empty() {
        println!("  {} compatible", "✓".green());
        true
    } else {
        for error in &errors {
            println!("  {} {error}", "✗".red());
        }
        false
    };
    // A standalone lxapp runs in the desktop Runner. A release Runner of this
    // CLI version is the one `lingxia dev` launches.
    if project_root.join("lxapp.json").is_file() && !crate::config::has_host_config(project_root) {
        let (runner, verdict) = crate::runner_cache::installed_report();
        println!("  Runner    {runner}");
        if let Err(reason) = verdict {
            println!("  {} {reason}", "✗".red());
            ok = false;
        }
    }
    ok
}

fn describe(component: &Component) -> String {
    match &component.commit {
        Some(commit) => format!("{} {} ({commit})", component.name, component.version),
        None => format!("{} {}", component.name, component.version),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn install(dir: &Path, name: &str, version: &str) {
        let package = dir.join("node_modules/@lingxia").join(name);
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(
            package.join("package.json"),
            format!(r#"{{"name":"@lingxia/{name}","version":"{version}"}}"#),
        )
        .unwrap();
    }

    fn write_lxapp(dir: &Path, min_runtime: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("lxapp.json"),
            format!(r#"{{"appId":"demo","minRuntime":"{min_runtime}","pages":[]}}"#),
        )
        .unwrap();
    }

    #[test]
    fn hoisted_packages_are_checked_and_nearest_copies_win() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("apps/demo");
        std::fs::create_dir_all(&app).unwrap();
        install(dir.path(), "react", "0.1.0");
        assert!(
            resolved_packages(&app)
                .iter()
                .any(|p| p.name == "@lingxia/react" && p.version == "0.1.0")
        );
        install(&app, "react", "0.2.0");
        let packages = resolved_packages(&app);
        assert_eq!(
            packages
                .iter()
                .filter(|p| p.name == "@lingxia/react")
                .count(),
            1
        );
        assert!(
            packages
                .iter()
                .any(|p| p.name == "@lingxia/react" && p.version == "0.2.0")
        );
    }

    #[test]
    fn an_older_project_inside_the_window_builds() {
        let (floor_major, floor_minor) = compat::BUILD_TARGET_FLOOR;
        let dir = tempfile::tempdir().unwrap();
        let min_runtime = format!("{floor_major}.{floor_minor}.0");
        write_lxapp(dir.path(), &min_runtime);
        install(
            dir.path(),
            "react",
            &format!("{floor_major}.{floor_minor}.4"),
        );
        assert!(
            ensure_project(dir.path()).is_ok(),
            "a project on the floor builds with a newer CLI"
        );
    }

    #[test]
    fn a_host_project_checks_its_sdk_and_each_lxapp() {
        let (tool_major, tool_minor) = tool_line();
        let (floor_major, floor_minor) = compat::BUILD_TARGET_FLOOR;
        let dir = tempfile::tempdir().unwrap();
        let min_runtime = format!("{floor_major}.{floor_minor}.0");
        std::fs::write(
            dir.path().join("lingxia.yaml"),
            "app:\n  projectName: demo-host\n  packageId: app.example.demohost\n  productName: Demo Host\n  productVersion: 1.0.0\n  platforms: [windows]\n  homeAppId: demo\nresources:\n  bundles:\n    - type: lxapp\n      appId: demo\n      path: lxapp\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("native")).unwrap();
        std::fs::write(
            dir.path().join("native/Cargo.toml"),
            format!("[dependencies]\nlingxia = \"~{tool_major}.{tool_minor}.0\"\n"),
        )
        .unwrap();
        write_lxapp(&dir.path().join("lxapp"), &min_runtime);
        install(
            &dir.path().join("lxapp"),
            "react",
            &format!("{floor_major}.{floor_minor}.7"),
        );
        assert_eq!(package_roots(dir.path()).len(), 2);
        assert!(ensure_project(dir.path()).is_ok());

        // A current copy at the host root must not conceal the bundled lxapp.
        install(dir.path(), "react", &format!("{tool_major}.{tool_minor}.7"));
        install(&dir.path().join("lxapp"), "react", "0.1.0");
        let err = ensure_project(dir.path()).unwrap_err().to_string();
        assert!(err.contains("@lingxia/react 0.1.0"), "{err}");
        assert!(
            err.contains(&dir.path().join("lxapp").display().to_string()),
            "{err}"
        );

        if (floor_major, floor_minor) < (tool_major, tool_minor) {
            install(
                &dir.path().join("lxapp"),
                "react",
                &format!("{tool_major}.{tool_minor}.1"),
            );
            // Inside the tool window, above the host pin.
            std::fs::write(
                dir.path().join("native/Cargo.toml"),
                format!("[dependencies]\nlingxia = \"~{floor_major}.{floor_minor}.0\"\n"),
            )
            .unwrap();
            write_lxapp(
                &dir.path().join("lxapp"),
                &format!("{tool_major}.{tool_minor}.0"),
            );
            let err = ensure_project(dir.path()).unwrap_err().to_string();
            assert!(err.contains("newer than the host SDK"), "{err}");
            assert!(!print_project_report(dir.path()));
        }
    }
}
