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
pub fn set_host_class(class: HostClass) {
    OVERRIDE.store(
        match class {
            HostClass::Mobile => MOBILE,
            HostClass::Desktop => DESKTOP,
        },
        Ordering::Relaxed,
    );
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
        // Restore so a shared-process test run does not inherit a simulated host.
        OVERRIDE.store(UNSET, Ordering::Relaxed);
        assert_eq!(host_class(), HostClass::built_for());
    }
}
