//! Certificates subcommand for Apple Developer Services.

use anyhow::Result;
use colored::Colorize;

use super::client::with_client;

/// Execute certificates command
pub fn execute() -> Result<()> {
    with_client(|client| {
        let certs = client.list_certificates()?;

        if certs.is_empty() {
            println!("{}", "No certificates found.".yellow());
            return Ok(());
        }

        println!("{}", "Certificates".cyan().bold());
        println!();

        for cert in certs {
            println!("- id: {}", cert.id.bold());

            if let Some(name) = &cert.name {
                println!("  name: {}", name);
            }

            if let Some(type_str) = &cert.type_string {
                println!("  type: {}", type_str);
            }

            if let Some(display_name) = &cert.display_name {
                println!("  display name: {}", display_name);
            }

            if let Some(serial) = &cert.serial_number {
                println!("  serial number: {}", serial);
            }

            if let Some(status) = &cert.status {
                let status_colored = match status.as_str() {
                    "Issued" => status.green(),
                    _ => status.yellow(),
                };
                println!("  status: {}", status_colored);
            }

            if let Some(date) = &cert.expiration_date {
                println!("  expiry: {}", date);
            }

            // Parse certificate content if available
            if let Some(content_b64) = &cert.certificate_content
                && let Ok(cert_data) =
                    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, content_b64)
            {
                println!("  content:");

                // Try to parse the certificate to extract common name and team ID
                if let Ok(parsed) = parse_cert_info(&cert_data) {
                    if let Some(cn) = parsed.common_name {
                        println!("    common name: {}", cn);
                    }
                    if let Some(team_id) = parsed.team_id {
                        println!("    team id: {}", team_id);
                    }
                }
            }
        }

        Ok(())
    })
}

/// Certificate parsed information
struct CertInfo {
    common_name: Option<String>,
    team_id: Option<String>,
}

/// Parse certificate to extract common name and team ID
fn parse_cert_info(cert_data: &[u8]) -> Result<CertInfo> {
    use x509_parser::prelude::*;

    let (_, cert) = X509Certificate::from_der(cert_data)
        .map_err(|e| anyhow::anyhow!("Failed to parse certificate: {:?}", e))?;

    // Extract common name from subject
    let common_name = cert
        .subject()
        .iter_common_name()
        .next()
        .and_then(|cn| cn.as_str().ok())
        .map(|s| s.to_string());

    // Extract team ID from OU (organizationalUnitName)
    let team_id = cert
        .subject()
        .iter_organizational_unit()
        .next()
        .and_then(|ou| ou.as_str().ok())
        .map(|s| s.to_string());

    Ok(CertInfo {
        common_name,
        team_id,
    })
}
