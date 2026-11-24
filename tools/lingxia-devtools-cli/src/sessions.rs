use crate::project;
use anyhow::Result;
use chrono::{DateTime, Local, TimeZone};
use serde_json::{Value, json};

/// `lxdev session [--json]`: every running session of this user, with its
/// state (starting, ready, stale), target, name, project, host build and log.
pub fn execute(json_output: bool) -> Result<()> {
    let sessions = project::list_all_sessions()?;
    let probes: Vec<project::SessionProbe> = sessions.iter().map(project::probe_session).collect();
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&to_json(&sessions, &probes))?
        );
    } else {
        print!("{}", render(&sessions, &probes));
    }
    Ok(())
}

fn to_json(sessions: &[project::SessionInfo], probes: &[project::SessionProbe]) -> Value {
    Value::Array(
        sessions
            .iter()
            .zip(probes)
            .enumerate()
            .map(|(index, (s, probe))| {
                json!({
                    "ordinal": index + 1,
                    "session_id": s.session_id,
                    "name": s.name,
                    "pid": s.pid,
                    "target": s.target,
                    "context_root": s.project_root,
                    "content": s.content,
                    "started_at": s.started_at,
                    "ws_url": s.ws_url,
                    "log_file": s.log_file,
                    "output": s.output,
                    "cli_build": s.build,
                    "cli_current": cli_current(s),
                    "host_build": probe.runtime_build,
                    "state": probe.state.as_str(),
                    "runtime_connected": probe.state == project::SessionState::Ready,
                    "stale": probe.state == project::SessionState::Stale,
                })
            })
            .collect(),
    )
}

/// The table, each session's log under its row.
fn render(sessions: &[project::SessionInfo], probes: &[project::SessionProbe]) -> String {
    if sessions.is_empty() {
        return "No running app session. Start one with `lingxia dev`.\n".to_string();
    }
    let rows: Vec<[String; 8]> = sessions
        .iter()
        .zip(probes)
        .enumerate()
        .map(|(index, (info, probe))| {
            [
                (index + 1).to_string(),
                info.session_id.clone(),
                info.name.clone().unwrap_or_else(|| "-".to_string()),
                info.target.clone(),
                probe.state.as_str().to_string(),
                host_label(probe),
                if info.started_at == 0 {
                    "-".to_string()
                } else {
                    format_started(info.started_at)
                },
                info.content
                    .as_ref()
                    .map(|content| content.display())
                    .unwrap_or(&info.project_root)
                    .to_string(),
            ]
        })
        .collect();
    let header = [
        "#", "ID", "NAME", "TARGET", "STATE", "HOST", "STARTED", "CONTENT",
    ];
    let mut widths = header.map(str::len);
    for row in &rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    let line = |cells: [&str; 8]| {
        let mut line = String::new();
        for (index, cell) in cells.iter().enumerate() {
            if index + 1 == cells.len() {
                line.push_str(cell);
            } else {
                line.push_str(&format!("{cell:<width$}  ", width = widths[index]));
            }
        }
        format!("{}\n", line.trim_end())
    };
    let mut out = line(header);
    for (row, info) in rows.iter().zip(sessions) {
        out.push_str(&line(row.each_ref().map(String::as_str)));
        if !info.log_file.is_empty() {
            out.push_str(&format!("  log: {}\n", info.log_file));
        }
        if let Some(output) = &info.output {
            out.push_str(&format!("  build: {output} (not dist/)\n"));
        }
        if !cli_current(info) {
            out.push_str(&format!(
                "  note: {} was rebuilt after this session started; restart it to pick up CLI changes\n",
                info.executable
            ));
        }
    }
    out.push_str(
        "\nSelect one with --session NAME|TARGET|TARGET@<dir>|#; name a session with \
         `lingxia dev --name NAME`.\n",
    );
    out
}

/// Whether the session's `lingxia dev` still runs the binary at its path: one rebuilt after the
/// session started serves the old CLI code until the session restarts.
fn cli_current(info: &project::SessionInfo) -> bool {
    let modified = std::fs::metadata(&info.executable)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok());
    match modified {
        Some(modified) if info.started_at > 0 => {
            modified.as_millis() <= u128::from(info.started_at)
        }
        _ => true,
    }
}

/// `0.19.0/p2 view 1/231`: the host runtime's LingXia version, dev protocol and View wire.
fn host_label(probe: &project::SessionProbe) -> String {
    match &probe.runtime_build {
        Some(build) => match build.view {
            Some(view) => format!("{}/p{} view {view}", build.version, build.protocol),
            None => format!("{}/p{}", build.version, build.protocol),
        },
        None => "-".to_string(),
    }
}

