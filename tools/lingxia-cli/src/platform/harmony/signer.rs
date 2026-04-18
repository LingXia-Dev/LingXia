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
const CODE_SIGN_BLOCK_MAGIC: u64 = 0xe046_c8c6_5389_fccd;
const CODE_SIGN_BLOCK_VERSION: u32 = 1;
const CODE_SIGN_BLOCK_HEADER_SIZE: usize = 32;
const CODE_SIGN_SEGMENT_HEADER_SIZE: usize = 12;
const CODE_SIGN_SEGMENT_COUNT: usize = 3;
const CODE_SIGN_FLAG_MERKLE_TREE_INLINED: u32 = 1;
const CODE_SIGN_FLAG_NATIVE_LIB_INCLUDED: u32 = 2;
const CODE_SIGN_FSVERITY_SEGMENT_TYPE: u32 = 1;
const CODE_SIGN_HAP_SEGMENT_TYPE: u32 = 2;
const CODE_SIGN_NATIVE_LIB_SEGMENT_TYPE: u32 = 3;
const FSVERITY_INFO_MAGIC: u32 = 0x1e38_31ab;
const HAP_INFO_MAGIC: u32 = 0xc1b5_cc66;
const NATIVE_LIB_INFO_MAGIC: u32 = 0x0ed2_e720;
const FSVERITY_HASH_ALGORITHM_SHA256: u8 = 1;
const FSVERITY_VERSION: u8 = 1;
const FSVERITY_LOG2_BLOCK_SIZE: u8 = 12;
const FSVERITY_BLOCK_SIZE: usize = 4096;
const FSVERITY_DIGEST_MAGIC: &[u8; 8] = b"FSVerity";
const SIGN_INFO_BASE_SIZE: usize = 60;
const MERKLE_TREE_EXTENSION_TYPE: u32 = 1;
const MERKLE_TREE_EXTENSION_PAYLOAD_SIZE: u32 = 80;

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
        optional_blocks
            .retain(|(typ, _)| *typ != HAP_PROPERTY_BLOCK_ID && *typ != HAP_CODE_SIGN_BLOCK_ID);
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

        let (pkey, signer_cert, cert_chain) = load_private_key_and_signer_cert(config)
            .context("Failed to load signing key/certificate for native signing")?;
        let property_header_count = optional_blocks.len() + 2;
        let code_sign_block_offset =
            signing_block_offset + (property_header_count * 12) as u64 + 12;
        let property_block = generate_code_sign_property_block(
            input_path,
            &zip_info,
            signing_block_offset,
            code_sign_block_offset,
            &pkey,
            &signer_cert,
            &cert_chain,
        )?;
        optional_blocks.insert(0, (HAP_PROPERTY_BLOCK_ID, property_block));

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

        let signature_block = pkcs7_sign_der(
            &encoded_digest_message,
            &pkey,
            &signer_cert,
            &cert_chain,
            false,
        )?;

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

