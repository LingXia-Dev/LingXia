//! Update signing: sign opaque manifest bytes, verify then parse.
//!
//! The manifest carries its own authenticated `v`; the transport carries its
//! version in the endpoint path. Neither belongs in an unsigned envelope field.

use crate::error::UpdateError;
use crate::{ReleaseType, UpdatePackageInfo, host_channel};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

const MAX_SIGNED_BYTES: usize = 8 * 1024;
const MAX_SIGNATURES: usize = 2;
const MAX_PUBLIC_KEYS: usize = 2;

/// Opaque envelope carried on check-update. Providers must not reserialize `signed`.
///
/// Carries no scheme identifier. The transport is versioned by its endpoint
/// path and the manifest by the authenticated `v` inside `signed`; a third
/// identifier, sitting *outside* the signature where a caller controls it,
/// would only invite a client to pick its verification by what it was handed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAuthentication {
    pub signed: String,
    pub signatures: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignRequest<'a> {
    pub kind: &'a str,
    pub target_id: &'a str,
    pub channel: &'a str,
    pub platform: &'a str,
    pub version: &'a str,
    pub sha256: &'a str,
    pub size: u64,
    pub required_runtime_version: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateVerifyTarget {
    pub kind: String,
    pub target_id: String,
    pub channel: String,
    pub platform: String,
    pub exact_version: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ManifestWire {
    v: u32,
    kind: String,
    #[serde(rename = "targetId")]
    target_id: String,
    channel: String,
    platform: String,
    version: String,
    sha256: String,
    size: u64,
    #[serde(rename = "requiredRuntimeVersion")]
    required_runtime_version: String,
}

pub fn archive_sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let mut out = String::with_capacity(digest.len() * 2);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

pub fn encode_base64url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn decode_base64url(value: &str) -> Result<Vec<u8>, UpdateError> {
    URL_SAFE_NO_PAD
        .decode(value.trim().as_bytes())
        .map_err(|e| UpdateError::invalid_parameter(format!("invalid base64url: {e}")))
}

pub fn compact_manifest(req: &SignRequest<'_>) -> Result<Vec<u8>, UpdateError> {
    let wire = ManifestWire {
        v: 1,
        kind: req.kind.to_string(),
        target_id: req.target_id.to_string(),
        channel: req.channel.to_string(),
        platform: req.platform.to_string(),
        version: req.version.to_string(),
        sha256: req.sha256.to_string(),
        size: req.size,
        required_runtime_version: req.required_runtime_version.to_string(),
    };
    serde_json::to_vec(&wire)
        .map_err(|e| UpdateError::runtime(format!("encode update manifest: {e}")))
}

pub fn public_key_base64url(seed: &[u8; 32]) -> String {
    encode_base64url(SigningKey::from_bytes(seed).verifying_key().as_bytes())
}

pub fn load_signing_seed_file(path: &Path) -> Result<[u8; 32], UpdateError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(path)
            .map_err(|e| UpdateError::io(format!("read signing key {}: {e}", path.display())))?
            .permissions()
            .mode()
            & 0o777;
        if mode & 0o077 != 0 {
            return Err(UpdateError::invalid_parameter(format!(
                "signing key {} must not be group/other-accessible",
                path.display()
            )));
        }
    }
    let text = fs::read_to_string(path)
        .map_err(|e| UpdateError::io(format!("read signing key {}: {e}", path.display())))?;
    let bytes = decode_base64url(text.trim())?;
    bytes
        .try_into()
        .map_err(|_| UpdateError::invalid_parameter("signing key must be a 32-byte Ed25519 seed"))
}

pub fn sign_package(
    seed: &[u8; 32],
    req: &SignRequest<'_>,
) -> Result<UpdateAuthentication, UpdateError> {
    let manifest = compact_manifest(req)?;
    let signature = SigningKey::from_bytes(seed).sign(&manifest).to_bytes();
    Ok(UpdateAuthentication {
        signed: encode_base64url(&manifest),
        signatures: vec![encode_base64url(&signature)],
    })
}

pub fn channel_requires_signature(channel: &str) -> bool {
    channel == ReleaseType::Preview.as_str() || channel == ReleaseType::Release.as_str()
}

