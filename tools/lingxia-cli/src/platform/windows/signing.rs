//! Authenticode signing for Windows payloads, installers and MSIX packages.
//!
//! Windows refuses to install an unsigned MSIX. For local / dev / internal use a
//! self-signed certificate is enough: we generate one **once** (subject == the
//! package Identity `Publisher`), persist the `.pfx` under `~/.lingxia/windows/`
//! and reuse it on every build, sign with `signtool`, and trust the public cert
//! so `lingxia install` works. Production signing uses an existing certificate
//! in the user certificate store, selected by thumbprint.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use colored::Colorize;

/// Fixed password for the throwaway dev self-signed `.pfx`. The cert is untrusted
/// by default and only guards a local dev key, so a constant is acceptable.
const SELF_SIGNED_PFX_PASSWORD: &str = "lingxia-self-signed";

/// How to sign the packaged MSIX.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsSigning {
    /// Leave the package unsigned (caller prints the manual `signtool` hint).
    None,
    /// Generate / reuse a persisted self-signed cert, then sign + trust it.
    SelfSigned,
}

/// Sign `msix_path` per `mode`. `publisher` is the MSIX Identity `Publisher`; a
/// self-signed cert's subject must equal it exactly or Windows rejects the package.
pub fn sign_msix(msix_path: &Path, publisher: &str, mode: WindowsSigning) -> Result<()> {
    match mode {
        WindowsSigning::None => Ok(()),
        WindowsSigning::SelfSigned => sign_self_signed(msix_path, publisher),
    }
}

fn sign_self_signed(msix_path: &Path, publisher: &str) -> Result<()> {
    let signtool = find_signtool()?;
    let (pfx, cer) = ensure_self_signed_cert(publisher)?;

    let status = Command::new(&signtool)
        .args(["sign", "/fd", "SHA256", "/f"])
        .arg(&pfx)
        .args(["/p", SELF_SIGNED_PFX_PASSWORD])
        .arg(msix_path)
        .status()
        .with_context(|| format!("Failed to run {}", signtool.display()))?;
    if !status.success() {
        bail!("signtool sign failed");
    }
    println!("  {} signed (self-signed: {})", "✓".green(), publisher);

    trust_certificate(&cer);
    Ok(())
}

/// Return `(pfx, cer)` for `publisher`, generating + persisting them once.
fn ensure_self_signed_cert(publisher: &str) -> Result<(PathBuf, PathBuf)> {
    let dir = crate::state_root::lingxia_dir()?.join("windows");
    std::fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    let stem = sanitize_file_stem(publisher);
    let pfx = dir.join(format!("{stem}.pfx"));
    let cer = dir.join(format!("{stem}.cer"));
    if pfx.is_file() && cer.is_file() {
        return Ok((pfx, cer));
    }

    println!(
        "  {} generating a self-signed code-signing cert for {}",
        "•".cyan(),
        publisher
    );
    generate_self_signed_cert(publisher, &pfx, &cer)?;
    Ok((pfx, cer))
}

/// Generate a self-signed code-signing cert via PowerShell, exporting the `.pfx`
/// (private) + `.cer` (public). The cert lives only transiently in the user store.
fn generate_self_signed_cert(publisher: &str, pfx: &Path, cer: &Path) -> Result<()> {
    let script = format!(
        "$ErrorActionPreference='Stop'; \
         $c = New-SelfSignedCertificate -Type CodeSigningCert -Subject '{subject}' \
         -CertStoreLocation Cert:\\CurrentUser\\My -KeyExportPolicy Exportable \
         -KeyUsage DigitalSignature -FriendlyName 'LingXia self-signed' \
         -NotAfter (Get-Date).AddYears(5); \
         $pw = ConvertTo-SecureString -String '{password}' -Force -AsPlainText; \
         Export-PfxCertificate -Cert $c -FilePath '{pfx}' -Password $pw | Out-Null; \
         Export-Certificate -Cert $c -FilePath '{cer}' | Out-Null; \
         Remove-Item ('Cert:\\CurrentUser\\My\\' + $c.Thumbprint) -Force;",
        subject = ps_single_quote(publisher),
        password = SELF_SIGNED_PFX_PASSWORD,
        pfx = ps_single_quote(&pfx.to_string_lossy()),
        cer = ps_single_quote(&cer.to_string_lossy()),
    );
    run_powershell(&script).context("Failed to generate self-signed certificate")
}

/// Best-effort: import the public cert so the MSIX installs. A self-signed cert
/// is its own root, so it must go into `LocalMachine\Root` or the chain won't
/// validate (App Installer reports `0x800B010A`); `TrustedPeople` additionally
/// covers `Add-AppxPackage`. Needs admin; on failure print the manual command.
fn trust_certificate(cer: &Path) {
    let cer_q = ps_single_quote(&cer.to_string_lossy());
    let script = format!(
        "$ErrorActionPreference='Stop'; \
         Import-Certificate -FilePath '{cer_q}' -CertStoreLocation Cert:\\LocalMachine\\Root | Out-Null; \
         Import-Certificate -FilePath '{cer_q}' -CertStoreLocation Cert:\\LocalMachine\\TrustedPeople | Out-Null;",
    );
    if run_powershell(&script).is_ok() {
        println!(
            "  {} trusted the cert (LocalMachine\\Root + TrustedPeople)",
            "✓".green()
        );
    } else {
        println!(
            "  {} couldn't auto-trust the cert (needs admin). To install, run as admin:\n     \
             Import-Certificate -FilePath \"{}\" -CertStoreLocation Cert:\\LocalMachine\\Root",
            "note:".yellow(),
            cer.display()
        );
    }
}