fn pkcs7_sign_der(
    data: &[u8],
    pkey: &PKey<Private>,
    signer_cert: &X509,
    cert_chain: &[X509],
    detached: bool,
) -> Result<Vec<u8>> {
    let mut certs: Stack<X509> = Stack::new()?;
    for cert in cert_chain {
        certs.push(cert.clone())?;
    }
    let mut flags = Pkcs7Flags::BINARY | Pkcs7Flags::NOVERIFY;
    if detached {
        flags |= Pkcs7Flags::DETACHED;
    }
    let pkcs7 = Pkcs7::sign(signer_cert, pkey, &certs, data, flags)
        .context("Failed to generate PKCS7 signed data")?;
    pkcs7.to_der().context("Failed to encode PKCS7")
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

#[derive(Debug, Clone)]
struct ZipLocalEntry {
    name: String,
    local_offset: u64,
    data_offset: u64,
    compressed_size: u64,
    uncompressed_size: u64,
    method: u16,
}

fn generate_code_sign_property_block(
    input_path: &Path,
    zip_info: &crate::platform::harmony::signer::zip::ZipInfo,
    signing_block_offset: u64,
    code_sign_block_offset: u64,
    pkey: &PKey<Private>,
    signer_cert: &X509,
    cert_chain: &[X509],
) -> Result<Vec<u8>> {
    let entries = parse_local_zip_entries(input_path, zip_info.cd_offset)?;
    let hap_data_size = compute_hap_code_sign_data_size(&entries)?;
    if hap_data_size > signing_block_offset {
        return Err(anyhow!(
            "computed Harmony code signing data size {} exceeds signing block offset {}",
            hap_data_size,
            signing_block_offset
        ));
    }
    let merkle_tree_offset = compute_code_sign_merkle_tree_offset(code_sign_block_offset);
    let hap_bytes = read_file_prefix(input_path, hap_data_size)?;
    let hap_fsverity = fsverity_for_bytes(&hap_bytes, true, merkle_tree_offset);
    let hap_signature = pkcs7_sign_der(&hap_fsverity.digest, pkey, signer_cert, cert_chain, true)?;
    let hap_sign_info = encode_sign_info(
        hap_data_size,
        true,
        &hap_signature,
        Some(encode_merkle_tree_extension(
            hap_fsverity.tree.len() as u64,
            merkle_tree_offset,
            &hap_fsverity.root_hash,
        )),
    );

    let mut native_entries = Vec::new();
    for entry in &entries {
        if !is_native_code_entry(entry) {
            continue;
        }
        let bytes = read_zip_entry_bytes(input_path, entry)?;
        let fsverity = fsverity_for_bytes(&bytes, false, 0);
        let signature = pkcs7_sign_der(&fsverity.digest, pkey, signer_cert, cert_chain, true)?;
        let sign_info = encode_sign_info(entry.uncompressed_size, false, &signature, None);
        native_entries.push((entry.name.clone(), sign_info));
    }

    let native_segment = encode_native_lib_info_segment(&native_entries);
    let fsverity_segment = encode_fsverity_info_segment();
    let hap_segment = encode_hap_info_segment(&hap_sign_info);

    let mut block = encode_code_sign_block(
        code_sign_block_offset,
        &hap_fsverity.tree,
        &fsverity_segment,
        &hap_segment,
        &native_segment,
    );

    let mut property = Vec::with_capacity(12 + block.len());
    property.extend_from_slice(&HAP_CODE_SIGN_BLOCK_ID.to_le_bytes());
    property.extend_from_slice(
        &(u32::try_from(block.len()).context("code sign block too large")?).to_le_bytes(),
    );
    property.extend_from_slice(
        &(u32::try_from(code_sign_block_offset).context("code sign block offset too large")?)
            .to_le_bytes(),
    );
    property.append(&mut block);
    Ok(property)
}

fn parse_local_zip_entries(input_path: &Path, cd_offset: u64) -> Result<Vec<ZipLocalEntry>> {
    let mut file = File::open(input_path)?;
    let mut entries = Vec::new();
    let mut offset = 0u64;
    while offset < cd_offset {
        file.seek(SeekFrom::Start(offset))?;
        let mut header = [0u8; 30];
        file.read_exact(&mut header)?;
        if u32::from_le_bytes([header[0], header[1], header[2], header[3]]) != 0x0403_4b50 {
            break;
        }
        let method = u16::from_le_bytes([header[8], header[9]]);
        let compressed_size =
            u32::from_le_bytes([header[18], header[19], header[20], header[21]]) as u64;
        let uncompressed_size =
            u32::from_le_bytes([header[22], header[23], header[24], header[25]]) as u64;
        let name_len = u16::from_le_bytes([header[26], header[27]]) as usize;
        let extra_len = u16::from_le_bytes([header[28], header[29]]) as usize;
        let mut name = vec![0u8; name_len];
        file.read_exact(&mut name)?;
        let name = String::from_utf8_lossy(&name).into_owned();
        let data_offset = offset + 30 + name_len as u64 + extra_len as u64;
        entries.push(ZipLocalEntry {
            name,
            local_offset: offset,
            data_offset,
            compressed_size,
            uncompressed_size,
            method,
        });
        offset = data_offset + compressed_size;
    }
    Ok(entries)
}

fn compute_hap_code_sign_data_size(entries: &[ZipLocalEntry]) -> Result<u64> {
    let mut data_size = 0;
    for entry in entries {
        if is_runnable_entry(entry) && entry.method == 0 {
            continue;
        }
        if entry.name == ".pages.info" {
            continue;
        }
        if entry.local_offset == 0 {
            break;
        }
        data_size = entry.data_offset;
        break;
    }
    if data_size % FSVERITY_BLOCK_SIZE as u64 != 0 {
        return Err(anyhow!(
            "Harmony code signing data size must be 4K aligned: {}",
            data_size
        ));
    }
    Ok(data_size)
}

fn is_runnable_entry(entry: &ZipLocalEntry) -> bool {
    entry.name.ends_with(".abc") || entry.name.ends_with(".an") || entry.name.starts_with("libs/")
}

fn is_native_code_entry(entry: &ZipLocalEntry) -> bool {
    entry.method == 0
        && !entry.name.ends_with('/')
        && (entry.name.starts_with("libs/") || entry.name.ends_with(".an"))
}

fn read_file_prefix(path: &Path, len: u64) -> Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let mut out = vec![0u8; usize::try_from(len).context("file prefix too large")?];
    file.read_exact(&mut out)?;
    Ok(out)
}

