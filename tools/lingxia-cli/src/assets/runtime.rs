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