/// Whether *this build* accepts an unsigned update, whatever channel is asked
/// for. Callers cannot pass a channel: that is the whole point — see
/// [`verify_checked_update`].
pub fn host_requires_signature() -> bool {
    channel_requires_signature(host_channel().as_str())
}

/// Preview/release only query check-update when the host embedded public keys.
/// Developer always queries; without keys it does not verify.
pub fn check_update_enabled(trusted_public_keys: &[String]) -> bool {
    !host_requires_signature() || !trusted_public_keys.is_empty()
}

pub fn sign_package_from_key_file(
    channel: &str,
    key_file: Option<&Path>,
    req: &SignRequest<'_>,
) -> Result<Option<UpdateAuthentication>, UpdateError> {
    let key_file = key_file.filter(|path| !path.as_os_str().is_empty());
    match (channel_requires_signature(channel), key_file) {
        (true, None) => Err(UpdateError::invalid_parameter(format!(
            "{channel} publish requires --update-signing-key-file"
        ))),
        (false, None) => Ok(None),
        (_, Some(path)) => {
            let seed = load_signing_seed_file(path)?;
            sign_package(&seed, req).map(Some)
        }
    }
}

pub fn verify_checked_update(
    mut package: UpdatePackageInfo,
    target: &UpdateVerifyTarget,
    trusted_public_keys: &[String],
) -> Result<UpdatePackageInfo, UpdateError> {
    // Whether a signature may be waived is a property of *this build*, never of
    // the request. An App Link query or `lx.navigateToApp({envVersion})` picks
    // the channel an lxapp is fetched on, so keying the waiver on
    // `target.channel` let anyone who can hand a release device a link ask for
    // the developer channel and be served an unsigned package.
    //
    // `target.channel` still binds the manifest below: a release host may open
    // a developer-channel lxapp, but only one a trusted key signed for that
    // channel.
    let signature_required = host_requires_signature();
    if trusted_public_keys.is_empty() && !signature_required {
        return Ok(package);
    }

    let Some(auth) = package.authentication.as_ref() else {
        if signature_required {
            return Err(UpdateError::invalid_parameter(format!(
                "{} builds require signed updates ({} channel package is unsigned)",
                host_channel(),
                target.channel
            )));
        }
        return Ok(package);
    };

    if auth.signatures.is_empty() || auth.signatures.len() > MAX_SIGNATURES {
        return Err(UpdateError::invalid_parameter(
            "authentication must include 1 or 2 signatures",
        ));
    }
    if trusted_public_keys.is_empty() {
        return Err(UpdateError::invalid_parameter(
            "no trusted update public keys are embedded in this build",
        ));
    }
    if trusted_public_keys.len() > MAX_PUBLIC_KEYS {
        return Err(UpdateError::invalid_parameter(
            "at most two trusted update public keys are allowed",
        ));
    }

    let signed = decode_base64url(&auth.signed)?;
    if signed.len() > MAX_SIGNED_BYTES {
        return Err(UpdateError::invalid_parameter(
            "signed manifest is too large",
        ));
    }
    let keys = decode_public_keys(trusted_public_keys)?;
    let mut accepted = false;
    for signature_b64 in &auth.signatures {
        let signature = decode_signature(signature_b64)?;
        if keys
            .iter()
            .any(|key| key.verify_strict(&signed, &signature).is_ok())
        {
            accepted = true;
            break;
        }
    }
    if !accepted {
        return Err(UpdateError::invalid_parameter(
            "update authentication signature is invalid",
        ));
    }

    let manifest: ManifestWire = serde_json::from_slice(&signed)
        .map_err(|e| UpdateError::invalid_parameter(format!("signed manifest is not JSON: {e}")))?;
    bind_manifest(&manifest, target)?;

    package.version = manifest.version;
    package.checksum_sha256 = manifest.sha256;
    package.size = Some(manifest.size);
    // Empty signed value is the authenticated "no floor"; do not keep the
    // provider's unsigned minRuntimeVersion.
    package.required_runtime_version = {
        let trimmed = manifest.required_runtime_version.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    };
    Ok(package)
}