fn read_zip_entry_bytes(path: &Path, entry: &ZipLocalEntry) -> Result<Vec<u8>> {
    if entry.method != 0 || entry.compressed_size != entry.uncompressed_size {
        return Err(anyhow!(
            "code signing only supports stored native entries: {}",
            entry.name
        ));
    }
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(entry.data_offset))?;
    let mut out = vec![0u8; usize::try_from(entry.uncompressed_size).context("entry too large")?];
    file.read_exact(&mut out)?;
    Ok(out)
}

struct FsVerityData {
    digest: Vec<u8>,
    root_hash: Vec<u8>,
    tree: Vec<u8>,
}

fn fsverity_for_bytes(data: &[u8], include_tree: bool, merkle_tree_offset: u64) -> FsVerityData {
    let (root_hash, tree) = fsverity_merkle_tree(data);
    let descriptor = fsverity_descriptor(
        data.len() as u64,
        &root_hash,
        if include_tree { 1 } else { 0 },
        if include_tree { merkle_tree_offset } else { 0 },
    );
    let descriptor_digest = hash(MessageDigest::sha256(), &descriptor)
        .expect("sha256 descriptor digest")
        .to_vec();
    let mut digest = Vec::with_capacity(12 + descriptor_digest.len());
    digest.extend_from_slice(FSVERITY_DIGEST_MAGIC);
    digest.extend_from_slice(&(FSVERITY_HASH_ALGORITHM_SHA256 as u16).to_le_bytes());
    digest.extend_from_slice(&(descriptor_digest.len() as u16).to_le_bytes());
    digest.extend_from_slice(&descriptor_digest);
    FsVerityData {
        digest,
        root_hash,
        tree,
    }
}

fn fsverity_merkle_tree(data: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut level = hash_4096_pages(data);
    if data.len() <= FSVERITY_BLOCK_SIZE {
        return (
            level.into_iter().next().unwrap_or_else(|| vec![0u8; 32]),
            Vec::new(),
        );
    }

    let mut levels = Vec::new();
    while level.len() > 1 {
        let mut level_bytes = level.concat();
        pad_to_4096(&mut level_bytes);
        levels.push(level_bytes.clone());
        level = hash_4096_pages(&level_bytes);
    }

    let mut tree = Vec::new();
    for level_bytes in levels.iter().rev() {
        tree.extend_from_slice(level_bytes);
    }
    (level.remove(0), tree)
}

fn hash_4096_pages(data: &[u8]) -> Vec<Vec<u8>> {
    if data.is_empty() {
        return vec![
            hash(MessageDigest::sha256(), &[0u8; FSVERITY_BLOCK_SIZE])
                .expect("sha256 empty fsverity page")
                .to_vec(),
        ];
    }
    let mut out = Vec::new();
    for chunk in data.chunks(FSVERITY_BLOCK_SIZE) {
        let mut page = Vec::with_capacity(FSVERITY_BLOCK_SIZE);
        page.extend_from_slice(chunk);
        page.resize(FSVERITY_BLOCK_SIZE, 0);
        out.push(
            hash(MessageDigest::sha256(), &page)
                .expect("sha256 fsverity page")
                .to_vec(),
        );
    }
    out
}

fn pad_to_4096(bytes: &mut Vec<u8>) {
    let rem = bytes.len() % FSVERITY_BLOCK_SIZE;
    if rem != 0 {
        bytes.resize(bytes.len() + FSVERITY_BLOCK_SIZE - rem, 0);
    }
}

fn fsverity_descriptor(
    file_size: u64,
    root_hash: &[u8],
    flags: u32,
    merkle_tree_offset: u64,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(256);
    out.push(1);
    out.push(FSVERITY_HASH_ALGORITHM_SHA256);
    out.push(FSVERITY_LOG2_BLOCK_SIZE);
    out.push(0);
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&file_size.to_le_bytes());
    out.extend_from_slice(&fixed_bytes(root_hash, 64));
    out.extend_from_slice(&[0u8; 32]);
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&merkle_tree_offset.to_le_bytes());
    out.resize(256, 0);
    out
}

