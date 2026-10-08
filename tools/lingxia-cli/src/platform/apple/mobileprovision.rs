//! Provisioning profile (`.mobileprovision`) decoding.
//!
//! A provisioning profile is a CMS `SignedData` envelope whose encapsulated
//! content is the profile's XML plist. The payload is read straight out of the
//! DER structure: `security cms -D` imports the signer certificates into the
//! default keychain while decoding, so it fails on hosts without one (a CI
//! runner whose `HOME` is overridden reports "A default keychain could not be
//! found" and "problem decoding"). The signature does not need verifying here;
//! `codesign` and App Store Connect validate the profile itself.

use anyhow::{Context, Result, anyhow, bail};
use std::process::Command;

const TAG_SEQUENCE: u8 = 0x30;
const TAG_SET: u8 = 0x31;
const TAG_INTEGER: u8 = 0x02;
const TAG_OID: u8 = 0x06;
const TAG_OCTET_STRING: u8 = 0x04;
const TAG_OCTET_STRING_CONSTRUCTED: u8 = 0x24;
const TAG_CONTEXT_0: u8 = 0xA0;

/// DER encoding of the OID 1.2.840.113549.1.7.2 (`id-signedData`).
const OID_SIGNED_DATA: &[u8] = &[0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x07, 0x02];

/// Decode a provisioning profile into its plist.
pub fn decode_plist(profile_data: &[u8]) -> Result<plist::Value> {
    let payload = match extract_payload(profile_data) {
        Ok(payload) => payload,
        Err(parse_err) => decode_with_security(profile_data).map_err(|security_err| {
            anyhow!(
                "Failed to decode provisioning profile ({} bytes): {parse_err:#}; \
                 `security cms -D` fallback: {security_err:#}",
                profile_data.len()
            )
        })?,
    };
    plist::from_bytes(&payload).context("Failed to parse provisioning profile plist")
}

/// Return the encapsulated content (the plist bytes) of a CMS SignedData blob.
fn extract_payload(data: &[u8]) -> Result<Vec<u8>> {
    let mut reader = Der::new(data);
    // ContentInfo ::= SEQUENCE { contentType OID, content [0] EXPLICIT ANY }
    let mut content_info = reader.enter(TAG_SEQUENCE, "ContentInfo")?;
    let oid = content_info.read(TAG_OID, "ContentInfo.contentType")?;
    if oid != OID_SIGNED_DATA {
        bail!("not a CMS SignedData envelope");
    }
    let mut explicit = content_info.enter(TAG_CONTEXT_0, "ContentInfo.content")?;
    // SignedData ::= SEQUENCE { version, digestAlgorithms SET, encapContentInfo, ... }
    let mut signed_data = explicit.enter(TAG_SEQUENCE, "SignedData")?;
    signed_data.read(TAG_INTEGER, "SignedData.version")?;
    signed_data.read(TAG_SET, "SignedData.digestAlgorithms")?;
    // EncapsulatedContentInfo ::= SEQUENCE { eContentType OID, eContent [0] EXPLICIT OCTET STRING }
    let mut encap = signed_data.enter(TAG_SEQUENCE, "EncapsulatedContentInfo")?;
    encap.read(TAG_OID, "EncapsulatedContentInfo.eContentType")?;
    let mut econtent = encap.enter(TAG_CONTEXT_0, "EncapsulatedContentInfo.eContent")?;
    let payload = econtent.octet_string("eContent")?;
    if payload.is_empty() {
        bail!("provisioning profile has no embedded content");
    }
    Ok(payload)
}

fn decode_with_security(profile_data: &[u8]) -> Result<Vec<u8>> {
    use std::io::Write;
    let mut file = tempfile::NamedTempFile::new().context("Failed to create profile temp file")?;
    file.write_all(profile_data)
        .context("Failed to write profile temp file")?;
    let output = Command::new("security")
        .args(["cms", "-D", "-i"])
        .arg(file.path())
        .output()
        .context("Failed to run `security cms -D`")?;
    if !output.status.success() {
        bail!(
            "{}",
            String::from_utf8_lossy(&output.stderr)
                .trim()
                .replace('\n', "; ")
        );
    }
    Ok(output.stdout)
}

/// Minimal DER reader over definite-length TLVs.
struct Der<'a> {
    data: &'a [u8],
}

