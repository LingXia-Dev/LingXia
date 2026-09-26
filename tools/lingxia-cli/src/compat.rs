//! The version check `lingxia dev`, `lingxia build` and `lxdev test` run
//! before they start, and `lingxia doctor --project` prints: this CLI and the
//! project's installed `@lingxia/*` packages must share a line, and a
//! standalone lxapp's desktop Runner must be this CLI's build
//! ([`crate::runner_cache`]). See
//! [`lingxia_control_protocol::dev_session::compat`].

use anyhow::{Result, anyhow};
use colored::Colorize;
use lingxia_control_protocol::dev_session::compat::{self, Component};
use std::path::{Path, PathBuf};

/// This CLI, with the commit it was built from.
pub fn cli() -> Component {
    Component::new("CLI", env!("LINGXIA_BUILD_VERSION"))
}

/// Directories whose `node_modules/@lingxia` belong to the project: the root,
/// and each local lxapp a host project bundles.
pub fn package_roots(project_root: &Path) -> Vec<PathBuf> {
    let mut roots = vec![project_root.to_path_buf()];
    if crate::config::has_host_config(project_root)
        && let Ok(config) = crate::config::LingXiaConfig::load(project_root)
        && let Some(resources) = config.resources.as_ref()
    {
        for bundle in &resources.bundles {
            if let Some(path) = bundle.path.as_deref() {
                let dir = project_root.join(path);
                if dir.join("lxapp.json").is_file() && !roots.contains(&dir) {
                    roots.push(dir);
                }
            }
        }
    }
    roots
}

/// The installed `@lingxia/*` packages of the project, first copy of each.
pub fn project_packages(project_root: &Path) -> Vec<Component> {
    let mut packages: Vec<Component> = Vec::new();
    for root in package_roots(project_root) {
        for package in compat::installed_packages(&root) {
            if !packages.iter().any(|known| known.name == package.name) {
                packages.push(package);
            }
        }
    }
    packages
}

/// Fail fast on a version skew (or warn under `LINGXIA_ALLOW_SKEW=1`).
pub fn ensure_project(project_root: &Path, hosts: &[Component]) -> Result<()> {
    match compat::check(&cli(), hosts, &project_packages(project_root)) {
        Ok(()) => Ok(()),
        Err(skew) if compat::skew_allowed() => {
            eprintln!("{} {skew}", "warning:".yellow());
            Ok(())
        }
        Err(skew) => Err(anyhow!("{skew}")),
    }
}

/// `lingxia doctor --project`: every component and the verdict.
pub fn print_project_report(project_root: &Path) -> bool {
    let cli = cli();
    println!("{}", "[project]".bold().cyan());
    println!("  CLI       {}", describe(&cli));
    let packages = project_packages(project_root);
    if packages.is_empty() {
        println!("  packages  none installed under node_modules/@lingxia (run npm install?)");
    }
    for package in &packages {
        println!("  {}", describe(package));
    }
    let mut ok = match compat::check(&cli, &[], &packages) {
        Ok(()) => {
            println!("  {} one version line", "✓".green());
            true
        }
        Err(skew) => {
            println!("  {} {skew}", "✗".red());
            false
        }
    };
    // A standalone lxapp runs in the desktop Runner, which must be this
    // CLI's build (a release CLI replaces another one by itself).
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

    #[test]
    fn a_host_project_checks_its_bundled_lxapps_too() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("lingxia.yaml"),
            "app:\n  projectName: demo-host\n  packageId: app.example.demohost\n  productName: Demo Host\n  productVersion: 1.0.0\n  platforms: [windows]\n  homeAppId: demo\nresources:\n  bundles:\n    - type: lxapp\n      appId: demo\n      path: lxapp\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("lxapp")).unwrap();
        std::fs::write(
            dir.path().join("lxapp/lxapp.json"),
            r#"{"appId":"demo","appName":"Demo","version":"1.0.0","pages":[]}"#,
        )
        .unwrap();
        let line = compat::line(&cli().version).unwrap();
        install(
            &dir.path().join("lxapp"),
            "react",
            &format!("{}.{}.7", line.0, line.1),
        );
        assert_eq!(package_roots(dir.path()).len(), 2);
        assert!(ensure_project(dir.path(), &[]).is_ok());

        install(&dir.path().join("lxapp"), "test", "0.1.0");
        let err = ensure_project(dir.path(), &[]).unwrap_err().to_string();
        assert!(err.contains("@lingxia/test 0.1.0"), "{err}");
        assert!(err.contains("npm install @lingxia/test@~"), "{err}");
        assert!(!print_project_report(dir.path()));
    }
}
