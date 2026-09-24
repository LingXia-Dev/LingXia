//! Isolated data profiles for `session.test` runs, and the
//! `session.profile.*` snapshot transfer.
//!
//! Isolation is host-side only: the target lxapp is closed, its data roots
//! are pointed at a fresh profile (optionally seeded from an uploaded
//! snapshot), and it is reopened. Nothing is written into the app. The run
//! that owns the profile returns the app to its own data when it ends.

use crate::util::run_async;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use lingxia_automation::runtime::{AutomationProfile, discard_retained, export_retained};
use lingxia_control_protocol::dev_session::session_test::*;
use lingxia_control_protocol::methods;
use lxapp::data_profile::{self, RunProfile};
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A staged snapshot nobody started a run with is deleted after this long.
const STAGED_TTL: Duration = Duration::from_secs(600);
const STAGED_DIR: &str = "staged";
const PART_EXT: &str = "part";
const STATE_EXT: &str = "lxstate";

pub(crate) fn capabilities() -> TestCapabilities {
    TestCapabilities {
        profile: Some(ProfileCapability {
            max_state_bytes: data_profile::MAX_PROFILE_BYTES,
            chunk_bytes: PROFILE_CHUNK_BYTES as u64,
        }),
    }
}

/// Whether `args` asks for isolation at all. A seed or a retained profile
/// implies it.
pub(crate) fn wants_isolation(args: Option<&TestProfileArgs>) -> bool {
    args.is_some_and(|args| args.isolate || args.seed_state_id.is_some() || args.retain)
}

/// Move the target lxapp onto a fresh profile before its run starts.
pub(crate) fn enter(args: &TestProfileArgs) -> Result<AutomationProfile, String> {
    let appid = target_appid(args.appid.as_deref())?;
    let base = data_profile::host_profiles_base().map_err(|err| err.to_string())?;
    let profile = RunProfile::create(&base).map_err(|err| err.to_string())?;
    if let Some(state_id) = &args.seed_state_id {
        let staged = staged_file(&base, state_id, STATE_EXT)?;
        let seeded = fs::read(&staged)
            .map_err(|err| format!("staged snapshot {state_id} is not available: {err}"))
            .and_then(|bytes| {
                data_profile::unpack(&bytes, &profile.live(), &data_profile::manifest_for(&appid))
                    .map_err(|err| format!("(usage): {err}"))
            });
        let _ = fs::remove_file(&staged);
        if let Err(err) = seeded {
            let _ = profile.remove();
            return Err(err);
        }
    }
    if let Err(err) = run_async(data_profile::enter(&appid, &profile)) {
        // The override may already be set; put the app back on its own data.
        let _ = run_async(data_profile::leave(&appid));
        let _ = profile.remove();
        return Err(format!(
            "could not move {appid} onto an isolated profile: {err}"
        ));
    }
    Ok(AutomationProfile {
        appid,
        profile,
        retain: args.retain,
    })
}

/// Undo [`enter`] for a run that never started.
pub(crate) fn abandon(profile: AutomationProfile) {
    if let Err(err) = run_async(data_profile::leave(&profile.appid)) {
        log::error!("returning {} to its own data: {err}", profile.appid);
    }
    let _ = profile.profile.remove();
}

fn target_appid(explicit: Option<&str>) -> Result<String, String> {
    if let Some(appid) = explicit.map(str::trim).filter(|appid| !appid.is_empty()) {
        return Ok(appid.to_string());
    }
    if let Some(home) = lingxia_app_context::home_app_id() {
        return Ok(home.to_string());
    }
    let (current, _, _) = lxapp::get_current_lxapp();
    if current.is_empty() {
        Err("(unavailable): no lxapp to isolate; open one first".to_string())
    } else {
        Ok(current)
    }
}

/// `<base>/staged/<id>.<ext>`, with `id` restricted so it cannot name a
/// path outside the staging directory.
fn staged_file(base: &Path, id: &str, ext: &str) -> Result<PathBuf, String> {
    let valid = !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
    if !valid {
        return Err(format!("(usage): invalid snapshot id {id:?}"));
    }
    Ok(base.join(STAGED_DIR).join(format!("{id}.{ext}")))
}

