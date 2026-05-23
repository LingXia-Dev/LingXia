use crate::project;
use anyhow::Result;
use chrono::{DateTime, Local, TimeZone};
use serde_json::{Value, json};
use std::path::Path;

pub fn execute_list(project_root: &Path, json_output: bool) -> Result<()> {
    let sessions = project::list_all_sessions(project_root)?;

    if json_output {
        let array: Vec<Value> = sessions
            .iter()
            .map(|s| {
                json!({
                    "session_id": s.session_id,
                    "pid": s.pid,
                    "platform": s.platform,
                    "started_at": s.started_at,
                    "ws_url": s.ws_url,
                    "log_file": s.log_file,
                    "stale": project::is_stale(s),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&Value::Array(array))?);
        return Ok(());
    }

    if sessions.is_empty() {
        println!("No active dev sessions.");
        return Ok(());
    }

    println!(
        "{:<36}  {:<5}  {:<8}  {:<19}  {:<22}  {}",
        "ID", "STATE", "PLATFORM", "STARTED", "WS", "PID"
    );
    for info in &sessions {
        let state = if project::is_stale(info) {
            "stale"
        } else {
            "live"
        };
        println!(
            "{:<36}  {:<5}  {:<8}  {:<19}  {:<22}  {}",
            info.session_id,
            state,
            info.platform,
            format_started(info.started_at),
            info.ws_url,
            info.pid,
        );
    }
    Ok(())
}

pub fn execute_prune(project_root: &Path) -> Result<()> {
    let pruned = project::prune_stale(project_root)?;
    if pruned.is_empty() {
        println!("No stale dev sessions to prune.");
        return Ok(());
    }
    println!("Pruned {} stale session(s):", pruned.len());
    for info in &pruned {
        println!(
            "  {}  platform={}  ws={}",
            info.session_id, info.platform, info.ws_url
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
