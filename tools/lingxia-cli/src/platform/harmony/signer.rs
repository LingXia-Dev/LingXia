//! Native HAP signing implementation in Rust.
//!
//! Design references:
//! - Harmony command-line signing flow:
//!   https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/ide-command-line-building-app#section103321051433
//! - OpenHarmony `hapsigner` implementation model:
//!   https://gitcode.com/openharmony/developtools_hapsigner
//!
//! This module reimplements the signing pipeline in Rust for CLI usage without Java tooling.

pub mod zip;

use anyhow::{Context, Result, anyhow};
use openssl::hash::{Hasher, MessageDigest, hash};
use openssl::pkcs7::{Pkcs7, Pkcs7Flags};
use openssl::pkcs12::Pkcs12;
use openssl::pkey::{PKey, Private};
use openssl::stack::Stack;
use openssl::x509::X509;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use self::zip::parse_zip;

const HAP_SIGNATURE_SCHEME_V1_BLOCK_ID: u32 = 0x20000000;
const HAP_PROOF_OF_ROTATION_BLOCK_ID: u32 = 0x20000001;
const HAP_PROFILE_BLOCK_ID: u32 = 0x20000002;
const HAP_PROPERTY_BLOCK_ID: u32 = 0x20000003;
const HAP_CODE_SIGN_BLOCK_ID: u32 = 0x30000001;

const HAP_SIGN_SCHEME_V2_BLOCK_VERSION: i32 = 2;
const HAP_SIGN_SCHEME_V3_BLOCK_VERSION: i32 = 3;
const MIN_COMPATIBLE_VERSION_FOR_SCHEMA_V3: i32 = 8;
const DEFAULT_COMPATIBLE_VERSION: i32 = 12;

const HAP_SIG_BLOCK_MAGIC_V2: [u8; 16] = [
    0x48, 0x41, 0x50, 0x20, 0x53, 0x69, 0x67, 0x20, 0x42, 0x6c, 0x6f, 0x63, 0x6b, 0x20, 0x34, 0x32,
];
const HAP_SIG_BLOCK_MAGIC_V3: [u8; 16] = [
    0x3c, 0x68, 0x61, 0x70, 0x20, 0x73, 0x69, 0x67, 0x6e, 0x20, 0x62, 0x6c, 0x6f, 0x63, 0x6b, 0x3e,
];

const ZIP_CHUNK_SIZE: usize = 1024 * 1024;
const ZIP_FIRST_LEVEL_CHUNK_PREFIX: u8 = 0x5a;
const ZIP_SECOND_LEVEL_CHUNK_PREFIX: u8 = 0xa5;

/// Configuration for HAP signing.
#[derive(Debug, Clone)]
pub struct SigningConfig {
    /// Path to keystore file (.p12/.pfx or PEM private key)
    pub keystore_path: PathBuf,
    /// Keystore password
    pub keystore_password: String,
    /// Key password (optional, defaults to keystore password if not provided)
    pub key_password: Option<String>,
    /// Path to certificate file (.cer/.pem)
    pub cert_path: PathBuf,
    /// Path to profile file (.p7b)
    pub profile_path: PathBuf,
    /// Signing algorithm
    pub sign_algorithm: SignAlgorithm,
}

/// Supported signing algorithms.
#[derive(Debug, Clone, Copy, Default)]
pub enum SignAlgorithm {
    #[default]
    SHA256withECDSA,
}

impl std::fmt::Display for SignAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SHA256withECDSA => write!(f, "SHA256withECDSA"),
        }
    }
}

/// Native Harmony signer.
pub struct HarmonySigner;

#[derive(Clone)]
struct ExistingSigningBlock {
    signing_block_offset: u64,
    optional_blocks: Vec<(u32, Vec<u8>)>,
}

impl HarmonySigner {
    pub fn new_native() -> Self {
        Self
    }

    /// Sign an unsigned HAP file.
    pub fn sign_hap(
        &self,
        config: &SigningConfig,
        input_path: &Path,
        output_path: &Path,
    ) -> Result<()> {
        self.sign_hap_native(config, input_path, output_path)
    }

