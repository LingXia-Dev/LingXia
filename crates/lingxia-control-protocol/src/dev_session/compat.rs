//! Compatibility for the pieces a dev session combines. A newer CLI builds
//! and drives anything inside its window; git commits are not a gate.
//!
//! The window is `[BUILD_TARGET_FLOOR, tool line]`. A host project's SDK pins
//! must sit in that window and agree with each other. Each lxapp's
//! `minRuntime` must sit in the window and not above the host SDK, and its
//! installed `@lingxia/*` packages must be on that `minRuntime` line. A live
//! session peer must speak a dev protocol this tool still speaks.
//!
//! This module is pure. Reading a project from disk is the `project` feature
//! ([`project`]), which only the CLI binaries enable.

#[cfg(feature = "project")]
mod project;
#[cfg(feature = "project")]
pub use project::*;

/// Oldest host line this tool can still build. Raise it in the same change
/// that stops emitting output that line can read, and say so in the release
/// notes. Patch never matters.
pub const BUILD_TARGET_FLOOR: (u64, u64) = (0, 25);

/// Oldest [`super::DEV_SESSION_PROTOCOL_VERSION`] this build still speaks.
/// Raise it together with [`BUILD_TARGET_FLOOR`] when the dropped host line
/// was the last one on that protocol.
pub const MIN_DEV_SESSION_PROTOCOL_VERSION: u32 = 2;

/// Whether a peer `hello` version is one this build still speaks.
pub fn speaks_dev_protocol(version: u32) -> bool {
    (MIN_DEV_SESSION_PROTOCOL_VERSION..=super::DEV_SESSION_PROTOCOL_VERSION).contains(&version)
}

/// Set to `1` to turn the pre-flight checks into warnings when one misreads a
/// project. Not named in check errors: the supported window never needs it.
/// The dev-session handshake still refuses a protocol it does not speak.
pub const SKIP_ENV: &str = "LINGXIA_SKIP_COMPAT";

/// Whether [`SKIP_ENV`] asks to warn instead of fail.
pub fn skip_requested() -> bool {
    std::env::var(SKIP_ENV).is_ok_and(|value| is_truthy(&value))
}

fn is_truthy(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty() && value != "0" && !value.eq_ignore_ascii_case("false")
}

/// The protocol versions this build speaks, for messages: `2` or `2..=3`.
pub fn protocol_span() -> String {
    if MIN_DEV_SESSION_PROTOCOL_VERSION == super::DEV_SESSION_PROTOCOL_VERSION {
        super::DEV_SESSION_PROTOCOL_VERSION.to_string()
    } else {
        format!(
            "{}..={}",
            MIN_DEV_SESSION_PROTOCOL_VERSION,
            super::DEV_SESSION_PROTOCOL_VERSION
        )
    }
}

/// A `major.minor` release line.
pub type Line = (u64, u64);

/// One versioned piece of the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    /// `CLI`, `Runner`, `host`, or a package name.
    pub name: String,
    pub version: String,
    /// Commit the piece was built from, when it says.
    pub commit: Option<String>,
}

impl Component {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        let version = version.into();
        let (version, commit) = split_build(&version);
        Self {
            name: name.into(),
            version,
            commit,
        }
    }

    pub fn with_commit(mut self, commit: Option<&str>) -> Self {
        if let Some(commit) = commit.map(str::trim).filter(|commit| is_commit(commit)) {
            self.commit = Some(commit.to_string());
        }
        self
    }

    pub fn label(&self) -> String {
        match &self.commit {
            Some(commit) => format!("{} {} ({})", self.name, self.version, short(commit)),
            None => format!("{} {}", self.name, self.version),
        }
    }
}

/// `0.19.0+abc1234` → (`0.19.0`, `abc1234`); a build stamp such as
/// `0.19.0 (abc1234 2026-09-25)` → (`0.19.0`, `abc1234`).
fn split_build(raw: &str) -> (String, Option<String>) {
    let raw = raw.trim();
    if let Some((version, rest)) = raw.split_once(" (") {
        let commit = rest
            .trim_end_matches(')')
            .split_whitespace()
            .next()
            .map(|commit| commit.trim_end_matches("-dirty"))
            .filter(|commit| is_commit(commit))
            .map(str::to_string);
        return (version.to_string(), commit);
    }
    if let Some((version, build)) = raw.split_once('+') {
        let commit = build
            .split('.')
            .find(|part| is_commit(part))
            .map(str::to_string);
        return (version.to_string(), commit);
    }
    (raw.to_string(), None)
}

