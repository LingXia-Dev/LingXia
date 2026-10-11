//! `dist/<platform>/manifest.json`: what one `lingxia package` run produced.
//! Publish, store submit, and install take artifacts from it rather than
//! guessing by name or mtime, and the checksums prove a file is the one that
//! run wrote.
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const FILE: &str = "manifest.json";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DistManifest {
    pub platform: String,
    pub version: String,
    pub env: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lingxia_id: Option<String>,
    pub artifacts: Vec<DistArtifact>,
}

/// One produced file. `format` names what it is (`apk`, `aab`, `update`,
/// `ipa`, `app`, `hap`, `setup`, `portable`, `msix`, `zip`, `dmg`); each
/// consumer picks the formats it accepts.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DistArtifact {
    pub format: String,
    pub file: String,
    pub sha256: String,
    pub size: u64,
}

pub struct Produced {
    pub format: &'static str,
    pub path: PathBuf,
}

/// Record `produced` (files inside `dir`) as this run's output, replacing any
/// earlier manifest. Nothing is written when the run produced nothing.
pub fn write(
    dir: &Path,
    platform: &str,
    version: &str,
    env: &str,
    lingxia_id: Option<&str>,
    produced: &[Produced],
) -> Result<Option<PathBuf>> {
    if produced.is_empty() {
        return Ok(None);
    }
    let artifacts = produced
        .iter()
        .map(|item| {
            if item.path.parent() != Some(dir) {
                bail!(
                    "{} is outside {}; package artifacts live beside their manifest",
                    item.path.display(),
                    dir.display()
                );
            }
            let (sha256, size) = digest(&item.path)?;
            Ok(DistArtifact {
                format: item.format.to_string(),
                file: file_name(&item.path)?,
                sha256,
                size,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let manifest = DistManifest {
        platform: platform.to_string(),
        version: version.to_string(),
        env: env.to_string(),
        lingxia_id: lingxia_id.map(str::to_string),
        artifacts,
    };
    let path = dir.join(FILE);
    fs::write(&path, serde_json::to_vec_pretty(&manifest)?)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(Some(path))
}

pub fn read(dir: &Path) -> Result<DistManifest> {
    let path = dir.join(FILE);
    let bytes = fs::read(&path).with_context(|| {
        format!(
            "No {FILE} in {} — run `lingxia package` to produce one",
            dir.display()
        )
    })?;
    serde_json::from_slice(&bytes).with_context(|| format!("Invalid {}", path.display()))
}

/// The first artifact in `dir`'s manifest whose format is in `formats`
/// (preference order), after checking it is still the file that was packaged.
pub fn resolve(dir: &Path, formats: &[&str]) -> Result<(DistManifest, PathBuf)> {
    let manifest = read(dir)?;
    let Some(artifact) = formats
        .iter()
        .find_map(|format| manifest.artifacts.iter().find(|a| a.format == *format))
    else {
        bail!(
            "{} lists no {} artifact (it has: {}); run `lingxia package` for it",
            dir.join(FILE).display(),
            formats.join(" / "),
            manifest
                .artifacts
                .iter()
                .map(|a| a.format.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    };
    if artifact.file.contains(['/', '\\']) {
        bail!(
            "{} names a file outside its directory",
            dir.join(FILE).display()
        );
    }
    let path = dir.join(&artifact.file);
    let (sha256, size) = digest(&path).with_context(|| {
        format!(
            "{} is listed in {} but cannot be read; run `lingxia package` again",
            path.display(),
            FILE
        )
    })?;
    if sha256 != artifact.sha256 || size != artifact.size {
        bail!(
            "{} changed since `lingxia package` wrote it; package again",
            path.display()
        );
    }
    Ok((manifest, path))
}

fn digest(path: &Path) -> Result<(String, u64)> {
    let mut file =
        fs::File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let mut hash = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
        size += read as u64;
    }
    Ok((
        hash.finalize().iter().map(|b| format!("{b:02x}")).collect(),
        size,
    ))
}

fn file_name(path: &Path) -> Result<String> {
    Ok(path
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("Invalid artifact path {}", path.display()))?
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn produced(dir: &Path, name: &str, format: &'static str, bytes: &[u8]) -> Produced {
        let path = dir.join(name);
        fs::write(&path, bytes).unwrap();
        Produced { format, path }
    }

    #[test]
    fn resolves_by_format_preference_and_checks_the_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path();
        let files = [
            produced(dir, "Demo-1.0.0.aab", "aab", b"bundle"),
            produced(dir, "Demo-1.0.0.apk", "apk", b"apk"),
        ];
        write(dir, "android", "1.0.0", "prod", Some("demo"), &files).unwrap();

        let (manifest, path) = resolve(dir, &["aab", "apk"]).unwrap();
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(path, dir.join("Demo-1.0.0.aab"));
        assert_eq!(
            resolve(dir, &["apk"]).unwrap().1,
            dir.join("Demo-1.0.0.apk")
        );

        let err = resolve(dir, &["ipa"]).unwrap_err().to_string();
        assert!(
            err.contains("lists no ipa artifact (it has: aab, apk)"),
            "{err}"
        );

        fs::write(dir.join("Demo-1.0.0.apk"), b"rebuilt elsewhere").unwrap();
        let err = resolve(dir, &["apk"]).unwrap_err().to_string();
        assert!(err.contains("changed since `lingxia package`"), "{err}");
    }

    #[test]
    fn a_directory_without_a_manifest_points_at_package() {
        let temp = tempfile::tempdir().unwrap();
        let err = resolve(temp.path(), &["apk"]).unwrap_err().to_string();
        assert!(err.contains("run `lingxia package`"), "{err}");
    }

    #[test]
    fn nothing_produced_writes_nothing_and_strays_are_refused() {
        let temp = tempfile::tempdir().unwrap();
        assert!(
            write(temp.path(), "macos", "1.0.0", "prod", None, &[])
                .unwrap()
                .is_none()
        );
        let other = tempfile::tempdir().unwrap();
        let stray = produced(other.path(), "x.zip", "update", b"x");
        assert!(write(temp.path(), "macos", "1.0.0", "prod", None, &[stray]).is_err());
    }
}