    /// Verify a signed HAP file (structural verification).
    pub fn verify_hap(&self, hap_path: &Path) -> Result<String> {
        let zip_info = parse_zip(hap_path)?;
        if zip_info.cd_offset < 32 {
            return Err(anyhow!("HAP too small to contain signing block"));
        }

        let mut f = File::open(hap_path)
            .with_context(|| format!("Failed to open {}", hap_path.display()))?;
        let mut head = [0u8; 32];
        f.seek(SeekFrom::Start(zip_info.cd_offset - 32))?;
        f.read_exact(&mut head)?;

        // [blockCount:4][size:8][magic:16][version:4]
        let magic = &head[12..28];
        if magic != HAP_SIG_BLOCK_MAGIC_V2.as_slice() && magic != HAP_SIG_BLOCK_MAGIC_V3.as_slice()
        {
            return Err(anyhow!("HAP signing block magic not found"));
        }

        Ok("ok".to_string())
    }

    /// Sign a HAP file natively using Rust.
    pub fn sign_hap_native(
        &self,
        config: &SigningConfig,
        input_path: &Path,
        output_path: &Path,
    ) -> Result<()> {
        if !input_path.exists() {
            return Err(anyhow!("Input file not found: {}", input_path.display()));
        }
        if !config.profile_path.exists() {
            return Err(anyhow!(
                "Profile not found: {}",
                config.profile_path.display()
            ));
        }

        let zip_info = parse_zip(input_path)?;
        let profile_data = std::fs::read(&config.profile_path).with_context(|| {
            format!("Failed to read profile: {}", config.profile_path.display())
        })?;

        let existing = read_existing_signing_block(input_path, &zip_info)?;
        let signing_block_offset = existing
            .as_ref()
            .map(|block| block.signing_block_offset)
            .unwrap_or(zip_info.cd_offset);

        let mut optional_blocks = existing
            .map(|block| block.optional_blocks)
            .unwrap_or_default();
        let mut replaced_profile = false;
        for (typ, value) in &mut optional_blocks {
            if *typ == HAP_PROFILE_BLOCK_ID {
                *value = profile_data.clone();
                replaced_profile = true;
            }
        }
        if !replaced_profile {
            optional_blocks.push((HAP_PROFILE_BLOCK_ID, profile_data));
        }

        let md = message_digest_for(config.sign_algorithm);
        let algo_id = signature_algorithm_id(config.sign_algorithm);

        let digest = compute_hap_content_digest(
            input_path,
            &zip_info,
            signing_block_offset,
            md,
            &optional_blocks,
        )?;
        let encoded_digest_message = encode_digest_message(algo_id, &digest)?;

        let (pkey, signer_cert, cert_chain) = load_private_key_and_signer_cert(config)
            .context("Failed to load signing key/certificate for native signing")?;

        let mut certs: Stack<X509> = Stack::new()?;
        for cert in cert_chain {
            certs.push(cert)?;
        }
        let pkcs7 = Pkcs7::sign(
            &signer_cert,
            &pkey,
            &certs,
            &encoded_digest_message,
            Pkcs7Flags::BINARY | Pkcs7Flags::NOVERIFY,
        )
        .context("Failed to generate PKCS7 signed data")?;
        let signature_block = pkcs7.to_der().context("Failed to encode PKCS7")?;

        let hap_signing_block = construct_hap_signing_block(
            optional_blocks,
            signature_block,
            DEFAULT_COMPATIBLE_VERSION,
        );

        write_signed_hap(
            input_path,
            output_path,
            &zip_info,
            signing_block_offset,
            &hap_signing_block,
        )
    }
}

fn load_private_key_and_signer_cert(
    config: &SigningConfig,
) -> Result<(PKey<Private>, X509, Vec<X509>)> {
    let ext = config
        .keystore_path
        .extension()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();

    if ext == "p12" || ext == "pfx" {
        let bytes = std::fs::read(&config.keystore_path)
            .with_context(|| format!("Failed to read {}", config.keystore_path.display()))?;
        let p12 = Pkcs12::from_der(&bytes).context("Invalid PKCS12 file")?;
        let pass = config
            .key_password
            .as_deref()
            .unwrap_or(&config.keystore_password);

        let parsed = p12.parse2(pass).with_context(|| {
            format!(
                "Failed to parse PKCS12 using provided password for {}",
                config.keystore_path.display()
            )
        })?;

        let pkey = parsed
            .pkey
            .ok_or_else(|| anyhow!("No private key in PKCS12"))?;
        let (signer_cert, chain) =
            select_matching_signer_cert_and_chain(&pkey, &config.cert_path, parsed.cert.as_ref())?;
        return Ok((pkey, signer_cert, chain));
    }

    let key_bytes = std::fs::read(&config.keystore_path)
        .with_context(|| format!("Failed to read {}", config.keystore_path.display()))?;
    let pkey = if key_bytes.starts_with(b"-----BEGIN") {
        PKey::private_key_from_pem(&key_bytes).context("Invalid PEM private key")?
    } else {
        PKey::private_key_from_der(&key_bytes).context("Invalid DER private key")?
    };

    let (signer_cert, chain) =
        select_matching_signer_cert_and_chain(&pkey, &config.cert_path, None)?;
    Ok((pkey, signer_cert, chain))
}

