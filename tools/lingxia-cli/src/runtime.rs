use sha2::Digest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeEcmaTarget {
    Es5,
    Es2020,
}

impl RuntimeEcmaTarget {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            RuntimeEcmaTarget::Es5 => "es5",
            RuntimeEcmaTarget::Es2020 => "es2020",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ScaffoldPackageVersions {
    pub bridge: String,
    pub types: String,
}

#[derive(Debug, Clone)]
pub(crate) struct EmbeddedRuntime {
    pub bytes: &'static [u8],
    pub hash: String,
    pub source: &'static str,
}

#[derive(Debug, Clone)]
pub(crate) struct EmbeddedPolyfills {
    pub bytes: &'static [u8],
    pub hash: String,
    pub source: &'static str,
}

/// Lowest Android API level whose factory-default WebView is guaranteed to
/// support the ES2015+ runtime features the bridge depends on (notably
/// `Proxy` and `Reflect`, which landed in Chromium 49). API 24 (Android 7.0)
/// ships Chromium 51 as the default WebView; earlier versions can have an
/// older Chromium and must be served the ES5 runtime.
pub(crate) const MODERN_WEBVIEW_MIN_SDK: u32 = 24;

pub(crate) fn target_from_build_targets(
    build_targets: &[String],
    android_min_sdk: Option<u32>,
) -> RuntimeEcmaTarget {
    // armv7 implies a very old device — always ES5.
    if build_targets.iter().any(|target| target.contains("armv7")) {
        return RuntimeEcmaTarget::Es5;
    }
    // For arm64/x86_64 Android, the WebView is what matters. If the app
    // declares minSdk below the safe threshold, fall back to ES5.
    if let Some(min) = android_min_sdk
        && min < MODERN_WEBVIEW_MIN_SDK
    {
        return RuntimeEcmaTarget::Es5;
    }
    RuntimeEcmaTarget::Es2020
}

pub(crate) fn current_scaffold_versions() -> ScaffoldPackageVersions {
    let version = env!("CARGO_PKG_VERSION").to_string();
    ScaffoldPackageVersions {
        bridge: version.clone(),
        types: version,
    }
}

pub(crate) fn embedded_runtime(target: RuntimeEcmaTarget) -> EmbeddedRuntime {
    let bytes = match target {
        RuntimeEcmaTarget::Es5 => include_bytes!(env!("LINGXIA_BRIDGE_RUNTIME_ES5")).as_slice(),
        RuntimeEcmaTarget::Es2020 => {
            include_bytes!(env!("LINGXIA_BRIDGE_RUNTIME_ES2020")).as_slice()
        }
    };

    EmbeddedRuntime {
        bytes,
        hash: sha256_hex(bytes),
        source: "embedded @lingxia/bridge runtime",
    }
}

/// ES5 stdlib polyfills shipped alongside the bridge runtime on builds that
/// must support old Android WebView (Chromium 37–44). Loaded as its own
/// `<script>` tag in the page, BEFORE bridge-runtime.js, so any other script
/// that runs afterwards sees the polyfilled globals.
pub(crate) fn embedded_polyfills_es5() -> EmbeddedPolyfills {
    let bytes = include_bytes!(env!("LINGXIA_POLYFILLS_ES5")).as_slice();
    EmbeddedPolyfills {
        bytes,
        hash: sha256_hex(bytes),
        source: "embedded @lingxia/polyfills (es5)",
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = sha2::Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    fn targets(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn armv7_always_picks_es5_even_with_modern_min_sdk() {
        let t = targets(&["armv7-linux-androideabi"]);
        assert_eq!(
            target_from_build_targets(&t, Some(34)),
            RuntimeEcmaTarget::Es5
        );
    }

    #[test]
    fn arm64_with_old_min_sdk_picks_es5() {
        let t = targets(&["aarch64-linux-android"]);
        assert_eq!(
            target_from_build_targets(&t, Some(23)),
            RuntimeEcmaTarget::Es5
        );
    }

    #[test]
    fn arm64_at_threshold_picks_es2020() {
        let t = targets(&["aarch64-linux-android"]);
        assert_eq!(
            target_from_build_targets(&t, Some(MODERN_WEBVIEW_MIN_SDK)),
            RuntimeEcmaTarget::Es2020
        );
    }

    #[test]
    fn arm64_with_no_min_sdk_picks_es2020() {
        let t = targets(&["aarch64-linux-android"]);
        assert_eq!(target_from_build_targets(&t, None), RuntimeEcmaTarget::Es2020);
    }
}
