//! Which kind of machine the lxapp is running on.
//!
//! This is deliberately not a size class. Window width answers "how much room
//! is there"; [`is_pad`] answers how many compact-strip slots fit. Neither
//! answers whether a camera-first destination is worth showing at all. A
//! narrowed desktop window is still a desktop.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

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
static PAD: AtomicBool = AtomicBool::new(false);

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

/// Whether this host is a tablet / pad.
///
/// Orthogonal to [`HostClass`]: an iPad build is still `mobile` for `showOn`,
/// but its compact tab strip has room for the declaration cap instead of the
/// phone's five slots. Desktop skins ignore the compact-strip fold.
pub fn is_pad() -> bool {
    PAD.load(Ordering::Relaxed)
}

/// Shipped hosts that know they are a tablet set this before the first tab-bar
/// snapshot is read. The runner sets it when the simulated frame is a tablet.
///
/// Changing it does not reload pages: the overflow fold is chrome-only, and a
/// real device does not flip pad at runtime. The runner rebuilds chrome when
/// it switches frames.
pub fn set_pad(pad: bool) {
    PAD.store(pad, Ordering::Relaxed);
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
        PAD.store(false, Ordering::Relaxed);
        assert_eq!(host_class(), HostClass::built_for());
        assert!(!is_pad());
    }

    #[test]
    fn pad_is_orthogonal_to_host_class() {
        set_host_class(HostClass::Mobile);
        set_pad(true);
        assert_eq!(host_class(), HostClass::Mobile);
        assert!(is_pad());
        set_pad(false);
        assert_eq!(host_class(), HostClass::Mobile);
        assert!(!is_pad());
        OVERRIDE.store(UNSET, Ordering::Relaxed);
    }
}
