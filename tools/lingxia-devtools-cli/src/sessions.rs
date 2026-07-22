use crate::project;
use anyhow::Result;
use chrono::{DateTime, Local, TimeZone};
use serde_json::{Value, json};

pub fn execute_list(json_output: bool) -> Result<()> {
    let sessions = project::list_all_sessions()?;

    if json_output {
        let array: Vec<Value> = sessions
            .iter()
            .map(|s| {
                let state = project::session_state(s);
                json!({
                    "session_id": s.session_id,
                    "pid": s.pid,
                    "target": s.target,
                    "context_root": s.project_root,
                    "content": s.content,
                    "started_at": s.started_at,
                    "ws_url": s.ws_url,
                    "log_file": s.log_file,
                    "state": state.as_str(),
                    "runtime_connected": state == project::SessionState::Ready,
                    "stale": state == project::SessionState::Stale,
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

    let id_width = sessions
        .iter()
        .map(|s| s.session_id.len())
        .chain([2])
        .max()
        .unwrap_or(2);
    let target_width = sessions
        .iter()
        .map(|s| s.target.len())
        .chain([6])
        .max()
        .unwrap_or(6);
    let ws_width = sessions
        .iter()
        .map(|s| s.ws_url.len())
        .chain([2])
        .max()
        .unwrap_or(2);
    println!(
        "{:<id_width$}  {:<target_width$}  {:<8}  {:<19}  {:<ws_width$}  CONTENT",
        "ID", "TARGET", "STATE", "STARTED", "WS"
    );
    for info in sessions.iter() {
        let state = project::session_state(info);
        println!(
            "{:<id_width$}  {:<target_width$}  {:<8}  {:<19}  {:<ws_width$}  {}",
            info.session_id,
            info.target,
            state.as_str(),
            if info.started_at == 0 {
                "-".to_string()
            } else {
                format_started(info.started_at)
            },
            info.ws_url,
            info.content
                .as_ref()
                .map(|content| content.display())
                .unwrap_or(&info.project_root),
        );
    }
    Ok(())
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
