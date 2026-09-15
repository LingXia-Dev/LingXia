//! CI output and processing orchestration for Apple and Harmony.

use super::backend::{StorePlatform, SubmitOptions, find_artifact};
use super::processing::{self, BuildSelection, ProcessingError, ProcessingOptions, Record, State};
use super::{
    appgallery, appstore, artifact_identity, asc_material, expected_identity, load_config,
};
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Serialize)]
struct Report {
    schema_version: u32,
    action: &'static str,
    platform: String,
    ok: bool,
    uploaded: bool,
    artifact: Option<PathBuf>,
    results: Vec<Record>,
    #[serde(skip_serializing_if = "Option::is_none")]
    app_status: Option<serde_json::Value>,
    error: Option<ReportError>,
}

#[derive(Serialize)]
struct ReportError {
    code: &'static str,
    message: String,
}

pub fn handles(
    platform: &str,
    options: &ProcessingOptions,
    build: &BuildSelection,
    id: Option<&str>,
) -> bool {
    matches!(
        StorePlatform::parse(platform),
        Ok(StorePlatform::Ios | StorePlatform::Macos | StorePlatform::Harmony)
    ) || options.json
        || options.wait
        || build.version.is_some()
        || build.build_number.is_some()
        || id.is_some()
}

pub fn run(
    platform: &str,
    submit: Option<SubmitOptions>,
    id: Option<String>,
    options: ProcessingOptions,
    build: BuildSelection,
) -> Result<()> {
    let mut report = Report {
        schema_version: 1,
        action: if submit.is_some() { "submit" } else { "status" },
        platform: platform.to_ascii_lowercase(),
        ok: false,
        uploaded: false,
        artifact: None,
        results: Vec::new(),
        app_status: None,
        error: None,
    };
    let result = execute(&mut report, submit, id, &options, build);
    report.ok = result.is_ok();
    if let Err(err) = &result {
        report.error = Some(ReportError {
            code: err
                .downcast_ref::<ProcessingError>()
                .map(|e| e.code)
                .unwrap_or("STORE_ERROR"),
            message: format!("{err:#}"),
        });
    }
    if options.json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        for record in &report.results {
            println!(
                "{}: {:?} (id: {}, version: {}, build: {}, store state: {})",
                record.app_id,
                record.state,
                record.submission_id.as_deref().unwrap_or("pending"),
                record.version.as_deref().unwrap_or("-"),
                record.build_number.as_deref().unwrap_or("-"),
                record.raw_state.as_deref().unwrap_or("-")
            );
        }
        if let Some(app_status) = &report.app_status {
            println!("{app_status}");
        }
        if result.is_ok() && report.uploaded {
            println!(
                "Uploaded{}; submit for review in the store console.",
                if options.wait { " and processed" } else { "" }
            );
        } else if result.is_ok() && report.results.is_empty() && report.app_status.is_none() {
            println!("No builds found.");
        }
    }
    result
}