fn select_matching_signer_cert_and_chain(
    private_key: &PKey<Private>,
    cert_path: &Path,
    p12_cert: Option<&X509>,
) -> Result<(X509, Vec<X509>)> {
    let mut certs = load_signer_certs(cert_path)?;
    if let Some(cert) = p12_cert {
        let exists = certs.iter().any(|c| c.to_der().ok() == cert.to_der().ok());
        if !exists {
            certs.push(cert.clone());
        }
    }

    for (idx, cert) in certs.iter().enumerate() {
        if cert_matches_private_key(private_key, cert)? {
            let signer = cert.clone();
            let chain = certs
                .into_iter()
                .enumerate()
                .filter_map(|(i, c)| if i == idx { None } else { Some(c) })
                .collect::<Vec<_>>();
            return Ok((signer, chain));
        }
    }

    Err(anyhow!(
        "No signing certificate in {} matches private key from keystore",
        cert_path.display()
    ))
}

fn cert_matches_private_key(private_key: &PKey<Private>, cert: &X509) -> Result<bool> {
    let cert_key = cert
        .public_key()
        .context("Failed to read public key from signing certificate")?;
    Ok(private_key.public_eq(&cert_key))
}

fn load_signer_certs(cert_path: &Path) -> Result<Vec<X509>> {
    let cert_bytes = std::fs::read(cert_path)
        .with_context(|| format!("Failed to read {}", cert_path.display()))?;

    if cert_bytes.starts_with(b"-----BEGIN") {
        let chain = X509::stack_from_pem(&cert_bytes).context("Invalid PEM certificate")?;
        if chain.is_empty() {
            return Err(anyhow!(
                "No certificate found in PEM: {}",
                cert_path.display()
            ));
        }
        Ok(chain)
    } else {
        let cert = X509::from_der(&cert_bytes).context("Invalid DER certificate")?;
        Ok(vec![cert])
    }
}

fn signature_algorithm_id(alg: SignAlgorithm) -> i32 {
    match alg {
        SignAlgorithm::SHA256withECDSA => 0x201,
    }
}

fn message_digest_for(alg: SignAlgorithm) -> MessageDigest {
    match alg {
        SignAlgorithm::SHA256withECDSA => MessageDigest::sha256(),
    }
}

fn encode_digest_message(alg_id: i32, digest: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(8 + 12 + digest.len());
    out.extend_from_slice(&2i32.to_le_bytes()); // content version
    out.extend_from_slice(&1i32.to_le_bytes()); // block number

    let pair_len = 4i32 + 4i32 + i32::try_from(digest.len()).context("Digest too large")?;
    out.extend_from_slice(&pair_len.to_le_bytes());
    out.extend_from_slice(&alg_id.to_le_bytes());
    out.extend_from_slice(
        &(i32::try_from(digest.len()).context("Digest too large")?).to_le_bytes(),
    );
    out.extend_from_slice(digest);
    Ok(out)
}

fn compute_hap_content_digest(
    input_path: &Path,
    zip_info: &crate::platform::harmony::signer::zip::ZipInfo,
    signing_block_offset: u64,
    md: MessageDigest,
    optional_blocks: &[(u32, Vec<u8>)],
) -> Result<Vec<u8>> {
    let mut chunk_digests = Vec::new();

    // segment 1: [0, signing_block_offset)
    digest_file_range_chunks(input_path, 0, signing_block_offset, md, &mut chunk_digests)?;

    // segment 2: central directory
    digest_file_range_chunks(
        input_path,
        zip_info.cd_offset,
        zip_info.cd_size,
        md,
        &mut chunk_digests,
    )?;

    // segment 3: eocd with CD offset patched to signing-block offset
    let mut eocd = zip_info.eocd_record.clone();
    let cd_offset_le = (signing_block_offset as u32).to_le_bytes();
    eocd[16..20].copy_from_slice(&cd_offset_le);
    digest_memory_chunks(&eocd, md, &mut chunk_digests)?;

    let digest_len = hash(md, &[])
        .context("Failed to determine digest length")?
        .as_ref()
        .len();

    let mut first_level = Vec::with_capacity(1 + 4 + chunk_digests.len() * digest_len);
    first_level.push(ZIP_FIRST_LEVEL_CHUNK_PREFIX);
    first_level.extend_from_slice(
        &(u32::try_from(chunk_digests.len()).context("Too many digest chunks")?).to_le_bytes(),
    );
    for d in &chunk_digests {
        first_level.extend_from_slice(d);
    }

    let mut hasher = Hasher::new(md)?;
    hasher.update(&first_level)?;
    for (_, value) in optional_blocks {
        hasher.update(value)?;
    }
    Ok(hasher.finish()?.to_vec())
}

