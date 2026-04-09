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

pub(crate) fn target_from_build_targets(build_targets: &[String]) -> RuntimeEcmaTarget {
    if build_targets.iter().any(|target| target.contains("armv7")) {
        RuntimeEcmaTarget::Es5
    } else {
        RuntimeEcmaTarget::Es2020
    }
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

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = sha2::Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}