pub fn verify_archive_bytes(
    data: &[u8],
    expected_sha256: &str,
    expected_size: u64,
) -> Result<(), UpdateError> {
    if data.len() as u64 != expected_size {
        return Err(UpdateError::invalid_parameter(format!(
            "archive size mismatch: expected {expected_size}, got {}",
            data.len()
        )));
    }
    let actual = archive_sha256_hex(data);
    if actual != expected_sha256 {
        return Err(UpdateError::invalid_parameter(format!(
            "archive sha256 mismatch: expected {expected_sha256}, got {actual}"
        )));
    }
    Ok(())
}

pub fn embedded_update_public_keys() -> Vec<String> {
    lingxia_app_context::app_config()
        .map(|config| config.update_trusted_public_keys.clone())
        .unwrap_or_default()
}

pub fn host_update_platform() -> &'static str {
    if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "ios") {
        "ios"
    } else if cfg!(all(target_os = "linux", target_env = "ohos")) {
        "harmony"
    } else {
        "any"
    }
}

fn bind_manifest(manifest: &ManifestWire, target: &UpdateVerifyTarget) -> Result<(), UpdateError> {
    if manifest.v != 1 {
        return Err(UpdateError::invalid_parameter(
            "signed manifest v must be 1",
        ));
    }
    if manifest.kind != target.kind {
        return Err(UpdateError::invalid_parameter(
            "signed kind does not match target",
        ));
    }
    if manifest.target_id != target.target_id {
        return Err(UpdateError::invalid_parameter(
            "signed targetId does not match target",
        ));
    }
    if manifest.channel != target.channel {
        return Err(UpdateError::invalid_parameter(
            "signed channel does not match target",
        ));
    }
    if manifest.platform != target.platform {
        return Err(UpdateError::invalid_parameter(
            "signed platform does not match target",
        ));
    }
    if let Some(expected) = target.exact_version.as_deref()
        && manifest.version != expected
    {
        return Err(UpdateError::invalid_parameter(
            "signed version does not match requested targetVersion",
        ));
    }
    Ok(())
}

fn decode_public_keys(keys: &[String]) -> Result<Vec<VerifyingKey>, UpdateError> {
    keys.iter()
        .map(|key| {
            let bytes = decode_fixed(key, 32, "public key")?;
            let raw: [u8; 32] = bytes
                .try_into()
                .map_err(|_| UpdateError::invalid_parameter("public key must be 32 bytes"))?;
            VerifyingKey::from_bytes(&raw).map_err(|e| {
                UpdateError::invalid_parameter(format!("invalid Ed25519 public key: {e}"))
            })
        })
        .collect()
}

fn decode_signature(value: &str) -> Result<Signature, UpdateError> {
    let bytes = decode_fixed(value, 64, "signature")?;
    let raw: [u8; 64] = bytes
        .try_into()
        .map_err(|_| UpdateError::invalid_parameter("signature must be 64 bytes"))?;
    Ok(Signature::from_bytes(&raw))
}