fn execute(
    report: &mut Report,
    submit: Option<SubmitOptions>,
    id: Option<String>,
    options: &ProcessingOptions,
    build: BuildSelection,
) -> Result<()> {
    let targeted = submit.is_some() || id.is_some() || build.version.is_some();
    let platform = StorePlatform::parse(&report.platform)?;
    if !matches!(
        platform,
        StorePlatform::Ios | StorePlatform::Macos | StorePlatform::Harmony
    ) {
        return Err(processing::failure(
            "STORE_PROCESSING_UNSUPPORTED",
            "JSON results and processing selection/waiting are supported for ios, macos, and harmony",
        ));
    }
    processing::validate_selection(&build)?;
    if id.as_deref().is_some_and(|id| id.trim().is_empty()) {
        bail!("submission id must not be empty");
    }
    if platform == StorePlatform::Harmony && build.version.is_some() {
        bail!("Harmony processing is selected by --submission-id, not Apple version/build number");
    }
    if submit.is_none() && options.wait && id.is_none() && build.version.is_none() {
        bail!("Waiting requires --submission-id, or Apple --version and --build-number");
    }
    let config = load_config()?;
    let artifact = if submit.is_some() {
        let artifact = find_artifact(&std::env::current_dir()?, platform)?;
        report.artifact = Some(artifact.clone());
        artifact_identity::verify(&artifact, &expected_identity(&config, platform)?)?;
        Some(artifact)
    } else {
        None
    };

    match platform {
        StorePlatform::Ios | StorePlatform::Macos => {
            let build = if let Some(artifact) = &artifact {
                appstore::upload_selection(artifact, &build)?
            } else {
                build
            };
            if submit.is_some() && options.wait && build.version.is_none() {
                bail!(
                    "Waiting for a PKG requires --version and --build-number matching the uploaded package"
                );
            }
            let creds = if options.json {
                let channel = if platform == StorePlatform::Ios {
                    crate::resolver::AppleChannel::Ios
                } else {
                    crate::resolver::AppleChannel::Macos
                };
                crate::resolver::try_resolve_apple_asc(Some(channel))?.context(
                    "Apple API credentials missing; configure credentials before JSON automation",
                )?
            } else {
                asc_material(platform)?
            };
            let app_id = appstore::resolve_app_id(&creds, &expected_identity(&config, platform)?)?;
            if let (Some(opts), Some(artifact)) = (submit, artifact) {
                appstore::submit(&creds, platform, &artifact, &opts)?;
                report.uploaded = true;
                let mut record = Record::new(&app_id, State::Uploaded);
                record.version = build.version.clone();
                record.build_number = build.build_number.clone();
                report.results.push(record);
                if options.wait {
                    processing::wait(&mut report.results[0], options, |budget| {
                        appstore::query_one(&creds, &app_id, platform, &build, None, budget)
                    })?;
                }
            } else if options.wait {
                let mut record = Record::new(&app_id, State::Pending);
                record.submission_id = id.clone();
                record.version = build.version.clone();
                record.build_number = build.build_number.clone();
                report.results.push(record);
                processing::wait(&mut report.results[0], options, |budget| {
                    appstore::query_one(&creds, &app_id, platform, &build, id.as_deref(), budget)
                })?;
            } else if id.is_some() || build.version.is_some() {
                report.results.push(appstore::query_one(
                    &creds,
                    &app_id,
                    platform,
                    &build,
                    id.as_deref(),
                    Duration::from_secs(180),
                )?);
            } else {
                report.results = appstore::query_builds(
                    &creds,
                    &app_id,
                    platform,
                    &build,
                    None,
                    Duration::from_secs(180),
                )?;
            }
        }
        StorePlatform::Harmony => {
            let cfg = config
                .harmony
                .as_ref()
                .and_then(|h| h.store.as_ref())
                .context("missing `harmony.store` (appId) in lingxia.yaml")?;
            let creds = crate::resolver::resolve_harmony_agc(!options.json)?.credentials;
            let id = if let (Some(opts), Some(artifact)) = (submit, artifact) {
                let record = appgallery::submit(&creds, cfg, &artifact, &opts)?;
                report.uploaded = true;
                let id = record.submission_id.clone();
                report.results.push(record);
                id
            } else {
                id
            };
            if options.wait {
                let id = id.context(
                    "Upload completed but AGC returned no packageId; cannot wait for this package",
                )?;
                if report.results.is_empty() {
                    let mut record = Record::new(&cfg.app_id, State::Pending);
                    record.submission_id = Some(id.clone());
                    report.results.push(record);
                }
                let mut query = appgallery::PackageQuery::new(&creds, cfg, &id);
                processing::wait(&mut report.results[0], options, |budget| {
                    query.query(budget)
                })?;
            } else if !report.uploaded {
                if let Some(id) = id {
                    report.results.push(
                        appgallery::PackageQuery::new(&creds, cfg, &id)
                            .query(Duration::from_secs(180))?,
                    );
                } else {
                    report.app_status = Some(appgallery::query_app(&creds, cfg)?);
                }
            }
        }
        _ => unreachable!(),
    }
    check_target_records(targeted, &report.results)
}

fn check_target_records(targeted: bool, records: &[Record]) -> Result<()> {
    if !targeted {
        return Ok(());
    }
    if records.iter().any(|r| r.state == State::Failed) {
        return Err(processing::failure(
            "STORE_PROCESSING_FAILED",
            "Store reported failed processing",
        ));
    }
    if records.iter().any(|r| r.state == State::Unknown) {
        return Err(processing::failure(
            "STORE_PROCESSING_UNKNOWN",
            "Store returned an unrecognized processing state",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historical_failures_do_not_fail_a_successful_list_query() {
        let records = [
            Record::new("app", State::Complete),
            Record::new("app", State::Failed),
        ];
        assert!(check_target_records(false, &records).is_ok());
        assert!(check_target_records(true, &records[1..]).is_err());
        assert!(check_target_records(true, &[Record::new("app", State::Unknown)]).is_err());
    }
}