fn format_started(started_at: u64) -> String {
    let secs = (started_at / 1000) as i64;
    let nsecs = ((started_at % 1000) * 1_000_000) as u32;
    match Local.timestamp_opt(secs, nsecs).single() {
        Some(dt) => {
            let dt: DateTime<Local> = dt;
            dt.format("%Y-%m-%d %H:%M:%S").to_string()
        }
        None => started_at.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(id: &str, name: Option<&str>) -> project::SessionInfo {
        serde_json::from_value(json!({
            "session_id": id,
            "project_root": "/work/app",
            "content": { "kind": "host", "path": "/work/app" },
            "target": "macos",
            "pid": 7,
            "started_at": 0,
            "ws_url": "ws://127.0.0.1:1",
            "log_file": format!("/work/app/.lingxia/logs/{id}.jsonl"),
            "output": format!("/work/app/.lingxia/dev/sessions/{id}"),
            "name": name,
        }))
        .unwrap()
    }

    fn probe(state: project::SessionState) -> project::SessionProbe {
        project::SessionProbe {
            state,
            runtime_build: None,
        }
    }

    #[test]
    fn no_session_says_how_to_start_one() {
        assert_eq!(
            render(&[], &[]),
            "No running app session. Start one with `lingxia dev`.\n"
        );
        assert_eq!(to_json(&[], &[]), json!([]));
    }

    #[test]
    fn the_table_shows_state_target_name_project_and_log() {
        let sessions = [session("aaa111", Some("ci")), session("bbb222", None)];
        let probes = [
            probe(project::SessionState::Ready),
            probe(project::SessionState::Starting),
        ];
        let table = render(&sessions, &probes);
        let lines: Vec<&str> = table.lines().collect();
        assert!(
            lines[0].starts_with("#  ID      NAME  TARGET  STATE"),
            "{table}"
        );
        assert!(
            lines[1].starts_with("1  aaa111  ci    macos   ready"),
            "{table}"
        );
        assert!(lines[1].ends_with("/work/app"), "{table}");
        assert_eq!(lines[2], "  log: /work/app/.lingxia/logs/aaa111.jsonl");
        assert_eq!(
            lines[3],
            "  build: /work/app/.lingxia/dev/sessions/aaa111 (not dist/)"
        );
        assert!(
            lines[4].starts_with("2  bbb222  -     macos   starting"),
            "{table}"
        );
        assert_eq!(lines[5], "  log: /work/app/.lingxia/logs/bbb222.jsonl");
    }

    #[test]
    fn json_carries_what_scripts_need() {
        let value = to_json(
            &[session("aaa111", Some("ci"))],
            &[probe(project::SessionState::Ready)],
        );
        let entry = &value[0];
        for key in [
            "ordinal",
            "session_id",
            "name",
            "pid",
            "target",
            "context_root",
            "content",
            "started_at",
            "ws_url",
            "log_file",
            "output",
            "cli_build",
            "host_build",
            "state",
        ] {
            assert!(entry.get(key).is_some(), "{key} missing: {entry}");
        }
        assert_eq!(entry["state"], "ready");
        assert_eq!(entry["name"], "ci");
    }

    #[test]
    fn a_session_whose_cli_was_rebuilt_since_says_to_restart_it() {
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join("lingxia");
        std::fs::write(&binary, "").unwrap();
        let built = std::fs::metadata(&binary)
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let mut old = session("aaa111", None);
        old.executable = binary.display().to_string();
        old.started_at = built - 60_000;
        let mut current = session("bbb222", None);
        current.executable = binary.display().to_string();
        current.started_at = built + 1_000;
        let probes = [
            probe(project::SessionState::Ready),
            probe(project::SessionState::Ready),
        ];
        let sessions = [old, current];
        let table = render(&sessions, &probes);
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(
            lines[4],
            format!(
                "  note: {} was rebuilt after this session started; restart it to pick up CLI changes",
                binary.display()
            ),
            "{table}"
        );
        assert!(lines[5].starts_with("2  bbb222"), "{table}");
        let value = to_json(&sessions, &probes);
        assert_eq!(value[0]["cli_current"], false);
        assert_eq!(value[1]["cli_current"], true);
    }

    #[test]
    fn the_host_build_names_its_view_wire() {
        use lingxia_control_protocol::dev_session::{PeerBuild, ViewWire};
        let mut ready = probe(project::SessionState::Ready);
        ready.runtime_build = Some(PeerBuild {
            version: "0.30.0".into(),
            protocol: 2,
            view: Some(ViewWire {
                ui_abi: 1,
                max_field: 231,
            }),
        });
        assert_eq!(host_label(&ready), "0.30.0/p2 view 1/231");
        let value = to_json(&[session("aaa111", None)], std::slice::from_ref(&ready));
        assert_eq!(
            value[0]["host_build"]["view"],
            json!({"uiAbi": 1, "maxField": 231})
        );
    }
}