fn decode_fixed(value: &str, expected_len: usize, label: &str) -> Result<Vec<u8>, UpdateError> {
    let bytes = decode_base64url(value)?;
    if bytes.len() != expected_len {
        return Err(UpdateError::invalid_parameter(format!(
            "{label} must be {expected_len} bytes"
        )));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: [u8; 32] = [7u8; 32];
    const ARCHIVE: &[u8] = b"lingxia-update-golden-archive";

    fn request<'a>(sha256: &'a str, size: u64) -> SignRequest<'a> {
        SignRequest {
            kind: "lxapp",
            target_id: "shop",
            channel: "release",
            platform: "any",
            version: "1.2.3",
            sha256,
            size,
            required_runtime_version: "",
        }
    }

    fn target() -> UpdateVerifyTarget {
        UpdateVerifyTarget {
            kind: "lxapp".into(),
            target_id: "shop".into(),
            channel: "release".into(),
            platform: "any".into(),
            exact_version: None,
        }
    }

    fn package(auth: Option<UpdateAuthentication>, sha256: &str, size: u64) -> UpdatePackageInfo {
        UpdatePackageInfo {
            version: "1.2.3".into(),
            url: "https://cdn.example.com/pkg".into(),
            checksum_sha256: sha256.into(),
            size: Some(size),
            release_notes: None,
            is_force_update: false,
            required_runtime_version: None,
            authentication: auth,
        }
    }

    #[test]
    fn matching_key_accepts_signed_artifact() {
        let sha256 = archive_sha256_hex(ARCHIVE);
        let size = ARCHIVE.len() as u64;
        let auth = sign_package(&SEED, &request(&sha256, size)).unwrap();
        let keys = [public_key_base64url(&SEED)];
        let verified =
            verify_checked_update(package(Some(auth), &sha256, size), &target(), &keys).unwrap();
        assert_eq!(verified.checksum_sha256, sha256);
        verify_archive_bytes(ARCHIVE, &verified.checksum_sha256, size).unwrap();
    }

    #[test]
    fn test_seed_public_key_is_stable() {
        assert_eq!(
            public_key_base64url(&SEED),
            "6kpsY-KcUgq-9VB7Ey7F-ZVHdq6-vnuSQh7qaRRG0iw"
        );
    }

    #[test]
    fn flipped_archive_byte_rejects() {
        let sha256 = archive_sha256_hex(ARCHIVE);
        let size = ARCHIVE.len() as u64;
        let auth = sign_package(&SEED, &request(&sha256, size)).unwrap();
        let keys = [public_key_base64url(&SEED)];
        let verified =
            verify_checked_update(package(Some(auth), &sha256, size), &target(), &keys).unwrap();
        let mut tampered = ARCHIVE.to_vec();
        tampered[0] ^= 0xff;
        assert!(verify_archive_bytes(&tampered, &verified.checksum_sha256, size).is_err());
    }

    #[test]
    fn flipped_signed_json_field_rejects() {
        let sha256 = archive_sha256_hex(ARCHIVE);
        let size = ARCHIVE.len() as u64;
        let mut auth = sign_package(&SEED, &request(&sha256, size)).unwrap();
        let original = decode_base64url(&auth.signed).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&original).unwrap();
        value["version"] = serde_json::json!("9.9.9");
        auth.signed = encode_base64url(&serde_json::to_vec(&value).unwrap());
        let keys = [public_key_base64url(&SEED)];
        assert!(
            verify_checked_update(package(Some(auth), &sha256, size), &target(), &keys).is_err()
        );
    }

    #[test]
    fn flipped_signature_rejects() {
        let sha256 = archive_sha256_hex(ARCHIVE);
        let size = ARCHIVE.len() as u64;
        let mut auth = sign_package(&SEED, &request(&sha256, size)).unwrap();
        let mut sig = decode_base64url(&auth.signatures[0]).unwrap();
        sig[0] ^= 0xff;
        auth.signatures[0] = encode_base64url(&sig);
        let keys = [public_key_base64url(&SEED)];
        assert!(
            verify_checked_update(package(Some(auth), &sha256, size), &target(), &keys).is_err()
        );
    }

    #[test]
    fn release_unsigned_rejects() {
        let sha256 = archive_sha256_hex(ARCHIVE);
        let size = ARCHIVE.len() as u64;
        let err = verify_checked_update(package(None, &sha256, size), &target(), &[]).unwrap_err();
        assert!(err.to_string().contains("require signed updates"), "{err}");
    }

    #[test]
    fn preview_unsigned_rejects() {
        let sha256 = archive_sha256_hex(ARCHIVE);
        let size = ARCHIVE.len() as u64;
        let mut preview = target();
        preview.channel = "preview".into();
        let err = verify_checked_update(package(None, &sha256, size), &preview, &[]).unwrap_err();
        assert!(err.to_string().contains("preview"));
        assert!(err.to_string().contains("require signed updates"), "{err}");
    }

    #[test]
    fn publishing_requires_a_signature_on_preview_and_release() {
        // The publish-time predicate does take a channel: it answers "which
        // channel am I signing for", not "may this build skip verifying".
        assert!(!channel_requires_signature("developer"));
        assert!(channel_requires_signature("preview"));
        assert!(channel_requires_signature("release"));
    }

    #[test]
    fn check_update_enabled_follows_this_build_and_its_keys() {
        // Tests run with no `app.json`, so the host channel defaults to
        // release — the safe default, and the one that makes the assertions
        // below meaningful.
        assert!(host_requires_signature());
        assert!(!check_update_enabled(&[]));
        assert!(check_update_enabled(&[public_key_base64url(&SEED)]));
    }

    #[test]
    fn asking_for_the_developer_channel_does_not_waive_a_release_build() {
        // The attack this closes: an App Link query or
        // `lx.navigateToApp({envVersion:'developer'})` picks the channel an
        // lxapp is fetched on. Keying the waiver on that let anyone who could
        // hand a release device a link be served an unsigned package.
        let sha256 = archive_sha256_hex(ARCHIVE);
        let size = ARCHIVE.len() as u64;
        let mut dev = target();
        dev.channel = "developer".into();

        let err = verify_checked_update(package(None, &sha256, size), &dev, &[]).unwrap_err();
        assert!(err.to_string().contains("require signed updates"), "{err}");

        // Nor with keys embedded: an unsigned package is still refused.
        let keys = [public_key_base64url(&SEED)];
        assert!(verify_checked_update(package(None, &sha256, size), &dev, &keys).is_err());
    }

    #[test]
    fn a_release_build_opens_a_developer_lxapp_only_when_it_is_signed_for_it() {
        // The channel still binds the manifest, so a developer-channel package
        // is openable on a release host — but only one a trusted key signed
        // for the developer channel.
        let sha256 = archive_sha256_hex(ARCHIVE);
        let size = ARCHIVE.len() as u64;
        let mut dev = target();
        dev.channel = "developer".into();
        let mut req = request(&sha256, size);
        req.channel = "developer";
        let auth = sign_package(&SEED, &req).unwrap();
        let keys = [public_key_base64url(&SEED)];
        verify_checked_update(package(Some(auth), &sha256, size), &dev, &keys).unwrap();

        // A package signed for release does not satisfy a developer request.
        let release_auth = sign_package(&SEED, &request(&sha256, size)).unwrap();
        assert!(
            verify_checked_update(package(Some(release_auth), &sha256, size), &dev, &keys).is_err()
        );
    }

    #[test]
    fn an_unsigned_developer_build_still_skips_verification() {
        // The waiver did not go away; it moved to where it cannot be asked for.
        assert!(!channel_requires_signature(
            crate::ReleaseType::Developer.as_str()
        ));
    }

    #[test]
    fn developer_invalid_envelope_rejects() {
        let sha256 = archive_sha256_hex(ARCHIVE);
        let size = ARCHIVE.len() as u64;
        let mut auth = sign_package(&SEED, &request(&sha256, size)).unwrap();
        auth.signatures[0] = encode_base64url(&[0u8; 64]);
        let mut dev = target();
        dev.channel = "developer".into();
        let keys = [public_key_base64url(&SEED)];
        assert!(verify_checked_update(package(Some(auth), &sha256, size), &dev, &keys).is_err());
    }

    #[test]
    fn release_publish_without_key_file_fails() {
        let sha256 = archive_sha256_hex(ARCHIVE);
        let err =
            sign_package_from_key_file("release", None, &request(&sha256, ARCHIVE.len() as u64))
                .unwrap_err();
        assert!(err.to_string().contains("--update-signing-key-file"));
    }

    #[test]
    fn preview_publish_without_key_file_fails() {
        let sha256 = archive_sha256_hex(ARCHIVE);
        let err =
            sign_package_from_key_file("preview", None, &request(&sha256, ARCHIVE.len() as u64))
                .unwrap_err();
        assert!(err.to_string().contains("preview publish"));
    }

    #[test]
    fn developer_publish_without_key_file_is_unsigned() {
        let sha256 = archive_sha256_hex(ARCHIVE);
        assert!(
            sign_package_from_key_file("developer", None, &request(&sha256, ARCHIVE.len() as u64),)
                .unwrap()
                .is_none()
        );
    }
}
