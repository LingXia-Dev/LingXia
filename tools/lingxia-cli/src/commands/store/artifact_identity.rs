//! Pre-flight artifact identity check.
//!
//! Extracts the real bundle/package identity from the built artifact and
//! compares it with the platform identity in `lingxia.yaml` BEFORE any
//! credential resolution or network request, so a `.dev`/`.preview` or
//! wrong-app artifact fails immediately with a rebuild hint. Formats whose
//! identity cannot be read offline are skipped with an explicit note, never
//! silently.

use anyhow::{Context, Result, bail};
use std::io::{Cursor, Read};
use std::path::Path;

pub const STORE_ARTIFACT_IDENTITY_MISMATCH: &str = "STORE_ARTIFACT_IDENTITY_MISMATCH";

enum Extracted {
    Identity(String),
    Unsupported(&'static str),
}

/// Verify `artifact` matches `expected`; on mismatch fail before any
/// credential is loaded.
pub fn verify(artifact: &Path, expected: &str) -> Result<()> {
    match extract(artifact)? {
        Extracted::Identity(found) if found == expected => Ok(()),
        Extracted::Identity(found) => bail!(
            "{STORE_ARTIFACT_IDENTITY_MISMATCH}: {} contains `{found}`, but lingxia.yaml \
             expects `{expected}`. Rebuild the release artifact first: \
             `lingxia build --release --env release`.",
            artifact.display()
        ),
        Extracted::Unsupported(why) => {
            eprintln!(
                "note: artifact identity not verified for {} ({why})",
                artifact.display()
            );
            Ok(())
        }
    }
}

fn extract(artifact: &Path) -> Result<Extracted> {
    let ext = artifact
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "ipa" => ipa_identity(artifact),
        "hap" | "app" => harmony_identity(artifact),
        "msix" => {
            let bytes =
                std::fs::read(artifact).with_context(|| format!("read {}", artifact.display()))?;
            msix_identity(&bytes)
        }
        "msixupload" => {
            // A .msixupload is a zip wrapping the .msix; check the inner one.
            let file = std::fs::File::open(artifact)
                .with_context(|| format!("open {}", artifact.display()))?;
            let mut zip = zip::ZipArchive::new(file)
                .with_context(|| format!("read {}", artifact.display()))?;
            let inner = (0..zip.len()).find_map(|i| {
                let name = zip.name_for_index(i)?.to_string();
                name.to_ascii_lowercase().ends_with(".msix").then_some(i)
            });
            match inner {
                Some(index) => {
                    let mut bytes = Vec::new();
                    zip.by_index(index)?.read_to_end(&mut bytes)?;
                    msix_identity(&bytes)
                }
                None => Ok(Extracted::Unsupported("no .msix inside the .msixupload")),
            }
        }
        "apk" | "aab" => Ok(Extracted::Unsupported(
            "Android manifests are binary-encoded; the store validates the package name",
        )),
        "pkg" => Ok(Extracted::Unsupported(
            "pkg archives cannot be inspected offline; App Store Connect validates the bundle id",
        )),
        _ => Ok(Extracted::Unsupported("unknown artifact format")),
    }
}

/// IPA: `Payload/<App>.app/Info.plist` → `CFBundleIdentifier`.
fn ipa_identity(artifact: &Path) -> Result<Extracted> {
    let file =
        std::fs::File::open(artifact).with_context(|| format!("open {}", artifact.display()))?;
    let mut zip =
        zip::ZipArchive::new(file).with_context(|| format!("read {}", artifact.display()))?;
    let plist_index = (0..zip.len()).find_map(|i| {
        let name = zip.name_for_index(i)?;
        let mut parts = name.split('/');
        (parts.next() == Some("Payload")
            && parts.next().is_some_and(|p| p.ends_with(".app"))
            && parts.next() == Some("Info.plist")
            && parts.next().is_none())
        .then_some(i)
    });
    let Some(index) = plist_index else {
        return Ok(Extracted::Unsupported("no Payload/*.app/Info.plist found"));
    };
    let mut bytes = Vec::new();
    zip.by_index(index)?.read_to_end(&mut bytes)?;
    let value = plist::Value::from_reader(Cursor::new(bytes)).context("parse Info.plist")?;
    let bundle_id = value
        .as_dictionary()
        .and_then(|d| d.get("CFBundleIdentifier"))
        .and_then(|v| v.as_string())
        .context("Info.plist has no CFBundleIdentifier")?;
    Ok(Extracted::Identity(bundle_id.to_string()))
}