fn fixed_bytes(bytes: &[u8], len: usize) -> Vec<u8> {
    let mut out = vec![0u8; len];
    let copy_len = bytes.len().min(len);
    out[..copy_len].copy_from_slice(&bytes[..copy_len]);
    out
}

fn encode_sign_info(
    data_size: u64,
    include_tree: bool,
    signature: &[u8],
    extension: Option<Vec<u8>>,
) -> Vec<u8> {
    let zero_padding_len = (4 - signature.len() % 4) % 4;
    let extension_num = usize::from(extension.is_some());
    let extension_offset = SIGN_INFO_BASE_SIZE + signature.len() + zero_padding_len;
    let mut out = Vec::with_capacity(extension_offset + extension.as_ref().map_or(0, Vec::len));
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(signature.len() as u32).to_le_bytes());
    out.extend_from_slice(&(if include_tree { 1u32 } else { 0u32 }).to_le_bytes());
    out.extend_from_slice(&data_size.to_le_bytes());
    out.extend_from_slice(&[0u8; 32]);
    out.extend_from_slice(&(extension_num as u32).to_le_bytes());
    out.extend_from_slice(&(extension_offset as u32).to_le_bytes());
    out.extend_from_slice(signature);
    out.extend(std::iter::repeat(0).take(zero_padding_len));
    if let Some(extension) = extension {
        out.extend_from_slice(&extension);
    }
    out
}

fn encode_merkle_tree_extension(tree_size: u64, tree_offset: u64, root_hash: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + MERKLE_TREE_EXTENSION_PAYLOAD_SIZE as usize);
    out.extend_from_slice(&MERKLE_TREE_EXTENSION_TYPE.to_le_bytes());
    out.extend_from_slice(&MERKLE_TREE_EXTENSION_PAYLOAD_SIZE.to_le_bytes());
    out.extend_from_slice(&tree_size.to_le_bytes());
    out.extend_from_slice(&tree_offset.to_le_bytes());
    out.extend_from_slice(&fixed_bytes(root_hash, 64));
    out.resize(8 + MERKLE_TREE_EXTENSION_PAYLOAD_SIZE as usize, 0);
    out
}

fn encode_fsverity_info_segment() -> Vec<u8> {
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(&FSVERITY_INFO_MAGIC.to_le_bytes());
    out.push(FSVERITY_HASH_ALGORITHM_SHA256);
    out.push(FSVERITY_VERSION);
    out.push(FSVERITY_LOG2_BLOCK_SIZE);
    out.resize(64, 0);
    out
}

fn encode_hap_info_segment(sign_info: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + sign_info.len());
    out.extend_from_slice(&HAP_INFO_MAGIC.to_le_bytes());
    out.extend_from_slice(sign_info);
    out
}

fn encode_native_lib_info_segment(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let section_num = entries.len();
    let header_len = 12 + section_num * 16;
    let mut file_names = Vec::new();
    let mut sign_infos = Vec::new();
    let mut positions = Vec::new();

    let mut name_offset = header_len;
    let mut sign_offset = header_len
        + entries
            .iter()
            .map(|(name, _)| name.as_bytes().len())
            .sum::<usize>();
    let sign_padding = (4 - sign_offset % 4) % 4;
    sign_offset += sign_padding;

    for (name, sign_info) in entries {
        let name_bytes = name.as_bytes();
        positions.push((name_offset, name_bytes.len(), sign_offset, sign_info.len()));
        file_names.extend_from_slice(name_bytes);
        name_offset += name_bytes.len();
        sign_infos.extend_from_slice(sign_info);
        sign_offset += sign_info.len();
    }
    file_names.extend(std::iter::repeat(0).take(sign_padding));

    let segment_size = 12 + positions.len() * 16 + file_names.len() + sign_infos.len();
    let mut out = Vec::with_capacity(segment_size);
    out.extend_from_slice(&NATIVE_LIB_INFO_MAGIC.to_le_bytes());
    out.extend_from_slice(&(segment_size as u32).to_le_bytes());
    out.extend_from_slice(&(section_num as u32).to_le_bytes());
    for (name_offset, name_size, sign_offset, sign_size) in positions {
        out.extend_from_slice(&(name_offset as u32).to_le_bytes());
        out.extend_from_slice(&(name_size as u32).to_le_bytes());
        out.extend_from_slice(&(sign_offset as u32).to_le_bytes());
        out.extend_from_slice(&(sign_size as u32).to_le_bytes());
    }
    out.extend_from_slice(&file_names);
    out.extend_from_slice(&sign_infos);
    out
}

