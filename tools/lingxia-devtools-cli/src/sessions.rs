use crate::project;
use anyhow::Result;
use chrono::{DateTime, Local, TimeZone};
use serde_json::{Value, json};

pub fn execute_list(json_output: bool) -> Result<()> {
    let sessions = project::list_all_sessions()?;
    let probes: Vec<project::SessionProbe> = sessions.iter().map(project::probe_session).collect();

    if json_output {
        let array: Vec<Value> = sessions
            .iter()
            .zip(&probes)
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
                    "cli_build": s.build,
                    "host_build": probe.runtime_build,
                    "state": probe.state.as_str(),
                    "runtime_connected": probe.state == project::SessionState::Ready,
                    "stale": probe.state == project::SessionState::Stale,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&Value::Array(array))?);
        return Ok(());
    }

    if sessions.is_empty() {
        println!("No live dev sessions. Run `lingxia dev`.");
        return Ok(());
    }

    let rows: Vec<[String; 8]> = sessions
        .iter()
        .zip(&probes)
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
    let print_row = |cells: [&str; 8]| {
        let mut line = String::new();
        for (index, cell) in cells.iter().enumerate() {
            if index + 1 == cells.len() {
                line.push_str(cell);
            } else {
                line.push_str(&format!("{cell:<width$}  ", width = widths[index]));
            }
        }
        println!("{}", line.trim_end());
    };
    print_row(header);
    for row in &rows {
        print_row(row.each_ref().map(String::as_str));
    }
    println!();
    println!(
        "Select one with --session NAME|TARGET|TARGET@<dir>|#; name a session with \
         `lingxia dev --name NAME`."
    );
    Ok(())
}

/// `0.19.0/p2`: the host runtime's LingXia version and dev protocol.
fn host_label(probe: &project::SessionProbe) -> String {
    match &probe.runtime_build {
        Some(build) => format!("{}/p{}", build.version, build.protocol),
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
