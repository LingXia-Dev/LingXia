use crate::commands::rust::cargo_version_line;
use crate::platform::doctor::command_version_line;
use sysinfo::System;

pub fn execute(verbose: bool) {
    print!("{}", render(verbose));
}

pub(crate) fn render(verbose: bool) -> String {
    let mut output = format!("{}\n", version_line());
    if !verbose {
        return output;
    }

    let os = os_line();
    let rustc = command_version_line("rustc", &["--version"], false)
        .unwrap_or_else(|| "not found".to_string());
    let cargo = cargo_version_line().unwrap_or_else(|| "not found".to_string());

    output.push_str(&format!("commit-hash: {}\n", env!("LINGXIA_COMMIT_HASH")));
    output.push_str(&format!("commit-date: {}\n", env!("LINGXIA_COMMIT_DATE")));
    output.push_str(&format!("host: {}\n", env!("LINGXIA_BUILD_HOST")));
    output.push_str(&format!("os: {os}\n"));
    output.push_str(&format!("rustc: {rustc}\n"));
    output.push_str(&format!("cargo: {cargo}\n"));
    output.push_str(&format!("sdk: {}\n", env!("LINGXIA_SDK_VERSION")));
    output.push_str(&format!(
        "lingxia-crate: {}\n",
        env!("LINGXIA_RUST_CRATE_VERSION")
    ));
    output.push_str(&format!("runner: {}\n", env!("CARGO_PKG_VERSION")));
    output.push_str(&format!("bridge: {}\n", env!("LINGXIA_BRIDGE_VERSION")));
    output.push_str(&format!("types: {}\n", env!("LINGXIA_TYPES_VERSION")));
    output.push_str(&format!("rong: {}\n", env!("LINGXIA_RONG_VERSION")));
    output
}

fn version_line() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let hash = env!("LINGXIA_COMMIT_HASH");
    let date = env!("LINGXIA_COMMIT_DATE");

    if hash == "unknown" || date == "unknown" {
        return format!("lingxia {version}");
    }

    let short_hash = hash.get(..9).unwrap_or(hash);
    format!("lingxia {version} ({short_hash} {date})")
}

fn os_line() -> String {
    let os = System::long_os_version()
        .or_else(System::name)
        .unwrap_or_else(|| "unknown".to_string());
    let bitness = if cfg!(target_pointer_width = "64") {
        "64-bit"
    } else if cfg!(target_pointer_width = "32") {
        "32-bit"
    } else {
        "unknown-bit"
    };
    format!("{os} [{bitness}]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terse_version_matches_cargo_style() {
        assert_eq!(render(false), format!("{}\n", version_line()));
    }

    #[test]
    fn verbose_version_includes_build_and_component_metadata() {
        let output = render(true);
        assert!(output.starts_with(&format!("{}\n", version_line())));
        for expected in [
            "commit-hash:",
            "commit-date:",
            "host:",
            "os:",
            "rustc:",
            "cargo:",
            "sdk:",
            "lingxia-crate:",
            "runner:",
            "bridge:",
            "types:",
            "rong:",
        ] {
            assert!(
                output.contains(expected),
                "missing {expected} in:\n{output}"
            );
        }
        for internal in [
            "release:",
            "polyfills:",
            "browser-shell-webui:",
            "resource-bundle:",
        ] {
            assert!(
                !output.contains(internal),
                "unexpected {internal} in:\n{output}"
            );
        }
    }
}