fn digest_file_range_chunks(
    path: &Path,
    offset: u64,
    length: u64,
    md: MessageDigest,
    out: &mut Vec<Vec<u8>>,
) -> Result<()> {
    let mut f = File::open(path)?;
    f.seek(SeekFrom::Start(offset))?;

    let mut remaining = length;
    let mut buf = vec![0u8; ZIP_CHUNK_SIZE];
    while remaining > 0 {
        let to_read = usize::try_from(std::cmp::min(remaining, ZIP_CHUNK_SIZE as u64))?;
        f.read_exact(&mut buf[..to_read])?;
        digest_one_chunk(&buf[..to_read], md, out)?;
        remaining -= to_read as u64;
    }
    Ok(())
}

fn digest_memory_chunks(data: &[u8], md: MessageDigest, out: &mut Vec<Vec<u8>>) -> Result<()> {
    let mut start = 0usize;
    while start < data.len() {
        let end = std::cmp::min(start + ZIP_CHUNK_SIZE, data.len());
        digest_one_chunk(&data[start..end], md, out)?;
        start = end;
    }
    Ok(())
}

fn digest_one_chunk(chunk: &[u8], md: MessageDigest, out: &mut Vec<Vec<u8>>) -> Result<()> {
    let mut prefixed = Vec::with_capacity(1 + 4 + chunk.len());
    prefixed.push(ZIP_SECOND_LEVEL_CHUNK_PREFIX);
    prefixed
        .extend_from_slice(&(u32::try_from(chunk.len()).context("Chunk too large")?).to_le_bytes());
    prefixed.extend_from_slice(chunk);
    out.push(hash(md, &prefixed)?.to_vec());
    Ok(())
}

fn construct_hap_signing_block(
    optional_blocks: Vec<(u32, Vec<u8>)>,
    signature_scheme_block: Vec<u8>,
    compatible_version: i32,
) -> Vec<u8> {
    let mut blocks = optional_blocks;
    blocks.push((HAP_SIGNATURE_SCHEME_V1_BLOCK_ID, signature_scheme_block));

    let head_count = blocks.len();
    let head_bytes_len = head_count * 12;

    let mut headers = Vec::with_capacity(head_bytes_len);
    let mut values = Vec::new();
    let mut offset = head_bytes_len as u32;

    for (typ, value) in &blocks {
        headers.extend_from_slice(&typ.to_le_bytes());
        headers.extend_from_slice(&(value.len() as u32).to_le_bytes());
        headers.extend_from_slice(&offset.to_le_bytes());

        values.extend_from_slice(value);
        offset = offset.saturating_add(value.len() as u32);
    }

    let version = if compatible_version >= MIN_COMPATIBLE_VERSION_FOR_SCHEMA_V3 {
        HAP_SIGN_SCHEME_V3_BLOCK_VERSION
    } else {
        HAP_SIGN_SCHEME_V2_BLOCK_VERSION
    };
    let magic = if version == HAP_SIGN_SCHEME_V3_BLOCK_VERSION {
        HAP_SIG_BLOCK_MAGIC_V3
    } else {
        HAP_SIG_BLOCK_MAGIC_V2
    };

    let total_size = headers.len() + values.len() + 4 + 8 + 16 + 4;

    let mut out = Vec::with_capacity(total_size);
    out.extend_from_slice(&headers);
    out.extend_from_slice(&values);
    out.extend_from_slice(&(head_count as i32).to_le_bytes());
    out.extend_from_slice(&(total_size as i64).to_le_bytes());
    out.extend_from_slice(&magic);
    out.extend_from_slice(&version.to_le_bytes());
    out
}

