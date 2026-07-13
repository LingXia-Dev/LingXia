use crate::client;
use crate::project::{self, SessionSelector};
use anyhow::Result;
use chrono::{DateTime, Local, TimeZone};
use lingxia_devtool_protocol::handlers;
use serde_json::{Value, json};

pub fn execute_list(json_output: bool) -> Result<()> {
    let sessions = project::list_selectable_sessions()?;

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
                    "remote": s.remote_name.is_some(),
                    "remote_name": s.remote_name,
                    "stale": if s.remote_name.is_some() {
                        !project::remote_is_reachable(s)
                    } else {
                        project::is_stale(s)
                    },
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&Value::Array(array))?);
        return Ok(());
    }

    if sessions.is_empty() {
        println!("No live dev sessions. Run `lingxia dev`, or `lxdev attach <ws-url>`.");
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
        "{:<id_width$}  {:<target_width$}  {:<19}  {:<ws_width$}  PROJECT",
        "ID", "TARGET", "STARTED", "WS"
    );
    for info in sessions.iter() {
        // Remote rows carry the same identity as local ones (fetched live via
        // session.info); the attach name tags the project column and the
        // non-loopback ws URL is what marks them as remote.
        let location = match info.remote_name.as_deref() {
            Some(name) if !project::remote_is_reachable(info) => {
                format!("[{name}] unreachable")
            }
            Some(name) => format!("[{name}] {}", info.project_root),
            None => info.project_root.clone(),
        };
        println!(
            "{:<id_width$}  {:<target_width$}  {:<19}  {:<ws_width$}  {}",
            info.session_id,
            info.target,
            if info.started_at == 0 {
                "-".to_string()
            } else {
                format_started(info.started_at)
            },
            info.ws_url,
            location,
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