fn is_commit(value: &str) -> bool {
    (7..=40).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn short(commit: &str) -> &str {
    &commit[..commit.len().min(9)]
}

/// `major.minor` of a version or a lower-bound Cargo/npm requirement, if it
/// parses. Leading `~`, `^`, `=`, `>`, and `v` are ignored; an upper bound
/// (`<0.27`) names no line and does not parse.
pub fn line(version: &str) -> Option<(u64, u64)> {
    let version = version
        .trim()
        .trim_start_matches(['^', '~', '=', '>', 'v', 'V']);
    let mut parts = version.split(['.', '-', '+']);
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

/// `major.minor` of an exact `major.minor.patch` version, as `minRuntime`
/// requires: no range marker, no prerelease/build suffix, no extra dot parts.
pub fn patch_line(version: &str) -> Option<(u64, u64)> {
    let mut parts = version.trim().split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next()?.parse::<u64>().ok()?;
    let _patch: u64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor))
}

/// What a compatibility check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skew {
    /// One line: what disagrees, and the fix.
    pub message: String,
}

impl std::fmt::Display for Skew {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Skew {}

/// One host SDK pin read from the project (`native/Cargo.toml (lingxia)`, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdkPin {
    pub source: String,
    pub spec: String,
}

/// An lxapp this tool is about to build or load.
#[derive(Debug, Clone)]
pub struct LxappCheck<'a> {
    /// Path or app id, shown in the error.
    pub name: &'a str,
    pub min_runtime: &'a str,
    pub packages: &'a [Component],
}

/// The single major.minor the pins name, with the first pin's text for
/// messages. `Ok(None)` when none of them is a version (path dependencies).
/// Different lines are an error.
pub fn agreed_host_line(pins: &[SdkPin]) -> Result<Option<(Line, &str)>, Skew> {
    let parsed: Vec<_> = pins
        .iter()
        .filter_map(|pin| line(&pin.spec).map(|parsed| (pin, parsed)))
        .collect();
    let Some((first_pin, first)) = parsed.first() else {
        return Ok(None);
    };
    if parsed.iter().any(|(_, parsed)| parsed != first) {
        let listed = parsed
            .iter()
            .map(|(pin, _)| format!("{} {}", pin.source, pin.spec))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(Skew {
            message: format!(
                "host SDK pins differ: {listed} — align them or run `lingxia upgrade`"
            ),
        });
    }
    Ok(Some((*first, first_pin.spec.as_str())))
}

/// `target` must lie in `[floor, tool]`. `what` is "host SDK" or an lxapp's
/// `minRuntime`; `version` is the text to print.
pub fn check_target_line(
    floor: (u64, u64),
    tool: (u64, u64),
    target: (u64, u64),
    what: &str,
    version: &str,
) -> Result<(), Skew> {
    if target > tool {
        return Err(Skew {
            message: format!(
                "{what} {version} is newer than this CLI ({}.{}); run `lingxia upgrade`",
                tool.0, tool.1
            ),
        });
    }
    if target < floor {
        return Err(Skew {
            message: format!(
                "{what} {version} is older than this CLI supports (>= {}.{}). \
                 To keep targeting {}.{}, run `lingxia upgrade --version {}.{}.0`; \
                 to move the project onto this CLI, run `lingxia upgrade`",
                floor.0, floor.1, target.0, target.1, target.0, target.1
            ),
        });
    }
    Ok(())
}

/// `minRuntime` against the tool window and the host line, then the lxapp's
/// packages against that `minRuntime` line. A different commit on the same
/// line is fine.
pub fn check_lxapp(
    floor: (u64, u64),
    tool: (u64, u64),
    host_line: Option<(u64, u64)>,
    lxapp: &LxappCheck<'_>,
) -> Result<(), Skew> {
    let Some(target) = patch_line(lxapp.min_runtime) else {
        return Err(Skew {
            message: format!(
                "{} minRuntime must be major.minor.patch, got \"{}\"",
                lxapp.name, lxapp.min_runtime
            ),
        });
    };
    check_target_line(
        floor,
        tool,
        target,
        &format!("{} minRuntime", lxapp.name),
        lxapp.min_runtime,
    )?;
    if let Some(host) = host_line
        && target > host
    {
        return Err(Skew {
            message: format!(
                "{} minRuntime {} is newer than the host SDK {}.{}; \
                 run `lingxia upgrade` to move the host, or lower this lxapp",
                lxapp.name, lxapp.min_runtime, host.0, host.1
            ),
        });
    }
    let ceiling = host_line.map_or(tool, |host| host.min(tool));
    check_packages(lxapp, target, ceiling)
}