fn run_powershell(script: &str) -> Result<()> {
    let status = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .status()
        .context("Failed to run powershell")?;
    if !status.success() {
        bail!("powershell script exited with {status}");
    }
    Ok(())
}

/// Locate `signtool.exe` from the Windows SDK (newest version, x64), or an
/// explicit `LINGXIA_SIGNTOOL` override.
fn find_signtool() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("LINGXIA_SIGNTOOL").map(PathBuf::from)
        && path.is_file()
    {
        return Ok(path);
    }
    let bin = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(bin) {
        for entry in entries.flatten() {
            let exe = entry.path().join("x64").join("signtool.exe");
            if exe.is_file() {
                candidates.push(exe);
            }
        }
    }
    candidates.sort();
    candidates.pop().ok_or_else(|| {
        anyhow!(
            "signtool.exe not found. Install the Windows 10/11 SDK (it ships \
             makeappx/signtool), or set LINGXIA_SIGNTOOL to its path."
        )
    })
}

/// A filesystem-safe stem derived from the publisher DN, e.g. `CN=My App` →
/// `CN_My_App`, so each publisher gets its own persisted cert.
fn sanitize_file_stem(publisher: &str) -> String {
    let mapped: String = publisher
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let trimmed = mapped.trim_matches('_');
    if trimmed.is_empty() {
        "publisher".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Escape a value for a PowerShell single-quoted string (double any quotes).
fn ps_single_quote(value: &str) -> String {
    value.replace('\'', "''")
}

/// Production signing uses a certificate already provisioned in the Windows
/// user certificate store (including hardware-backed keys). No PFX password is
/// persisted in configuration, scripts, or build output.
fn release_certificate() -> Result<Option<String>> {
    let cert = std::env::var("LINGXIA_WINDOWS_CERT_SHA1")
        .ok()
        .filter(|s| !s.is_empty());
    if let Some(cert) = &cert
        && (cert.len() != 40 || !cert.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        bail!("LINGXIA_WINDOWS_CERT_SHA1 must be a 40-digit certificate thumbprint");
    }
    if cert.is_none() && std::env::var("LINGXIA_WINDOWS_REQUIRE_SIGNING").as_deref() == Ok("1") {
        bail!("Windows signing is required; set LINGXIA_WINDOWS_CERT_SHA1");
    }
    Ok(cert)
}

fn timestamp_url() -> Result<String> {
    let value = std::env::var("LINGXIA_WINDOWS_TIMESTAMP_URL")
        .unwrap_or_else(|_| "http://timestamp.digicert.com".into());
    // Also embedded in NSIS's compile-time command, where shell metacharacters
    // and percent expansion must not be accepted.
    let parsed = url::Url::parse(&value).context("Invalid Windows timestamp URL")?;
    if !matches!(parsed.scheme(), "http" | "https")
        || value
            .chars()
            .any(|c| c.is_whitespace() || "\"'&|<>^%$`".contains(c))
    {
        bail!("Invalid Windows timestamp URL");
    }
    Ok(value)
}

pub fn release_signing_enabled() -> Result<bool> {
    Ok(release_certificate()?.is_some())
}

pub fn preflight_release_signing() -> Result<()> {
    if release_certificate()?.is_some() {
        find_signtool()?;
        timestamp_url()?;
    }
    Ok(())
}

pub fn sign_release_artifact(path: &Path) -> Result<()> {
    let Some(cert) = release_certificate()? else {
        return Ok(());
    };
    let tool = find_signtool()?;
    let status = Command::new(&tool)
        .args([
            "sign",
            "/sha1",
            &cert,
            "/fd",
            "SHA256",
            "/tr",
            &timestamp_url()?,
            "/td",
            "SHA256",
        ])
        .arg(path)
        .status()?;
    if !status.success() {
        bail!("Windows signing failed for {}", path.display());
    }
    let status = Command::new(tool)
        .args(["verify", "/pa"])
        .arg(path)
        .status()?;
    if !status.success() {
        bail!(
            "Windows signature verification failed for {}",
            path.display()
        );
    }
    Ok(())
}

pub fn nsis_uninstaller_signing() -> Result<String> {
    let Some(cert) = release_certificate()? else {
        return Ok(String::new());
    };
    let tool = find_signtool()?.to_string_lossy().into_owned();
    if tool
        .chars()
        .any(|c| c.is_control() || "\"'&|<>^%$`".contains(c))
    {
        bail!("signtool path cannot contain shell metacharacters");
    }
    Ok(format!(
        "!uninstfinalize '\"{tool}\" sign /sha1 {cert} /fd SHA256 /tr {} /td SHA256 \"%1\"' = 0\n",
        timestamp_url()?
    ))
}
