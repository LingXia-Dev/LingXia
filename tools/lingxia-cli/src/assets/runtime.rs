use crate::runtime;
use colored::Colorize;

pub(super) struct PreparedRuntimeAsset {
    pub(super) bytes: Vec<u8>,
    pub(super) runtime_hash: String,
}

pub(super) fn prepare_runtime_asset(target: runtime::RuntimeEcmaTarget) -> PreparedRuntimeAsset {
    let resolved = runtime::embedded_runtime(target);
    println!(
        "  {} bridge-runtime.js ({}) ← {}",
        "✓".green(),
        target.as_str(),
        resolved.source
    );

    PreparedRuntimeAsset {
        bytes: resolved.bytes.to_vec(),
        runtime_hash: resolved.hash,
    }
}

pub(super) struct PreparedPolyfillsAsset {
    pub(super) bytes: Vec<u8>,
    pub(super) hash: String,
}

pub(super) fn prepare_polyfills_es5_asset() -> PreparedPolyfillsAsset {
    let resolved = runtime::embedded_polyfills_es5();
    println!(
        "  {} polyfills.es5.js ← {}",
        "✓".green(),
        resolved.source
    );
    PreparedPolyfillsAsset {
        bytes: resolved.bytes.to_vec(),
        hash: resolved.hash,
    }
}
