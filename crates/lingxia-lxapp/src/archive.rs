//! Shared archive utilities for tar.zst extraction and SHA256 verification.

use crate::error::LxAppError;
use ring::digest::{Context, SHA256};
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;
use tar::Archive;
use zstd::stream::read::Decoder as ZstdDecoder;

/// zstd frame magic, little-endian (`0xFD2FB528`).
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

/// Extract a tar.zst archive to the destination directory.
/// Cleans the destination if it exists before extraction.
pub fn extract_tar_zst(archive_path: &Path, destination: &Path) -> Result<(), LxAppError> {
    // Read the whole file first. Harmony's cache File handle can fail a
    // streaming zstd+tar walk even when the bytes on disk are complete.
    let bytes = fs::read(archive_path).map_err(|e| {
        LxAppError::IoError(format!(
            "Failed to read archive {}: {}",
            archive_path.display(),
            e
        ))
    })?;
    if bytes.len() < ZSTD_MAGIC.len() {
        return Err(LxAppError::IoError(format!(
            "Failed to extract archive {}: empty or truncated ({} bytes)",
            archive_path.display(),
            bytes.len()
        )));
    }
    if bytes[..ZSTD_MAGIC.len()] != ZSTD_MAGIC {
        return Err(LxAppError::IoError(format!(
            "Failed to extract archive {}: not zstd (magic {:02x}{:02x}{:02x}{:02x}, {} bytes)",
            archive_path.display(),
            bytes[0],
            bytes[1],
            bytes[2],
            bytes[3],
            bytes.len()
        )));
    }

    let mut zstd_decoder = ZstdDecoder::new(Cursor::new(bytes)).map_err(|e| {
        LxAppError::IoError(format!(
            "Failed to create zstd decoder for {}: {}",
            archive_path.display(),
            e
        ))
    })?;
    // Published packages may use a larger window than the decoder default.
    zstd_decoder.window_log_max(31).map_err(|e| {
        LxAppError::IoError(format!(
            "Failed to configure zstd decoder for {}: {}",
            archive_path.display(),
            e
        ))
    })?;
    // Reject unreadable or invalid downloads before touching a previous install.
    if destination.exists() {
        fs::remove_dir_all(destination).map_err(|e| {
            LxAppError::IoError(format!(
                "Failed to clean destination {}: {}",
                destination.display(),
                e
            ))
        })?;
    }
    fs::create_dir_all(destination)?;
    let mut archive = Archive::new(zstd_decoder);
    archive.set_preserve_permissions(false);
    archive.set_unpack_xattrs(false);
    archive.unpack(destination).map_err(|e| {
        LxAppError::IoError(format!(
            "Failed to extract archive {}: {}",
            archive_path.display(),
            e
        ))
    })?;

    Ok(())
}

/// Verify SHA256 checksum of a file. Returns Ok if matches or if expected is empty.
pub fn verify_sha256(path: &Path, expected_hex: &str) -> Result<(), LxAppError> {
    if expected_hex.is_empty() {
        return Ok(());
    }
    let actual = compute_sha256_hex(path)?;
    if actual.eq_ignore_ascii_case(expected_hex) {
        Ok(())
    } else {
        Err(LxAppError::IoError(format!(
            "checksum mismatch: expected {}, got {}",
            expected_hex, actual
        )))
    }
}

/// Compute SHA-256 of a file and return lowercase hex string.
fn compute_sha256_hex(path: &Path) -> Result<String, LxAppError> {
    use std::fmt::Write;

    let mut file = fs::File::open(path)?;
    let mut ctx = Context::new(&SHA256);
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        ctx.update(&buf[..n]);
    }
    let digest = ctx.finish();
    let mut hex = String::with_capacity(digest.as_ref().len() * 2);
    for b in digest.as_ref() {
        let _ = write!(hex, "{:02x}", b);
    }
    Ok(hex)
}

#[cfg(test)]
mod tests {
    use super::extract_tar_zst;
    use std::fs;

    fn write_tar_zst(src: &std::path::Path, dest: &std::path::Path) {
        let file = fs::File::create(dest).unwrap();
        let encoder = zstd::stream::write::Encoder::new(file, 0).unwrap();
        let mut builder = tar::Builder::new(encoder);
        builder.append_dir_all(".", src).unwrap();
        builder.into_inner().unwrap().finish().unwrap();
    }

    #[test]
    fn extracts_a_tar_zst_package() {
        let root = std::env::temp_dir().join(format!("lx-archive-{}", std::process::id()));
        let src = root.join("src");
        let archive = root.join("pkg.lxapp");
        let dest = root.join("out");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("lxapp.json"), "{\"appId\":\"demo\"}\n").unwrap();
        write_tar_zst(&src, &archive);

        extract_tar_zst(&archive, &dest).unwrap();
        let manifest = fs::read_to_string(dest.join("lxapp.json")).unwrap();
        assert!(manifest.contains("demo"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_a_non_zstd_file() {
        let path = std::env::temp_dir().join(format!("lx-archive-bad-{}", std::process::id()));
        fs::write(&path, b"PK\x03\x04not-zstd").unwrap();
        let dest = std::env::temp_dir().join(format!("lx-archive-bad-out-{}", std::process::id()));
        let error = extract_tar_zst(&path, &dest).unwrap_err().to_string();
        assert!(error.contains("not zstd"), "{error}");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(&dest);
    }

    #[test]
    fn invalid_header_preserves_previous_install() {
        let root = tempfile::tempdir().unwrap();
        let archive = root.path().join("invalid.lxapp");
        let destination = root.path().join("installed");
        fs::create_dir_all(&destination).unwrap();
        fs::write(destination.join("lxapp.json"), "previous manifest").unwrap();
        fs::write(&archive, b"not a zstd archive").unwrap();

        assert!(extract_tar_zst(&archive, &destination).is_err());
        assert_eq!(
            fs::read_to_string(destination.join("lxapp.json")).unwrap(),
            "previous manifest"
        );
    }
}