fn expire_staged(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age > STAGED_TTL);
        if old {
            let _ = fs::remove_file(entry.path());
        }
    }
}

pub(crate) fn handle(handler: &str, args: Option<Value>) -> Result<Option<Value>, String> {
    let base = data_profile::host_profiles_base().map_err(|err| err.to_string())?;
    match handler {
        methods::session::profile::UPLOAD => respond(upload(&base, parse(handler, args)?)?),
        methods::session::profile::EXPORT => respond(export(parse(handler, args)?)?),
        methods::session::profile::DISCARD => respond(discard(&base, parse(handler, args)?)?),
        other => Err(format!("unknown session.profile handler: {other}")),
    }
}

fn upload(base: &Path, args: ProfileUploadArgs) -> Result<ProfileUploadResponse, String> {
    let dir = base.join(STAGED_DIR);
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    expire_staged(&dir);
    let part = staged_file(base, &args.upload_id, PART_EXT)?;
    let chunk = BASE64
        .decode(args.base64.as_bytes())
        .map_err(|err| format!("(usage): invalid base64: {err}"))?;
    if chunk.len() > PROFILE_CHUNK_BYTES {
        return Err(format!(
            "(usage): chunk is {} bytes; the limit is {PROFILE_CHUNK_BYTES}",
            chunk.len()
        ));
    }
    let staged = fs::metadata(&part).map(|meta| meta.len()).unwrap_or(0);
    if staged != args.offset {
        return Err(format!(
            "(usage): upload {} is at byte {staged}, not {}",
            args.upload_id, args.offset
        ));
    }
    let received = staged + chunk.len() as u64;
    if received > data_profile::MAX_PROFILE_BYTES {
        let _ = fs::remove_file(&part);
        return Err(format!(
            "(usage): snapshot exceeds {} bytes",
            data_profile::MAX_PROFILE_BYTES
        ));
    }
    let mut file = open_private_append(&part).map_err(|err| err.to_string())?;
    file.write_all(&chunk).map_err(|err| err.to_string())?;
    drop(file);
    if !args.done {
        return Ok(ProfileUploadResponse {
            upload_id: args.upload_id,
            received,
            state_id: None,
        });
    }
    let finished = finish_upload(&part, args.sha256.as_deref());
    if let Err(err) = finished {
        let _ = fs::remove_file(&part);
        return Err(err);
    }
    fs::rename(&part, staged_file(base, &args.upload_id, STATE_EXT)?)
        .map_err(|err| err.to_string())?;
    Ok(ProfileUploadResponse {
        state_id: Some(args.upload_id.clone()),
        upload_id: args.upload_id,
        received,
    })
}

/// Check a complete upload: its digest when given, and that it is a
/// snapshot at all. Whether it fits the target app is checked at start.
fn finish_upload(part: &Path, sha256: Option<&str>) -> Result<(), String> {
    let bytes = fs::read(part).map_err(|err| err.to_string())?;
    if let Some(expected) = sha256 {
        let actual = data_profile::sha256_hex(&bytes);
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(format!(
                "(usage): snapshot digest mismatch: expected {expected}, received {actual}"
            ));
        }
    }
    data_profile::read_manifest(&bytes).map_err(|err| format!("(usage): {err}"))?;
    Ok(())
}

fn open_private_append(path: &Path) -> std::io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

fn export(args: ProfileExportArgs) -> Result<ProfileExportResponse, String> {
    let packed = export_retained(&args.run_id).map_err(|err| format!("(not_found): {err}"))?;
    let total = packed.bytes.len();
    let start = usize::try_from(args.offset)
        .ok()
        .filter(|offset| *offset <= total)
        .ok_or_else(|| format!("(usage): offset {} is past the end ({total})", args.offset))?;
    let end = (start + PROFILE_CHUNK_BYTES).min(total);
    Ok(ProfileExportResponse {
        base64: BASE64.encode(&packed.bytes[start..end]),
        total: total as u64,
        done: end == total,
        sha256: packed.sha256,
    })
}