/// Harmony `.app` / `.hap`: `pack.info` JSON → `summary.app.bundleName`.
fn harmony_identity(artifact: &Path) -> Result<Extracted> {
    let file =
        std::fs::File::open(artifact).with_context(|| format!("open {}", artifact.display()))?;
    let mut zip =
        zip::ZipArchive::new(file).with_context(|| format!("read {}", artifact.display()))?;
    let Ok(mut entry) = zip.by_name("pack.info") else {
        return Ok(Extracted::Unsupported("no pack.info in the package"));
    };
    let mut text = String::new();
    entry.read_to_string(&mut text)?;
    let json: serde_json::Value = serde_json::from_str(&text).context("parse pack.info")?;
    let bundle_name = json
        .pointer("/summary/app/bundleName")
        .and_then(|v| v.as_str())
        .context("pack.info has no summary.app.bundleName")?;
    Ok(Extracted::Identity(bundle_name.to_string()))
}

/// MSIX: `AppxManifest.xml` → `<Identity Name="...">`.
fn msix_identity(msix_bytes: &[u8]) -> Result<Extracted> {
    let mut zip = zip::ZipArchive::new(Cursor::new(msix_bytes)).context("read msix")?;
    let Ok(mut entry) = zip.by_name("AppxManifest.xml") else {
        return Ok(Extracted::Unsupported("no AppxManifest.xml in the msix"));
    };
    let mut text = String::new();
    entry.read_to_string(&mut text)?;
    let identity = text
        .split("<Identity")
        .nth(1)
        .and_then(|after| after.split("Name=\"").nth(1))
        .and_then(|after| after.split('"').next())
        .context("AppxManifest.xml has no Identity Name")?;
    Ok(Extracted::Identity(identity.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn write_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(&mut buffer);
        for (name, bytes) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
        buffer.into_inner()
    }

    #[test]
    fn ipa_identity_matches_and_mismatches() {
        let plist = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>com.example.app</string>
</dict></plist>"#;
        let bytes = write_zip(&[("Payload/My.app/Info.plist", plist)]);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("My.ipa");
        std::fs::write(&path, bytes).unwrap();

        verify(&path, "com.example.app").unwrap();
        let err = verify(&path, "com.example.app.dev")
            .unwrap_err()
            .to_string();
        assert!(err.starts_with(STORE_ARTIFACT_IDENTITY_MISMATCH));
        assert!(err.contains("com.example.app"));
    }

    #[test]
    fn hap_identity_from_pack_info() {
        let pack = br#"{"summary":{"app":{"bundleName":"com.example.demo"}}}"#;
        let bytes = write_zip(&[("pack.info", pack)]);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("demo.hap");
        std::fs::write(&path, bytes).unwrap();

        verify(&path, "com.example.demo").unwrap();
        assert!(verify(&path, "com.other").is_err());
    }

    #[test]
    fn msix_identity_from_manifest() {
        let manifest = br#"<?xml version="1.0"?>
<Package><Identity Name="Example.App" Publisher="CN=E" Version="1.0.0.0" /></Package>"#;
        let bytes = write_zip(&[("AppxManifest.xml", manifest)]);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.msix");
        std::fs::write(&path, bytes).unwrap();

        verify(&path, "Example.App").unwrap();
        assert!(verify(&path, "Other.App").is_err());
    }

    #[test]
    fn unverifiable_formats_are_skipped_with_note() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.apk");
        std::fs::write(&path, b"binary").unwrap();
        verify(&path, "com.example.app").unwrap();
    }
}
