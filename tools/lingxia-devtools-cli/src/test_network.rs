//! Network pieces of `lxdev test`: `--record-network <dir>` and the Logic
//! `fetch` calls a failed spec carries in its report.

use anyhow::{Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// The attachment `@lingxia/test` writes for each spec under
/// `--record-network`: `attachments/<spec id>[/repeat-k]/attempt-n/<name>`.
pub const RECORDED_SCENARIO: &str = "network.scenario.json";

/// Copy each spec's recorded scenario into `dir` as `<spec id>.json`
/// (`<spec id>-repeat-<k>.json` under `--repeat-each`). A retried spec keeps
/// its last attempt. Two specs whose ids map to the same file name get a
/// numeric suffix (`<stem>-2.json`), so one never overwrites another.
/// Returns the files written, in spec order.
pub fn save_recorded_scenarios(
    dir: &Path,
    artifacts: &[(String, PathBuf, usize)],
) -> Result<Vec<PathBuf>> {
    let mut names: std::collections::HashMap<(String, Option<String>), String> =
        std::collections::HashMap::new();
    let mut written: Vec<PathBuf> = Vec::new();
    for (name, path, _) in artifacts {
        let Some(key) = scenario_key(name) else {
            continue;
        };
        let file = match names.get(&key) {
            // Another attempt of the same spec: the later one wins.
            Some(file) => file.clone(),
            None => {
                let stem = scenario_stem(&key.0, key.1.as_deref());
                let taken = |file: &String| names.values().any(|used| used == file);
                let mut file = format!("{stem}.json");
                let mut n = 2;
                while taken(&file) {
                    file = format!("{stem}-{n}.json");
                    n += 1;
                }
                names.insert(key, file.clone());
                file
            }
        };
        let target = dir.join(file);
        std::fs::copy(path, &target).with_context(|| {
            format!(
                "cannot copy the recorded scenario {} to {}",
                path.display(),
                target.display()
            )
        })?;
        if !written.contains(&target) {
            written.push(target);
        }
    }
    Ok(written)
}

/// `(spec id, repeat)` of a recorded-scenario attachment name.
fn scenario_key(artifact: &str) -> Option<(String, Option<String>)> {
    let rest = artifact.strip_prefix("attachments/")?;
    let rest = rest.strip_suffix(RECORDED_SCENARIO)?.strip_suffix('/')?;
    let mut segments = rest.split('/');
    // Attachment names are `encodeURIComponent` output: `+` is literal.
    let id = lingxia_control_protocol::text::percent_decode(segments.next()?, false);
    let repeat = segments
        .find_map(|segment| segment.strip_prefix("repeat-"))
        .map(str::to_string);
    Some((id, repeat))
}

/// A file stem for a spec id: unsafe characters become `-`.
fn scenario_stem(id: &str, repeat: Option<&str>) -> String {
    let mut stem: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect();
    if stem.is_empty() || stem.chars().all(|c| c == '.') {
        stem = "spec".into();
    }
    if let Some(repeat) = repeat {
        stem.push_str(&format!("-repeat-{repeat}"));
    }
    stem
}

/// `<spec id>.json` for a recorded-scenario attachment name.
#[cfg(test)]
fn scenario_file_name(artifact: &str) -> Option<String> {
    let (id, repeat) = scenario_key(artifact)?;
    Some(format!("{}.json", scenario_stem(&id, repeat.as_deref())))
}

/// One line per Logic `fetch` call of a failure, for plain-text reports.
pub fn network_lines(calls: &Value) -> Vec<String> {
    calls
        .as_array()
        .into_iter()
        .flatten()
        .map(|call| {
            let outcome = match (call["status"].as_u64(), call["error"].as_str()) {
                (Some(status), _) => status.to_string(),
                (None, Some(error)) => error.to_string(),
                (None, None) => "pending".to_string(),
            };
            let duration = call["durationMs"]
                .as_u64()
                .map(|ms| format!(" {ms}ms"))
                .unwrap_or_default();
            let source = match call["source"].as_str() {
                Some("route") => " [route]",
                _ => "",
            };
            format!(
                "{} {} → {outcome}{duration}{source}",
                call["method"].as_str().unwrap_or("GET"),
                call["url"].as_str().unwrap_or(""),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn recorded_scenarios_are_named_after_their_spec() {
        assert_eq!(
            scenario_file_name("attachments/AUT-NET-001/attempt-0/network.scenario.json")
                .as_deref(),
            Some("AUT-NET-001.json")
        );
        assert_eq!(
            scenario_file_name(
                "attachments/pages%2Fnotes%20save/repeat-2/attempt-1/network.scenario.json"
            )
            .as_deref(),
            Some("pages-notes-save-repeat-2.json")
        );
        assert_eq!(
            scenario_file_name("attachments/x/attempt-0/failure.png"),
            None
        );
        assert_eq!(scenario_file_name("report.json"), None);
    }

    #[test]
    fn the_last_attempt_of_a_spec_wins() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("a.json");
        let second = dir.path().join("b.json");
        std::fs::write(&first, "{\"attempt\":0}").unwrap();
        std::fs::write(&second, "{\"attempt\":1}").unwrap();
        let out = dir.path().join("out");
        std::fs::create_dir_all(&out).unwrap();
        let written = save_recorded_scenarios(
            &out,
            &[
                (
                    "attachments/s1/attempt-0/network.scenario.json".into(),
                    first,
                    1,
                ),
                (
                    "attachments/s1/attempt-1/network.scenario.json".into(),
                    second,
                    1,
                ),
            ],
        )
        .unwrap();
        assert_eq!(written, vec![out.join("s1.json")]);
        assert_eq!(
            std::fs::read_to_string(out.join("s1.json")).unwrap(),
            "{\"attempt\":1}"
        );
    }

    #[test]
    fn specs_whose_names_collide_get_a_suffix() {
        let dir = tempfile::tempdir().unwrap();
        let mut artifacts = Vec::new();
        for (i, id) in ["a%2Fb", "a-b", "a%20b", "a%2Fb"].iter().enumerate() {
            let source = dir.path().join(format!("src-{i}.json"));
            std::fs::write(&source, format!("{{\"n\":{i}}}")).unwrap();
            artifacts.push((
                format!("attachments/{id}/attempt-{i}/network.scenario.json"),
                source,
                1,
            ));
        }
        let out = dir.path().join("out");
        std::fs::create_dir_all(&out).unwrap();
        let written = save_recorded_scenarios(&out, &artifacts).unwrap();
        assert_eq!(
            written,
            vec![
                out.join("a-b.json"),
                out.join("a-b-2.json"),
                out.join("a-b-3.json")
            ]
        );
        // `a/b` retried: its last attempt, in its own file.
        assert_eq!(
            std::fs::read_to_string(out.join("a-b.json")).unwrap(),
            "{\"n\":3}"
        );
        assert_eq!(
            std::fs::read_to_string(out.join("a-b-2.json")).unwrap(),
            "{\"n\":1}"
        );
        assert_eq!(
            std::fs::read_to_string(out.join("a-b-3.json")).unwrap(),
            "{\"n\":2}"
        );
    }

    #[test]
    fn network_calls_read_as_one_line_each() {
        let lines = network_lines(&json!([
            { "method": "GET", "url": "https://h/a", "status": 200, "durationMs": 12, "source": "network" },
            { "method": "POST", "url": "https://h/b", "status": null, "error": "TypeError: fetch failed", "source": "route" }
        ]));
        assert_eq!(
            lines,
            vec![
                "GET https://h/a → 200 12ms".to_string(),
                "POST https://h/b → TypeError: fetch failed [route]".to_string(),
            ]
        );
    }
}