fn discard(base: &Path, args: ProfileDiscardArgs) -> Result<ProfileDiscardResponse, String> {
    match (args.run_id, args.state_id) {
        (Some(run_id), None) => Ok(ProfileDiscardResponse {
            discarded: discard_retained(&run_id),
        }),
        (None, Some(state_id)) => {
            let mut discarded = false;
            for ext in [PART_EXT, STATE_EXT] {
                discarded |= fs::remove_file(staged_file(base, &state_id, ext)?).is_ok();
            }
            Ok(ProfileDiscardResponse { discarded })
        }
        _ => Err("(usage): pass exactly one of run_id or state_id".to_string()),
    }
}

fn parse<T: serde::de::DeserializeOwned>(handler: &str, args: Option<Value>) -> Result<T, String> {
    let value = args.ok_or_else(|| format!("missing args for {handler}"))?;
    serde_json::from_value(value).map_err(|err| format!("invalid args for {handler}: {err}"))
}

fn respond<T: serde::Serialize>(response: T) -> Result<Option<Value>, String> {
    serde_json::to_value(response)
        .map(Some)
        .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lingxia-profile-upload-{}", uuid()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn uuid() -> String {
        format!(
            "{:x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    }

    fn snapshot(base: &Path) -> Vec<u8> {
        let profile = RunProfile::create(base).unwrap();
        fs::write(profile.live().join("storage.redb"), vec![7u8; 1000]).unwrap();
        let manifest = data_profile::manifest_for("app.lingxia.upload-test");
        let bytes = data_profile::pack(&profile.live(), &manifest).unwrap();
        profile.remove().unwrap();
        bytes
    }

    fn chunk(
        id: &str,
        offset: u64,
        bytes: &[u8],
        done: bool,
        sha: Option<String>,
    ) -> ProfileUploadArgs {
        ProfileUploadArgs {
            upload_id: id.to_string(),
            offset,
            base64: BASE64.encode(bytes),
            done,
            sha256: sha,
        }
    }

    #[test]
    fn chunked_upload_checks_offsets_and_digest() {
        let base = scratch();
        let bytes = snapshot(&base);
        let (head, tail) = bytes.split_at(bytes.len() / 2);
        let sha = data_profile::sha256_hex(&bytes);

        let first = upload(&base, chunk("up-1", 0, head, false, None)).unwrap();
        assert_eq!(first.received, head.len() as u64);
        assert!(first.state_id.is_none());
        let skipped = upload(&base, chunk("up-1", 0, tail, false, None)).unwrap_err();
        assert!(skipped.contains("is at byte"));
        let done = upload(
            &base,
            chunk("up-1", head.len() as u64, tail, true, Some(sha)),
        )
        .unwrap();
        assert_eq!(done.state_id.as_deref(), Some("up-1"));
        assert!(staged_file(&base, "up-1", STATE_EXT).unwrap().is_file());
        assert!(!staged_file(&base, "up-1", PART_EXT).unwrap().exists());

        let discarded = discard(
            &base,
            ProfileDiscardArgs {
                state_id: Some("up-1".into()),
                run_id: None,
            },
        )
        .unwrap();
        assert!(discarded.discarded);
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn upload_refuses_a_wrong_digest_or_a_non_snapshot() {
        let base = scratch();
        let bytes = snapshot(&base);
        let wrong =
            upload(&base, chunk("up-2", 0, &bytes, true, Some("00".repeat(32)))).unwrap_err();
        assert!(wrong.contains("digest mismatch"));
        assert!(!staged_file(&base, "up-2", PART_EXT).unwrap().exists());
        let junk = upload(&base, chunk("up-3", 0, b"not a snapshot", true, None)).unwrap_err();
        assert!(junk.contains("not a profile snapshot"));
        assert!(staged_file(&base, "../escape", STATE_EXT).is_err());
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn isolation_is_implied_by_a_seed_or_retain() {
        assert!(!wants_isolation(None));
        assert!(!wants_isolation(Some(&TestProfileArgs::default())));
        for args in [
            TestProfileArgs {
                isolate: true,
                ..Default::default()
            },
            TestProfileArgs {
                seed_state_id: Some("s".into()),
                ..Default::default()
            },
            TestProfileArgs {
                retain: true,
                ..Default::default()
            },
        ] {
            assert!(wants_isolation(Some(&args)));
        }
    }
}