/// A live Runner or host must speak a protocol this tool still speaks.
/// `protocol` is `None` when the peer never reported one.
pub fn check_session(tool: &str, peer: &str, protocol: Option<u32>) -> Result<(), Skew> {
    let Some(protocol) = protocol else {
        return Err(Skew {
            message: format!(
                "{peer} did not report a dev protocol — restart the session with this CLI \
                 (`lingxia dev`)"
            ),
        });
    };
    if speaks_dev_protocol(protocol) {
        return Ok(());
    }
    let spoken = protocol_span();
    let fix = if protocol > super::DEV_SESSION_PROTOCOL_VERSION {
        "run `lingxia upgrade`"
    } else {
        "restart the session with this CLI (`lingxia dev`)"
    };
    Err(Skew {
        message: format!(
            "{peer} speaks dev protocol {protocol}; this {tool} speaks {spoken} — {fix}"
        ),
    })
}

/// Packages newer than `minRuntime` may call APIs a `minRuntime` host lacks;
/// older ones miss what that line's build output expects.
/// `ceiling` is the highest `minRuntime` the host and this tool accept.
fn check_packages(lxapp: &LxappCheck<'_>, target: Line, ceiling: Line) -> Result<(), Skew> {
    let LxappCheck {
        name,
        min_runtime,
        packages,
    } = *lxapp;
    let behind: Vec<_> = packages
        .iter()
        .filter(|package| line(&package.version).is_some_and(|parsed| parsed < target))
        .collect();
    let ahead: Vec<_> = packages
        .iter()
        .filter(|package| line(&package.version).is_some_and(|parsed| parsed > target))
        .collect();
    if behind.is_empty() && ahead.is_empty() {
        return Ok(());
    }
    let mut fixes = Vec::new();
    let install = |list: &[&Component]| {
        let specs: Vec<_> = list
            .iter()
            .map(|package| format!("{}@~{}.{}.0", package.name, target.0, target.1))
            .collect();
        format!("npm install {}", specs.join(" "))
    };
    if !behind.is_empty() {
        fixes.push(install(&behind));
    }
    match ahead
        .iter()
        .filter_map(|package| line(&package.version))
        .max()
    {
        Some(newest) if newest <= ceiling => fixes.push(format!(
            "raise lxapp.json minRuntime to {}.{}.0 (its hosts need that line too), or {}",
            newest.0,
            newest.1,
            install(&ahead)
        )),
        Some(_) => fixes.push(install(&ahead)),
        None => {}
    }
    let what = behind
        .iter()
        .chain(&ahead)
        .map(|package| package.label())
        .collect::<Vec<_>>()
        .join(", ");
    Err(Skew {
        message: format!(
            "{name}: {what} vs minRuntime {min_runtime} — fix: {}",
            fixes.join("; ")
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLOOR: (u64, u64) = (0, 25);
    const TOOL: (u64, u64) = (0, 26);

    fn lxapp<'a>(name: &'a str, min_runtime: &'a str, packages: &'a [Component]) -> LxappCheck<'a> {
        LxappCheck {
            name,
            min_runtime,
            packages,
        }
    }

    #[test]
    fn versions_and_requirements_parse() {
        assert_eq!(line("0.18.3"), Some((0, 18)));
        assert_eq!(line("v1.2.0-beta.1"), Some((1, 2)));
        assert_eq!(line("~0.25.2"), Some((0, 25)));
        assert_eq!(line("^0.25.0"), Some((0, 25)));
        assert_eq!(line(">=0.25.0"), Some((0, 25)));
        assert_eq!(line("<0.27"), None);
        assert_eq!(line("x"), None);
        let stamped = Component::new("CLI", "0.18.0 (ae2f993ab 2026-09-20)");
        assert_eq!(stamped.version, "0.18.0");
        assert_eq!(stamped.commit.as_deref(), Some("ae2f993ab"));
        let packed = Component::new("@lingxia/test", "0.18.0+ae2f993");
        assert_eq!(packed.commit.as_deref(), Some("ae2f993"));
    }

    #[test]
    fn an_older_lxapp_inside_the_window_builds() {
        let packages = [Component::new("@lingxia/react", "0.25.4")];
        assert!(check_lxapp(FLOOR, TOOL, None, &lxapp("app", "0.25.0", &packages)).is_ok());
        // Same line, another commit: not a gate.
        let local = [Component::new("@lingxia/react", "0.25.0").with_commit(Some("1234567"))];
        assert!(check_lxapp(FLOOR, TOOL, None, &lxapp("app", "0.25.0", &local)).is_ok());
    }

    #[test]
    fn min_runtime_must_be_exact_major_minor_patch() {
        for min_runtime in ["0.25", "0.25.0-beta", "0.25.0.1", "v0.25.0"] {
            let err = check_lxapp(FLOOR, TOOL, None, &lxapp("app", min_runtime, &[]))
                .unwrap_err()
                .message;
            assert!(
                err.contains("must be major.minor.patch"),
                "{min_runtime:?}: {err}"
            );
        }
        assert!(check_lxapp(FLOOR, TOOL, None, &lxapp("app", "0.25.0", &[])).is_ok());
    }

    #[test]
    fn packages_must_match_min_runtime_not_the_tool() {
        let behind = [Component::new("@lingxia/react", "0.24.2")];
        let err = check_lxapp(FLOOR, TOOL, None, &lxapp("app", "0.25.0", &behind))
            .unwrap_err()
            .message;
        assert!(err.contains("npm install @lingxia/react@~0.25.0"), "{err}");
        let ahead = [Component::new("@lingxia/react", "0.26.0")];
        let err = check_lxapp(FLOOR, TOOL, None, &lxapp("app", "0.25.0", &ahead))
            .unwrap_err()
            .message;
        assert!(
            err.contains("raise lxapp.json minRuntime to 0.26.0")
                && err.contains("npm install @lingxia/react@~0.25.0"),
            "{err}"
        );
        // A 0.25 host cannot take a 0.26 lxapp: only the install is offered.
        let err = check_lxapp(FLOOR, TOOL, Some((0, 25)), &lxapp("app", "0.25.0", &ahead))
            .unwrap_err()
            .message;
        assert!(!err.contains("raise"), "{err}");
        assert!(err.contains("npm install @lingxia/react@~0.25.0"), "{err}");
    }

    #[test]
    fn a_line_outside_the_window_names_the_fix() {
        let err = check_lxapp(FLOOR, TOOL, None, &lxapp("app", "0.24.0", &[]))
            .unwrap_err()
            .message;
        assert!(err.contains("lingxia upgrade --version 0.24.0"), "{err}");
        let err = check_lxapp(FLOOR, TOOL, None, &lxapp("app", "0.27.0", &[]))
            .unwrap_err()
            .message;
        assert!(err.contains("newer than this CLI (0.26)"), "{err}");
    }

    #[test]
    fn a_host_loads_an_older_lxapp_and_refuses_a_newer_one() {
        let packages = [Component::new("@lingxia/react", "0.25.1")];
        assert!(
            check_lxapp(
                FLOOR,
                TOOL,
                Some((0, 26)),
                &lxapp("lxapp", "0.25.0", &packages)
            )
            .is_ok()
        );
        let err = check_lxapp(FLOOR, TOOL, Some((0, 25)), &lxapp("lxapp", "0.26.0", &[]))
            .unwrap_err()
            .message;
        assert!(err.contains("newer than the host SDK 0.25"), "{err}");
    }

    #[test]
    fn host_pins_agree_or_are_named() {
        let pins = [
            SdkPin {
                source: "native/Cargo.toml (lingxia)".into(),
                spec: "~0.25.2".into(),
            },
            SdkPin {
                source: "android/app/build.gradle.kts".into(),
                spec: "0.25.0".into(),
            },
        ];
        assert_eq!(agreed_host_line(&pins).unwrap(), Some(((0, 25), "~0.25.2")));
        assert_eq!(agreed_host_line(&[]).unwrap(), None);
        let mixed = [
            pins[0].clone(),
            SdkPin {
                source: "windows/Cargo.toml (lingxia-windows-sdk)".into(),
                spec: "~0.26.0".into(),
            },
        ];
        let err = agreed_host_line(&mixed).unwrap_err().message;
        assert!(err.contains("host SDK pins differ"), "{err}");
        assert!(err.contains("0.26.0"), "{err}");
        let err = check_target_line(FLOOR, TOOL, (0, 24), "host SDK", "0.24.0")
            .unwrap_err()
            .message;
        assert!(err.contains("host SDK 0.24.0"), "{err}");
    }

    #[test]
    fn the_skip_switch_reads_like_a_flag() {
        for on in ["1", "true", "yes"] {
            assert!(is_truthy(on), "{on}");
        }
        for off in ["", " ", "0", "false", "FALSE"] {
            assert!(!is_truthy(off), "{off:?}");
        }
    }

    #[test]
    fn a_session_speaks_protocol_not_a_commit() {
        assert!(
            check_session(
                "lxdev",
                "Runner",
                Some(super::super::DEV_SESSION_PROTOCOL_VERSION)
            )
            .is_ok()
        );
        let missing = check_session("lxdev", "Runner", None).unwrap_err().message;
        assert!(
            missing.contains("did not report a dev protocol"),
            "{missing}"
        );
        let newer = check_session("lxdev", "host", Some(99))
            .unwrap_err()
            .message;
        assert!(newer.contains("lingxia upgrade"), "{newer}");
        let older = check_session("CLI", "host", Some(1)).unwrap_err().message;
        assert!(older.contains("lingxia dev"), "{older}");
    }
}