impl<'a> Der<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    fn next(&mut self, what: &str) -> Result<(u8, &'a [u8])> {
        let truncated = || anyhow!("truncated DER while reading {what}");
        let (&tag, rest) = self.data.split_first().ok_or_else(truncated)?;
        let (&first, mut rest) = rest.split_first().ok_or_else(truncated)?;
        let len = if first < 0x80 {
            first as usize
        } else {
            let count = (first & 0x7F) as usize;
            if count == 0 {
                bail!("indefinite-length DER is not supported ({what})");
            }
            if count > std::mem::size_of::<usize>() || rest.len() < count {
                return Err(truncated());
            }
            let (len_bytes, tail) = rest.split_at(count);
            rest = tail;
            len_bytes
                .iter()
                .fold(0usize, |acc, &b| (acc << 8) | b as usize)
        };
        if rest.len() < len {
            return Err(truncated());
        }
        let (value, tail) = rest.split_at(len);
        self.data = tail;
        Ok((tag, value))
    }

    fn read(&mut self, expected: u8, what: &str) -> Result<&'a [u8]> {
        let (tag, value) = self.next(what)?;
        if tag != expected {
            bail!("unexpected DER tag 0x{tag:02X} for {what} (expected 0x{expected:02X})");
        }
        Ok(value)
    }

    fn enter(&mut self, expected: u8, what: &str) -> Result<Der<'a>> {
        self.read(expected, what).map(Der::new)
    }

    /// Read an OCTET STRING, joining the segments of a constructed one.
    fn octet_string(&mut self, what: &str) -> Result<Vec<u8>> {
        let (tag, value) = self.next(what)?;
        match tag {
            TAG_OCTET_STRING => Ok(value.to_vec()),
            TAG_OCTET_STRING_CONSTRUCTED => {
                let mut inner = Der::new(value);
                let mut out = Vec::new();
                while !inner.data.is_empty() {
                    out.extend(inner.octet_string(what)?);
                }
                Ok(out)
            }
            _ => bail!("unexpected DER tag 0x{tag:02X} for {what} (expected OCTET STRING)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tlv(tag: u8, value: &[u8]) -> Vec<u8> {
        let mut out = vec![tag];
        let len = value.len();
        if len < 0x80 {
            out.push(len as u8);
        } else {
            let bytes: Vec<u8> = len
                .to_be_bytes()
                .into_iter()
                .skip_while(|b| *b == 0)
                .collect();
            out.push(0x80 | bytes.len() as u8);
            out.extend(bytes);
        }
        out.extend_from_slice(value);
        out
    }

    fn signed_data(econtent: Vec<u8>) -> Vec<u8> {
        // id-data
        let data_oid = [0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x07, 0x01];
        let encap = tlv(
            TAG_SEQUENCE,
            &[tlv(TAG_OID, &data_oid), tlv(TAG_CONTEXT_0, &econtent)].concat(),
        );
        let signed = tlv(
            TAG_SEQUENCE,
            &[
                tlv(TAG_INTEGER, &[1]),
                tlv(TAG_SET, &[]),
                encap,
                tlv(TAG_SET, &[]), // signerInfos (unused by the reader)
            ]
            .concat(),
        );
        tlv(
            TAG_SEQUENCE,
            &[tlv(TAG_OID, OID_SIGNED_DATA), tlv(TAG_CONTEXT_0, &signed)].concat(),
        )
    }

    const PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>Name</key><string>LingXia Store Test</string>
<key>Entitlements</key><dict><key>application-identifier</key><string>TEAM.com.example.app</string></dict>
</dict></plist>"#;

    #[test]
    fn decodes_signed_data_payload() {
        let profile = signed_data(tlv(TAG_OCTET_STRING, PLIST.as_bytes()));
        let plist = decode_plist(&profile).unwrap();
        let name = plist
            .as_dictionary()
            .and_then(|d| d.get("Name"))
            .and_then(plist::Value::as_string);
        assert_eq!(name, Some("LingXia Store Test"));
    }

    #[test]
    fn joins_constructed_octet_string() {
        let (a, b) = PLIST.as_bytes().split_at(40);
        let constructed = tlv(
            TAG_OCTET_STRING_CONSTRUCTED,
            &[tlv(TAG_OCTET_STRING, a), tlv(TAG_OCTET_STRING, b)].concat(),
        );
        assert_eq!(
            extract_payload(&signed_data(constructed)).unwrap(),
            PLIST.as_bytes()
        );
    }

    #[test]
    fn rejects_non_cms_input() {
        assert!(extract_payload(b"not a profile").is_err());
        assert!(extract_payload(&[]).is_err());
    }
}
