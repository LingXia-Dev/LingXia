use crate::client;
use crate::project::{self, SessionSelector};
use anyhow::Result;
use chrono::{DateTime, Local, TimeZone};
use lingxia_devtool_protocol::handlers;
use serde_json::{Value, json};

pub fn execute_list(json_output: bool) -> Result<()> {
    let sessions = project::list_all_sessions()?;

    if json_output {
        let array: Vec<Value> = sessions
            .iter()
            .map(|s| {
                json!({
                    "session_id": s.session_id,
                    "pid": s.pid,
                    "target": s.target,
                    "project_root": s.project_root,
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
        println!("No live dev sessions.");
        return Ok(());
    }

    println!(
        "{:<8}  {:<8}  {:<19}  {:<22}  PROJECT",
        "ID", "TARGET", "STARTED", "WS"
    );
    for info in sessions.iter() {
        println!(
            "{:<8}  {:<8}  {:<19}  {:<22}  {}",
            info.session_id,
            info.target,
            format_started(info.started_at),
            info.ws_url,
            info.project_root,
        );
    }
    Ok(())
}

pub fn execute_stop(selector: &SessionSelector) -> Result<()> {
    let info = project::resolve_session(selector)?;
    client::execute_command(&info.ws_url, handlers::session::SHUTDOWN, None)?;
    println!(
        "Stop requested for {} dev session {}.",
        info.target, info.session_id
    );
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
