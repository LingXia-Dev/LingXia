#![cfg_attr(target_os = "windows", allow(dead_code))]

use anyhow::{Context, Result};
use rcgen::{CertificateParams, DistinguishedName, DnType, IsCa, KeyPair, KeyUsagePurpose};

/// Information for the CSR subject.
pub struct CsrSubject {
    pub common_name: String,
    pub organization: String,
    pub country: String,
}

/// Generate a new EC P-256 key pair and a CSR.
///
/// Returns the (private_key_pem, csr_pem).
pub fn generate_ec_csr(subject: &CsrSubject) -> Result<(String, String)> {
    // 1. Generate Key Pair (P-256)
    let key_pair = KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)
        .context("Failed to generate EC P-256 key pair")?;
    let private_key_pem = key_pair.serialize_pem();

    // 2. Configure CSR parameters
    let mut params = CertificateParams::default();
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, &subject.common_name);
    dn.push(DnType::OrganizationName, &subject.organization);
    dn.push(DnType::CountryName, &subject.country);

    params.distinguished_name = dn;
    params.is_ca = IsCa::NoCa;
    params.key_usages = vec![KeyUsagePurpose::DigitalSignature];

    // 3. Generate CSR
    let csr = params
        .serialize_request(&key_pair)
        .context("Failed to generate CSR")?;
    let csr_pem = csr.pem().context("Failed to serialize CSR to PEM")?;

    Ok((private_key_pem, csr_pem))
}
