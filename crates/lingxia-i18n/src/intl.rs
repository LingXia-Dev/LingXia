//! The locale data `Intl` reads on QuickJS, which has none of its own. JavaScriptCore and ArkJS
//! carry theirs; on QuickJS every Logic and View context loads this one blob, whatever lxapp runs.

/// The ICU4X data blob (`data/intl.postcard`, described by `data/intl.toml`).
pub static INTL_DATA: &[u8] = include_bytes!("../data/intl.postcard");

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    #[test]
    fn the_blob_is_the_one_its_manifest_names() {
        let manifest = include_str!("../data/intl.toml");
        let named = manifest
            .lines()
            .find_map(|line| line.strip_prefix("sha256 = \""))
            .and_then(|rest| rest.strip_suffix('"'))
            .expect("the manifest names a sha256");
        let actual: String = Sha256::digest(super::INTL_DATA)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(
            actual, named,
            "regenerate data/intl.toml's sha256 with the blob"
        );
    }
}
