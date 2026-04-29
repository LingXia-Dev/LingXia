use anyhow::{Result, anyhow};
use std::collections::HashSet;

const ABI_ARM64: &str = "arm64-v8a";
const ABI_ARMV7: &str = "armeabi-v7a";
const ABI_ALL: &str = "all";

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
        let targets = match normalized {
            ABI_ALL => [TARGET_ARMV7, TARGET_ARM64].as_slice(),
            ABI_ARMV7 => [TARGET_ARMV7].as_slice(),
            ABI_ARM64 => [TARGET_ARM64].as_slice(),
            _ => return Err(unsupported_abi_error(normalized)),
        };
        for target in targets {
            if dedup.insert(*target) {
                out.push((*target).to_string());
            }
        }
    }
    Ok(out)
}

fn unsupported_abi_error(abi: &str) -> anyhow::Error {
    anyhow!(
        "Unsupported Android ABI: {}.\n\
Supported --abis values:\n\
  - all\n\
  - arm64-v8a\n\
  - armeabi-v7a\n\
Default: arm64-v8a",
        abi
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(values: &[&str]) -> Vec<String> {
        resolve_android_targets_from_abis(
            &values
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>(),
        )
        .unwrap()
    }

    #[test]
    fn defaults_to_arm64() {
        assert_eq!(
            resolve_android_targets_from_abis(&[]).unwrap(),
            vec![TARGET_ARM64.to_string()]
        );
    }

    #[test]
    fn all_builds_arm32_and_arm64() {
        assert_eq!(
            resolve(&[ABI_ALL]),
            vec![TARGET_ARMV7.to_string(), TARGET_ARM64.to_string()]
        );
    }

    #[test]
    fn keeps_requested_order_and_deduplicates() {
        assert_eq!(
            resolve(&[ABI_ARM64, ABI_ALL, ABI_ARMV7]),
            vec![TARGET_ARM64.to_string(), TARGET_ARMV7.to_string()]
        );
    }

    #[test]
    fn rejects_unknown_abi() {
        let err = resolve_android_targets_from_abis(&["x86".to_string()]).unwrap_err();
        assert!(err.to_string().contains("Unsupported Android ABI: x86"));
    }
}