fn write_signed_hap(
    input_path: &Path,
    output_path: &Path,
    zip_info: &crate::platform::harmony::signer::zip::ZipInfo,
    signing_block_offset: u64,
    signing_block: &[u8],
) -> Result<()> {
    let mut output_file = File::create(output_path)?;
    let mut input_file = File::open(input_path)?;

    // Content [0..old signing block offset)
    let mut buffer = vec![0u8; 4096];
    input_file.seek(SeekFrom::Start(0))?;
    let mut remaining = signing_block_offset;
    while remaining > 0 {
        let to_read = std::cmp::min(remaining, buffer.len() as u64) as usize;
        input_file.read_exact(&mut buffer[0..to_read])?;
        output_file.write_all(&buffer[0..to_read])?;
        remaining -= to_read as u64;
    }

    // Signing block
    output_file.write_all(signing_block)?;

    // Central Directory
    input_file.seek(SeekFrom::Start(zip_info.cd_offset))?;
    let mut remaining = zip_info.cd_size;
    while remaining > 0 {
        let to_read = std::cmp::min(remaining, buffer.len() as u64) as usize;
        input_file.read_exact(&mut buffer[0..to_read])?;
        output_file.write_all(&buffer[0..to_read])?;
        remaining -= to_read as u64;
    }

    // EOCD with updated CD offset
    let mut eocd = zip_info.eocd_record.clone();
    let new_cd_offset = signing_block_offset + signing_block.len() as u64;
    eocd[16..20].copy_from_slice(&(new_cd_offset as u32).to_le_bytes());
    output_file.write_all(&eocd)?;

    Ok(())
}

fn read_existing_signing_block(
    path: &Path,
    zip_info: &crate::platform::harmony::signer::zip::ZipInfo,
) -> Result<Option<ExistingSigningBlock>> {
    if zip_info.cd_offset < 32 {
        return Ok(None);
    }

    let mut f = File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let mut tail = [0u8; 32];
    f.seek(SeekFrom::Start(zip_info.cd_offset - 32))?;
    f.read_exact(&mut tail)?;

    let block_count = i32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]);
    let block_size = i64::from_le_bytes([
        tail[4], tail[5], tail[6], tail[7], tail[8], tail[9], tail[10], tail[11],
    ]);
    if block_count <= 0 || block_size < 32 {
        return Ok(None);
    }

    let magic = &tail[12..28];
    if magic != HAP_SIG_BLOCK_MAGIC_V2.as_slice() && magic != HAP_SIG_BLOCK_MAGIC_V3.as_slice() {
        return Ok(None);
    }

    let block_size_u = usize::try_from(block_size).context("Invalid signing block size")?;
    let block_size_u64 = u64::try_from(block_size).context("Invalid signing block size")?;
    if block_size_u64 > zip_info.cd_offset {
        return Ok(None);
    }

    let block_offset = zip_info.cd_offset - block_size_u64;
    let mut block = vec![0u8; block_size_u];
    f.seek(SeekFrom::Start(block_offset))?;
    f.read_exact(&mut block)?;

    let count = usize::try_from(block_count).context("Invalid signing block count")?;
    let headers_len = count
        .checked_mul(12)
        .ok_or_else(|| anyhow!("Signing block header overflow"))?;
    if headers_len + 32 > block.len() {
        return Ok(None);
    }

    let values_end = block.len() - 32;
    let mut out = Vec::new();
    for idx in 0..count {
        let base = idx * 12;
        let typ = u32::from_le_bytes([
            block[base],
            block[base + 1],
            block[base + 2],
            block[base + 3],
        ]);
        let len = u32::from_le_bytes([
            block[base + 4],
            block[base + 5],
            block[base + 6],
            block[base + 7],
        ]) as usize;
        let offset = u32::from_le_bytes([
            block[base + 8],
            block[base + 9],
            block[base + 10],
            block[base + 11],
        ]) as usize;

        if typ == HAP_SIGNATURE_SCHEME_V1_BLOCK_ID {
            continue;
        }
        if typ != HAP_PROFILE_BLOCK_ID
            && typ != HAP_PROOF_OF_ROTATION_BLOCK_ID
            && typ != HAP_PROPERTY_BLOCK_ID
            && typ != HAP_CODE_SIGN_BLOCK_ID
        {
            continue;
        }
        if offset < headers_len {
            continue;
        }
        let end = offset.saturating_add(len);
        if end > values_end {
            continue;
        }
        out.push((typ, block[offset..end].to_vec()));
    }

    Ok(Some(ExistingSigningBlock {
        signing_block_offset: zip_info.cd_offset - block_size_u64,
        optional_blocks: out,
    }))
}
