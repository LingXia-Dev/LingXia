//! Which kind of machine the lxapp is running on.
//!
//! This is deliberately not a size class. Window width answers "how much room
//! is there", which is what a tab strip uses to decide whether to fold; it does
//! not answer whether a camera-first destination is worth showing at all. A
//! narrowed desktop window is still a desktop.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HostClass {
    Mobile,
    Desktop,
}

impl HostClass {
    /// The wire value shared by the bridge config, `lx.app.getBaseInfo()` and
    /// the tab-bar `showOn` list, so the three can never disagree.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mobile => "mobile",
            Self::Desktop => "desktop",
        }
    }

    /// What the build targets. The runner overrides this per simulated device.
    const fn built_for() -> Self {
        if cfg!(any(
            target_os = "ios",
            target_os = "android",
            target_env = "ohos"
        )) {
            Self::Mobile
        } else {
            Self::Desktop
        }
    }
}

const UNSET: u8 = 0;
const MOBILE: u8 = 1;
const DESKTOP: u8 = 2;

static OVERRIDE: AtomicU8 = AtomicU8::new(UNSET);

/// The host this lxapp is running on. Defaults to the build target, which is
/// right for every shipped host; only the runner simulates a different one.
pub fn host_class() -> HostClass {
    match OVERRIDE.load(Ordering::Relaxed) {
        MOBILE => HostClass::Mobile,
        DESKTOP => HostClass::Desktop,
        _ => HostClass::built_for(),
    }
}

/// Simulate a host. The runner is a desktop binary standing in for a phone, so
/// the build target alone would answer for the wrong machine.
///
/// Changing it reloads open pages: a page is handed its class in the bridge
/// config at load, so one that is already rendering would otherwise keep the
/// layout for the machine the developer just switched away from. Switching
/// device frames within one class (iPhone to iPhone SE) is not a change and
/// reloads nothing.
pub fn set_host_class(class: HostClass) {
    // Against the effective class, not the stored override: the runner's first
    // call replaces `built_for()` with the same answer on a desktop preset, and
    // that is not a change.
    let changed = host_class() != class;
    OVERRIDE.store(
        match class {
            HostClass::Mobile => MOBILE,
            HostClass::Desktop => DESKTOP,
        },
        Ordering::Relaxed,
    );
    if changed {
        super::runtime_registry::reload_pages_for_host_class_change();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_replaces_the_build_target_both_ways() {
        set_host_class(HostClass::Mobile);
        assert_eq!(host_class(), HostClass::Mobile);
        set_host_class(HostClass::Desktop);
        assert_eq!(host_class(), HostClass::Desktop);
        assert_eq!(HostClass::Mobile.as_str(), "mobile");
        assert_eq!(HostClass::Desktop.as_str(), "desktop");
        // Restore so a shared-process test run does not inherit a simulated host.
        OVERRIDE.store(UNSET, Ordering::Relaxed);
        assert_eq!(host_class(), HostClass::built_for());
    }
}
