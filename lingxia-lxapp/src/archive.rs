//! Shared archive utilities for tar.zst extraction and SHA256 verification.

use crate::error::LxAppError;
use ring::digest::{Context, SHA256};
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use tar::Archive;
use zstd::stream::read::Decoder as ZstdDecoder;

/// Extract a tar.zst archive to the destination directory.
/// Cleans the destination if it exists before extraction.
pub fn extract_tar_zst(archive_path: &Path, destination: &Path) -> Result<(), LxAppError> {
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

    let file = File::open(archive_path)?;
    let zstd_decoder = ZstdDecoder::new(file).map_err(|e| {
        LxAppError::IoError(format!(
            "Failed to create zstd decoder for {}: {}",
            archive_path.display(),
            e
        ))
    })?;
    let mut archive = Archive::new(zstd_decoder);
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

    let mut file = File::open(path)?;
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
