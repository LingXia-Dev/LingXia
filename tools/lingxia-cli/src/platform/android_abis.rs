use anyhow::{Result, anyhow};
use std::collections::HashSet;

const ABI_ARM64: &str = "arm64-v8a";
const ABI_ARMV7: &str = "armeabi-v7a";

const TARGET_ARM64: &str = "aarch64-linux-android";
const TARGET_ARMV7: &str = "armv7-linux-androideabi";

pub fn resolve_android_targets_from_abis(abis: &[String]) -> Result<Vec<String>> {
    if abis.is_empty() {
        return Ok(vec![TARGET_ARM64.to_string()]);
    }

    let mut dedup = HashSet::new();
    let mut out = Vec::new();
    for abi in abis {
        let normalized = abi.trim();
        let target = match normalized {
            ABI_ARM64 => TARGET_ARM64,
            ABI_ARMV7 => TARGET_ARMV7,
            _ => return Err(unsupported_abi_error(normalized)),
        };
        if dedup.insert(target) {
            out.push(target.to_string());
        }
    }
    Ok(out)
}

fn unsupported_abi_error(abi: &str) -> anyhow::Error {
    anyhow!(
        "Unsupported Android ABI: {}.\n\
Supported --abis values:\n\
  - arm64-v8a\n\
  - armeabi-v7a",
        abi
    )
}
