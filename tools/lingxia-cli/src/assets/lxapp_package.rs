use anyhow::{Context, Result, anyhow};
use flate2::read::GzDecoder;
use semver::Version;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tar::Archive;

#[derive(Deserialize)]
struct NpmPackResult {
    filename: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum NpmViewVersions {
    One(String),
    Many(Vec<String>),
}

pub(super) fn resolve_lxapp_package(
    project_root: &Path,
    package: &str,
    version: &str,
    cache_kind: &str,
    config_key: &str,
) -> Result<PathBuf> {
    let package = package.trim();
    let version = version.trim();
    if package.is_empty() {
        return Err(anyhow!("{config_key}.package must not be empty"));
    }
    if version.is_empty() {
        return Err(anyhow!("{config_key}.version must not be empty"));
    }

    let version = resolve_npm_version(package, version, config_key)?;

    let package_dir = project_root
        .join(".lingxia")
        .join(cache_kind)
        .join(sanitize_package_name(package))
        .join(&version)
        .join("package");
    if package_dir.join("lxapp.json").exists() {
        return Ok(package_dir);
    }

    let cache_dir = package_dir
        .parent()
        .ok_or_else(|| anyhow!("Invalid {config_key} cache path: {}", package_dir.display()))?;
    fs::create_dir_all(cache_dir)?;

    let temp_dir = tempfile::Builder::new()
        .prefix(&format!("{cache_kind}-"))
        .tempdir_in(cache_dir)
        .with_context(|| format!("Failed to create temp dir in {}", cache_dir.display()))?;
    let spec = format!("{package}@{version}");
    let output = Command::new(crate::npm::command())
        .arg("pack")
        .arg("--json")
        .arg(&spec)
        .arg("--pack-destination")
        .arg(temp_dir.path())
        .current_dir(project_root)
        .output()
        .with_context(|| format!("Failed to run npm pack {spec}"))?;
    if !output.status.success() {
        return Err(anyhow!(
            "npm pack {spec} failed with status {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let packed: Vec<NpmPackResult> =
        serde_json::from_slice(&output.stdout).context("Failed to parse npm pack --json output")?;
    let tarball = packed
        .first()
        .map(|info| temp_dir.path().join(&info.filename))
        .ok_or_else(|| anyhow!("npm pack {spec} returned no tarball"))?;
    if !tarball.is_file() {
        return Err(anyhow!(
            "npm pack {spec} did not create expected tarball: {}",
            tarball.display()
        ));
    }

    let extract_dir = temp_dir.path().join("extract");
    fs::create_dir_all(&extract_dir)?;
    let tar_gz = fs::File::open(&tarball)
        .with_context(|| format!("Failed to open {}", tarball.display()))?;
    Archive::new(GzDecoder::new(tar_gz))
        .unpack(&extract_dir)
        .with_context(|| format!("Failed to unpack {}", tarball.display()))?;

    let unpacked_package = extract_dir.join("package");
    if !unpacked_package.join("lxapp.json").exists() {
        return Err(anyhow!(
            "lxapp package {spec} must contain lxapp.json at package root"
        ));
    }
    if !unpacked_package.join("dist").is_dir() {
        return Err(anyhow!("lxapp package {spec} must contain prebuilt dist/"));
    }

    if package_dir.exists() {
        fs::remove_dir_all(&package_dir)
            .with_context(|| format!("Failed to remove {}", package_dir.display()))?;
    }
    fs::rename(&unpacked_package, &package_dir).with_context(|| {
        format!(
            "Failed to move {config_key} package into cache: {}",
            package_dir.display()
        )
    })?;

    Ok(package_dir)
}

/// Exact versions stay as written. Ranges (`~0.11.0`) resolve to the latest
/// published match so a published patch is picked up without a CLI rebuild.
fn resolve_npm_version(package: &str, spec: &str, config_key: &str) -> Result<String> {
    if Version::parse(spec).is_ok() {
        return Ok(spec.to_string());
    }

    let package_spec = format!("{package}@{spec}");
    let output = Command::new(crate::npm::command())
        .args(["view", &package_spec, "version", "--json"])
        .output()
        .with_context(|| format!("Failed to run npm view {package_spec}"))?;
    if !output.status.success() {
        return Err(anyhow!(
            "npm view {package_spec} failed with status {}\nstdout:\n{}\nstderr:\n{}\nSet {config_key}.version to pin an exact published version.",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    latest_npm_version(&output.stdout, &package_spec)
}

fn latest_npm_version(output: &[u8], package_spec: &str) -> Result<String> {
    let versions: NpmViewVersions = serde_json::from_slice(output)
        .with_context(|| format!("npm view {package_spec} returned invalid JSON"))?;
    let versions = match versions {
        NpmViewVersions::One(version) => vec![version],
        NpmViewVersions::Many(versions) => versions,
    };

    versions
        .into_iter()
        .map(|version| match Version::parse(&version) {
            Ok(parsed) => Ok((parsed, version)),
            Err(err) => Err(err).with_context(|| {
                format!("npm view {package_spec} returned a non-semver version: {version}")
            }),
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .max_by(|(left, _), (right, _)| left.cmp(right))
        .map(|(_, version)| version)
        .ok_or_else(|| anyhow!("npm view {package_spec} returned no matching versions"))
}

fn sanitize_package_name(package: &str) -> String {
    package
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::latest_npm_version;

    #[test]
    fn npm_view_single_version_is_accepted() {
        assert_eq!(
            latest_npm_version(br#""0.11.2""#, "@lingxia/example@latest").unwrap(),
            "0.11.2"
        );
    }

    #[test]
    fn npm_view_range_uses_highest_matching_version() {
        assert_eq!(
            latest_npm_version(
                br#"["0.11.0", "0.11.3", "0.11.2"]"#,
                "@lingxia/example@~0.11.0"
            )
            .unwrap(),
            "0.11.3"
        );
    }
}