fn compute_code_sign_merkle_tree_offset(code_sign_block_offset: u64) -> u64 {
    let base =
        CODE_SIGN_BLOCK_HEADER_SIZE + CODE_SIGN_SEGMENT_COUNT * CODE_SIGN_SEGMENT_HEADER_SIZE;
    let residual = (code_sign_block_offset + base as u64) % FSVERITY_BLOCK_SIZE as u64;
    let padding = if residual == 0 {
        0
    } else {
        FSVERITY_BLOCK_SIZE as u64 - residual
    };
    code_sign_block_offset + base as u64 + padding
}

fn encode_code_sign_block(
    code_sign_block_offset: u64,
    hap_merkle_tree: &[u8],
    fsverity_segment: &[u8],
    hap_segment: &[u8],
    native_segment: &[u8],
) -> Vec<u8> {
    let base =
        CODE_SIGN_BLOCK_HEADER_SIZE + CODE_SIGN_SEGMENT_COUNT * CODE_SIGN_SEGMENT_HEADER_SIZE;
    let residual = (code_sign_block_offset + base as u64) % FSVERITY_BLOCK_SIZE as u64;
    let padding_len = if residual == 0 {
        0
    } else {
        FSVERITY_BLOCK_SIZE - residual as usize
    };

    let fsverity_offset = base + padding_len + hap_merkle_tree.len();
    let hap_offset = fsverity_offset + fsverity_segment.len();
    let native_offset = hap_offset + hap_segment.len();
    let block_size = native_offset + native_segment.len();

    let mut out = Vec::with_capacity(block_size);
    out.extend_from_slice(&CODE_SIGN_BLOCK_MAGIC.to_le_bytes());
    out.extend_from_slice(&CODE_SIGN_BLOCK_VERSION.to_le_bytes());
    out.extend_from_slice(&(block_size as u32).to_le_bytes());
    out.extend_from_slice(&(CODE_SIGN_SEGMENT_COUNT as u32).to_le_bytes());
    let flags = CODE_SIGN_FLAG_MERKLE_TREE_INLINED
        | if native_segment.len() > 12 {
            CODE_SIGN_FLAG_NATIVE_LIB_INCLUDED
        } else {
            0
        };
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&0u64.to_le_bytes());
    for (typ, offset, size) in [
        (
            CODE_SIGN_FSVERITY_SEGMENT_TYPE,
            fsverity_offset,
            fsverity_segment.len(),
        ),
        (CODE_SIGN_HAP_SEGMENT_TYPE, hap_offset, hap_segment.len()),
        (
            CODE_SIGN_NATIVE_LIB_SEGMENT_TYPE,
            native_offset,
            native_segment.len(),
        ),
    ] {
        out.extend_from_slice(&typ.to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        out.extend_from_slice(&(size as u32).to_le_bytes());
    }
    out.extend(std::iter::repeat(0).take(padding_len));
    out.extend_from_slice(hap_merkle_tree);
    out.extend_from_slice(fsverity_segment);
    out.extend_from_slice(hap_segment);
    out.extend_from_slice(native_segment);
    out
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

#[cfg(test)]
mod tests {
    use super::{
        FSVERITY_BLOCK_SIZE, ZipLocalEntry, compute_hap_code_sign_data_size, is_runnable_entry,
    };

    fn entry(name: &str, local_offset: u64, data_offset: u64, method: u16) -> ZipLocalEntry {
        ZipLocalEntry {
            name: name.to_string(),
            local_offset,
            data_offset,
            compressed_size: 0,
            uncompressed_size: 0,
            method,
        }
    }

    #[test]
    fn hap_code_sign_data_size_matches_hapsigner_boundary() {
        let entries = [
            entry("libs/arm64-v8a/liblingxia.so", 0, 64, 0),
            entry("ets/modules.abc", 4096, 4160, 0),
            entry("module.json", 8192, 8192, 8),
        ];

        assert!(is_runnable_entry(&entries[0]));
        assert_eq!(compute_hap_code_sign_data_size(&entries).unwrap(), 8192);
    }

    #[test]
    fn hap_code_sign_data_size_rejects_unaligned_boundary() {
        let entries = [entry(
            "module.json",
            FSVERITY_BLOCK_SIZE as u64,
            FSVERITY_BLOCK_SIZE as u64 + 64,
            8,
        )];

        assert!(compute_hap_code_sign_data_size(&entries).is_err());
    }
}
