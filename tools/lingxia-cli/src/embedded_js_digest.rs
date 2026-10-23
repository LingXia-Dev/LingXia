//! Digest of the page JS the CLI embeds (`@lingxia/bridge`,
//! `@lingxia/polyfills`): their `package.json` and `src/`. Shared with
//! `build.rs`, which stamps the CLI's own; a pinned commit's is computed
//! from its checkout to tell whether this CLI ships that commit's bridge.

use std::path::Path;

pub const PACKAGES: [&str; 2] = ["lingxia-bridge", "lingxia-polyfills"];

/// FNV-1a over relative paths and contents, in path order.
pub fn digest(packages_dir: &Path) -> std::io::Result<String> {
    fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                collect(root, &path, out)?;
            } else {
                let relative = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((relative, std::fs::read(&path)?));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    for package in PACKAGES {
        let dir = packages_dir.join(package);
        let manifest = dir.join("package.json");
        files.push((format!("{package}/package.json"), std::fs::read(manifest)?));
        let mut sources = Vec::new();
        collect(&dir, &dir.join("src"), &mut sources)?;
        files.extend(
            sources
                .into_iter()
                .map(|(path, bytes)| (format!("{package}/{path}"), bytes)),
        );
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    };
    for (path, bytes) in &files {
        feed(path.as_bytes());
        feed(&[0]);
        feed(&(bytes.len() as u64).to_le_bytes());
        feed(bytes);
    }
    Ok(format!("{hash:016x}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_stamped_digest_is_this_checkout_s() {
        let packages = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages");
        assert_eq!(
            super::digest(&packages).unwrap(),
            env!("LINGXIA_EMBEDDED_JS_DIGEST")
        );
    }
}
