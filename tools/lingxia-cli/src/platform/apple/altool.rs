//! `xcrun altool` with an Apple ID and an App-specific password — Apple's
//! public upload path for an Apple ID. The password reaches altool through
//! its `@env:` form on the child's environment, never on the command line.

use super::auth::AppPasswordCredentials;
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::ffi::OsStr;
use std::process::{Command, Output};

const PASSWORD_ENV: &str = "LINGXIA_ALTOOL_APP_PASSWORD";

/// `xcrun altool <args> -u <Apple ID> -p @env:… --output-format json`.
pub fn run<I, S>(creds: &AppPasswordCredentials, args: I) -> Result<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    if !cfg!(target_os = "macos") {
        bail!("`xcrun altool` requires Xcode on macOS.");
    }
    Command::new("xcrun")
        .arg("altool")
        .args(args)
        .args(["-u", &creds.apple_id, "-p", &format!("@env:{PASSWORD_ENV}")])
        .args(["--output-format", "json"])
        .env(PASSWORD_ENV, &creds.password)
        .output()
        .context("run xcrun altool")
}

/// altool's JSON response; xcrun may print a launcher banner before it.
pub fn json(stdout: &[u8]) -> Result<Value> {
    let start = stdout
        .iter()
        .position(|byte| *byte == b'{')
        .context("altool returned no JSON response")?;
    serde_json::from_slice(&stdout[start..]).context("parse altool JSON response")
}

/// The messages of altool's `product-errors`, joined; `None` when there are none.
pub fn errors(response: &Value) -> Option<String> {
    let errors = response.get("product-errors")?.as_array()?;
    if errors.is_empty() {
        return None;
    }
    Some(
        errors
            .iter()
            .map(|error| {
                error
                    .pointer("/user-info/NSLocalizedFailureReason")
                    .or_else(|| error.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("unknown altool error")
            })
            .collect::<Vec<_>>()
            .join("; "),
    )
}

/// A team the Apple ID can upload for.
#[derive(Debug, Clone, PartialEq)]
pub struct Provider {
    pub team_id: String,
    pub name: Option<String>,
}

/// The teams this Apple ID can upload for. Fails when altool rejects the
/// Apple ID or password.
pub fn list_providers(creds: &AppPasswordCredentials) -> Result<Vec<Provider>> {
    let output = run(creds, ["--list-providers"])?;
    let response = json(&output.stdout).with_context(|| {
        format!(
            "altool --list-providers failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )
    })?;
    if let Some(message) = errors(&response) {
        bail!("altool rejected the Apple ID or App-specific password: {message}");
    }
    if !output.status.success() {
        bail!("altool --list-providers failed with a nonzero exit status");
    }
    let found = providers(&response);
    if found.is_empty() {
        bail!(
            "altool --list-providers named no team. Its response:\n{}",
            serde_json::to_string_pretty(&response).unwrap_or_default()
        );
    }
    Ok(found)
}

/// Every object carrying a developer team id, wherever altool nests it:
/// `developerTeamId` (App Store Connect's provider resources), `teamID`, or
/// `WWDRTeamID` (earlier altool). An actor's `teamId` is a provider UUID, not
/// a team id.
fn providers(value: &Value) -> Vec<Provider> {
    let mut found = Vec::new();
    collect_providers(value, &mut found);
    found
}

fn collect_providers(value: &Value, found: &mut Vec<Provider>) {
    match value {
        Value::Object(map) => {
            let field = |keys: &[&str]| {
                keys.iter()
                    .find_map(|key| map.get(*key).and_then(Value::as_str))
                    .map(str::to_string)
            };
            if let Some(team_id) = field(&["developerTeamId", "teamID", "WWDRTeamID"]) {
                found.push(Provider {
                    team_id,
                    name: field(&["name", "ProviderName"]),
                });
            }
            map.values().for_each(|v| collect_providers(v, found));
        }
        Value::Array(items) => items.iter().for_each(|v| collect_providers(v, found)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_skips_the_xcrun_banner() {
        let out = b"Running altool at path '/x/altool'...\n{\"success-message\":\"ok\"}";
        assert_eq!(json(out).unwrap()["success-message"], "ok");
    }

    #[test]
    fn errors_join_failure_reasons() {
        let response = serde_json::json!({
            "product-errors": [
                {"message": "a", "user-info": {"NSLocalizedFailureReason": "bad password"}},
                {"message": "second"}
            ]
        });
        assert_eq!(errors(&response).unwrap(), "bad password; second");
        assert!(errors(&serde_json::json!({"product-errors": []})).is_none());
    }

    #[test]
    fn providers_read_from_app_store_connect_resources() {
        let response = serde_json::json!({
            "included": [{
                "id": "provider-uuid",
                "type": "providers",
                "attributes": {"shortName": "TEAMAAAAAA", "name": "Example LLC", "developerTeamId": "TEAMAAAAAA"}
            }],
            "data": [{
                "type": "actors",
                "attributes": {"roles": ["APP_MANAGER"], "teamId": "provider-uuid"}
            }]
        });
        assert_eq!(
            providers(&response),
            vec![Provider {
                team_id: "TEAMAAAAAA".into(),
                name: Some("Example LLC".into())
            }]
        );
    }

    #[test]
    fn providers_read_from_xcode_26_keys() {
        let response = serde_json::json!({
            "providers": [
                {"name": "Example LLC", "shortName": "ExampleLLC", "publicID": "p-1", "teamID": "TEAMAAAAAA"}
            ]
        });
        assert_eq!(
            providers(&response),
            vec![Provider {
                team_id: "TEAMAAAAAA".into(),
                name: Some("Example LLC".into())
            }]
        );
    }

    #[test]
    fn providers_found_wherever_nested() {
        let response = serde_json::json!({
            "providers": [
                {"ProviderName": "Example LLC", "PublicID": "p-1", "WWDRTeamID": "TEAMAAAAAA"},
                {"PublicID": "p-2", "WWDRTeamID": "TEAMBBBBBB"}
            ]
        });
        assert_eq!(
            providers(&response),
            vec![
                Provider {
                    team_id: "TEAMAAAAAA".into(),
                    name: Some("Example LLC".into())
                },
                Provider {
                    team_id: "TEAMBBBBBB".into(),
                    name: None
                },
            ]
        );
    }
}
