use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Local};
use clap::Args;
use lingxia_control_protocol::dev_session::broker::{SessionContent, SessionInfo};
use lingxia_control_protocol::dev_session::log_files::{rotated_log_path, session_log_files};
use lingxia_control_protocol::dev_session::{DevSessionEvent, DevSessionLog, DevSessionLogLevel};
use owo_colors::OwoColorize;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_millis(100);
const MISSING_FILE_BACKOFF: Duration = Duration::from_millis(500);

#[derive(Args, Clone)]
pub struct LogsOptions {
    /// Only include entries from this origin (prefix match)
    #[arg(value_name = "ORIGIN")]
    pub origin: Option<String>,

    /// Only include entries whose message/path/appid contains this text
    #[arg(long)]
    pub grep: Option<String>,

    /// Only include entries at this level
    #[arg(long, value_parser = ["verbose", "debug", "info", "warn", "error"])]
    pub level: Option<String>,

    /// Only include entries for this app id (exact match)
    #[arg(long)]
    pub app: Option<String>,

    /// Only include entries whose page path contains this text
    #[arg(long)]
    pub path: Option<String>,

    /// List origins currently present in the session, then exit
    #[arg(
        long,
        conflicts_with_all = ["origin", "follow", "level", "app", "path", "grep"]
    )]
    pub origins: bool,

    /// Show only the most recent N matching backlog entries (0 to skip backlog when --follow)
    #[arg(long, default_value_t = 200)]
    pub limit: usize,

    /// Print matching entries as JSONL
    #[arg(long = "jsonl", conflicts_with = "color")]
    pub json: bool,

    /// Keep running and stream new matching entries as they are appended
    #[arg(long, short = 'f')]
    pub follow: bool,

    /// Colorize output by level (TTY decoration; not for machine consumption)
    #[arg(long)]
    pub color: bool,
}

struct Filters {
    level: Option<DevSessionLogLevel>,
    origin: Option<String>,
    app: Option<String>,
    grep: Option<String>,
    path: Option<String>,
}

struct LogEntry {
    event: DevSessionEvent,
    log: DevSessionLog,
}

#[derive(Clone, Copy)]
struct RenderOpts {
    json: bool,
    pretty: bool,
    show_origin: bool,
    show_appid: bool,
}

pub fn execute(session: &SessionInfo, options: LogsOptions) -> Result<()> {
    let log_file = Path::new(&session.log_file);
    if options.origins {
        return list_origins(log_file, options.json);
    }

    let filters = Filters {
        level: options.level.as_deref().map(parse_level).transpose()?,
        origin: options.origin.as_deref().map(str::to_lowercase),
        app: options.app.as_deref().map(str::to_lowercase),
        grep: options.grep.as_deref().map(str::to_lowercase),
        path: options.path.as_deref().map(str::to_lowercase),
    };
    let render = RenderOpts {
        json: options.json,
        pretty: options.color,
        show_origin: options.origin.is_none(),
        show_appid: matches!(session.content, Some(SessionContent::Host { .. })),
    };

    let start = drain_backlog(log_file, &filters, options.limit, render, options.follow)?;

    if options.follow {
        if render.pretty {
            println!("{}", "── live (Ctrl+C to exit) ──".dimmed());
        }
        tail_loop(session, log_file, start, &filters, render)?;
    }
    Ok(())
}

fn list_origins(log_file: &Path, json: bool) -> Result<()> {
    let mut origins = std::collections::BTreeSet::new();
    for path in existing_log_files(log_file)? {
        let file =
            File::open(&path).with_context(|| format!("Failed to open {}", path.display()))?;
        for line in BufReader::new(file).lines() {
            let line = line.context("Failed to read session event line")?;
            if line.trim().is_empty() {
                continue;
            }
            let event: DevSessionEvent =
                serde_json::from_str(&line).context("Failed to parse session event JSON line")?;
            if event.kind == lingxia_control_protocol::dev_session::event_kinds::LOG {
                origins.insert(event.origin);
            }
        }
    }

    for origin in origins {
        if json {
            println!("{}", serde_json::to_string(&origin)?);
        } else {
            println!("{origin}");
        }
    }
    Ok(())
}

/// The session's log files, oldest first (rotated ones, then the current);
/// an error when there is none at all.
fn existing_log_files(log_file: &Path) -> Result<Vec<PathBuf>> {
    let files = session_log_files(log_file);
    if files.is_empty() {
        File::open(log_file).with_context(|| format!("Failed to open {}", log_file.display()))?;
    }
    Ok(files)
}

/// Where following starts: the current file's end, and which file that is.
struct Position {
    offset: u64,
    identity: Option<FileIdentity>,
}

fn drain_backlog(
    log_file: &Path,
    filters: &Filters,
    limit: usize,
    render: RenderOpts,
    follow: bool,
) -> Result<Position> {
    let files = existing_log_files(log_file)?;
    let end = || -> Result<Position> {
        match File::open(log_file) {
            Ok(file) => {
                let metadata = file.metadata()?;
                Ok(Position {
                    offset: metadata.len(),
                    identity: file_identity(&metadata),
                })
            }
            Err(_) => Ok(Position {
                offset: 0,
                identity: None,
            }),
        }
    };
    if follow && limit == 0 {
        return end();
    }

    // Rotated files first: the backlog reads as one log.
    let mut matches = std::collections::VecDeque::new();
    for path in &files {
        let file =
            File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
        for line in BufReader::new(file).lines() {
            let line = line.context("Failed to read log line")?;
            if let Some(entry) = parse_and_filter(&line, filters)? {
                if matches.len() == limit {
                    matches.pop_front();
                }
                if limit > 0 {
                    matches.push_back(entry);
                }
            }
        }
    }
    for entry in matches {
        println!("{}", render_entry(&entry, render)?);
    }
    end()
}

/// Tell a rotated-away file from its replacement at the same path.
#[cfg(unix)]
type FileIdentity = (u64, u64);
#[cfg(not(unix))]
type FileIdentity = ();

#[cfg(unix)]
fn file_identity(metadata: &std::fs::Metadata) -> Option<FileIdentity> {
    use std::os::unix::fs::MetadataExt;
    Some((metadata.dev(), metadata.ino()))
}

#[cfg(not(unix))]
fn file_identity(_metadata: &std::fs::Metadata) -> Option<FileIdentity> {
    None
}

/// Print the complete lines of `file` from `offset`, keeping a trailing
/// half line in `pending` until the rest arrives.
fn read_new_lines(
    file: &File,
    offset: &mut u64,
    pending: &mut String,
    filters: &Filters,
    render: RenderOpts,
) -> Result<()> {
    let mut file = file;
    file.seek(SeekFrom::Start(*offset))?;
    let mut reader = BufReader::new(file);
    loop {
        let mut buf = String::new();
        let read = reader.read_line(&mut buf)?;
        if read == 0 {
            return Ok(());
        }
        pending.push_str(&buf);
        *offset += read as u64;
        if !pending.ends_with('\n') {
            // Half-line; wait for the rest before parsing.
            return Ok(());
        }
        let line = std::mem::take(pending);
        if let Some(entry) = parse_and_filter(line.trim_end_matches('\n'), filters)? {
            println!("{}", render_entry(&entry, render)?);
        }
    }
}

fn tail_loop(
    session: &SessionInfo,
    log_file: &Path,
    start: Position,
    filters: &Filters,
    render: RenderOpts,
) -> Result<()> {
    let Position {
        mut offset,
        mut identity,
    } = start;
    let mut pending = String::new();
    let mut polls: u8 = 0;
    loop {
        let file = match File::open(log_file) {
            Ok(f) => f,
            Err(_) => {
                if !session_owner_alive(session) {
                    eprintln!("Dev session ended.");
                    return Ok(());
                }
                thread::sleep(MISSING_FILE_BACKOFF);
                continue;
            }
        };

        let metadata = file.metadata()?;
        let current = file_identity(&metadata);
        let replaced = identity.is_some() && current != identity;
        if replaced || metadata.len() < offset {
            // Rotated: finish the file we were reading (now the first
            // rotated one), then read the new one from its start.
            let previous = rotated_log_path(log_file, 1);
            if let Ok(rotated) = File::open(&previous)
                && identity.is_some()
                && rotated.metadata().ok().as_ref().and_then(file_identity) == identity
            {
                read_new_lines(&rotated, &mut offset, &mut pending, filters, render)?;
            }
            offset = 0;
            pending.clear();
        }
        identity = current;

        if metadata.len() > offset {
            read_new_lines(&file, &mut offset, &mut pending, filters, render)?;
        }

        polls = polls.wrapping_add(1);
        if polls.is_multiple_of(10) && !session_owner_alive(session) {
            eprintln!("Dev session ended.");
            return Ok(());
        }

        thread::sleep(POLL_INTERVAL);
    }
}

/// Match the original owner, not just a PID that may have been reused. A
/// zombie has exited even if its parent has not reaped it yet.
fn session_owner_alive(session: &SessionInfo) -> bool {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessStatus, ProcessesToUpdate, System, UpdateKind};
    let pid = Pid::from_u32(session.pid);
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
    );
    let Some(process) = system.process(pid) else {
        return false;
    };
    process.status() != ProcessStatus::Zombie
        && (session.executable.is_empty() || process.exe() == Some(Path::new(&session.executable)))
        && (session.started_at == 0 || process.start_time() <= session.started_at / 1000 + 2)
}

fn parse_and_filter(line: &str, filters: &Filters) -> Result<Option<LogEntry>> {
    if line.trim().is_empty() {
        return Ok(None);
    }
    let event: DevSessionEvent =
        serde_json::from_str(line).context("Failed to parse session event JSON line")?;
    let Some(log) = event
        .as_log()
        .context("Failed to parse session log event")?
    else {
        return Ok(None);
    };
    let entry = LogEntry { event, log };
    Ok(matches_filters(&entry, filters).then_some(entry))
}

fn matches_filters(entry: &LogEntry, filters: &Filters) -> bool {
    if let Some(level) = filters.level
        && entry.log.level != level
    {
        return false;
    }
    if let Some(origin) = filters.origin.as_deref()
        && !entry.event.origin.to_lowercase().starts_with(origin)
    {
        return false;
    }
    if let Some(app_filter) = filters.app.as_deref() {
        let hay = entry.log.appid.as_deref().unwrap_or("").to_lowercase();
        if hay != app_filter {
            return false;
        }
    }
    if let Some(path_filter) = filters.path.as_deref() {
        let hay = entry.log.path.as_deref().unwrap_or("").to_lowercase();
        if !hay.contains(path_filter) {
            return false;
        }
    }
    if let Some(grep) = filters.grep.as_deref() {
        let mut haystacks = vec![
            entry.log.message.to_lowercase(),
            entry.event.origin.to_lowercase(),
        ];
        if let Some(path) = entry.log.path.as_deref() {
            haystacks.push(path.to_lowercase());
        }
        if let Some(appid) = entry.log.appid.as_deref() {
            haystacks.push(appid.to_lowercase());
        }
        if let Some(target) = entry.log.target.as_deref() {
            haystacks.push(target.to_lowercase());
        }
        if !haystacks.iter().any(|hay| hay.contains(grep)) {
            return false;
        }
    }
    true
}

fn render_entry(entry: &LogEntry, render: RenderOpts) -> Result<String> {
    if render.json {
        return serde_json::to_string(&entry.event).context("Failed to encode session event JSON");
    }
    let dt = DateTime::from_timestamp_millis(entry.event.timestamp_ms as i64)
        .ok_or_else(|| anyhow!("Invalid log timestamp: {}", entry.event.timestamp_ms))?
        .with_timezone(&Local);
    let timestamp = dt.format("%H:%M:%S%.3f").to_string();
    let level = format_level(entry.log.level);
    let origin = entry.event.origin.as_str();
    let context = context_column(&entry.log, render.show_appid);

    if render.pretty {
        let level_field = format!("{level:<7}");
        let level_colored = match entry.log.level {
            DevSessionLogLevel::Error => level_field.red().bold().to_string(),
            DevSessionLogLevel::Warn => level_field.yellow().bold().to_string(),
            DevSessionLogLevel::Info => level_field.clone(),
            DevSessionLogLevel::Debug | DevSessionLogLevel::Verbose => {
                level_field.dimmed().to_string()
            }
        };
        let mut line = format!("{} {}", timestamp.dimmed(), level_colored);
        if render.show_origin {
            line.push(' ');
            line.push_str(&origin.dimmed().to_string());
        }
        if !context.is_empty() {
            line.push(' ');
            line.push_str(&context.dimmed().to_string());
        }
        line.push(' ');
        line.push_str(&entry.log.message);
        Ok(line)
    } else {
        let mut prefix = format!("{timestamp} {level:<7}");
        if render.show_origin {
            prefix.push(' ');
            prefix.push_str(origin);
        }
        if !context.is_empty() {
            prefix.push(' ');
            prefix.push_str(&context);
        }
        Ok(format!("{prefix} {}", entry.log.message))
    }
}

/// Render only context that can vary inside the selected session. Host
/// sessions may contain several apps; Runner-style sessions bind one app.
fn context_column(log: &DevSessionLog, show_appid: bool) -> String {
    let path = log.path.as_deref().unwrap_or("").trim();
    let appid = log.appid.as_deref().unwrap_or("").trim();
    match (show_appid, appid.is_empty(), path.is_empty()) {
        (true, false, false) => format!("{appid}/{path}"),
        (true, false, true) => appid.to_string(),
        _ => path.to_string(),
    }
}

fn parse_level(value: &str) -> Result<DevSessionLogLevel> {
    match value {
        "verbose" => Ok(DevSessionLogLevel::Verbose),
        "debug" => Ok(DevSessionLogLevel::Debug),
        "info" => Ok(DevSessionLogLevel::Info),
        "warn" => Ok(DevSessionLogLevel::Warn),
        "error" => Ok(DevSessionLogLevel::Error),
        _ => Err(anyhow!("Unsupported log level: {}", value)),
    }
}

fn format_level(level: DevSessionLogLevel) -> &'static str {
    match level {
        DevSessionLogLevel::Verbose => "VERBOSE",
        DevSessionLogLevel::Debug => "DEBUG",
        DevSessionLogLevel::Info => "INFO",
        DevSessionLogLevel::Warn => "WARN",
        DevSessionLogLevel::Error => "ERROR",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(origin: &str, appid: &str, path: &str) -> LogEntry {
        let log = DevSessionLog {
            level: DevSessionLogLevel::Info,
            appid: Some(appid.to_string()),
            path: Some(path.to_string()),
            target: None,
            message: "hi".to_string(),
            attributes: Default::default(),
        };
        let event = DevSessionEvent::log(0, origin, log.clone()).unwrap();
        LogEntry { event, log }
    }

    fn no_filters() -> Filters {
        Filters {
            level: None,
            origin: None,
            app: None,
            grep: None,
            path: None,
        }
    }

    #[test]
    fn origin_filter_accepts_dynamic_prefixes() {
        let mut filters = no_filters();
        filters.origin = Some("service".to_string());
        assert!(matches_filters(
            &entry("service.api", "com.demo.app", "x"),
            &filters
        ));
        assert!(!matches_filters(
            &entry("lxview", "com.demo.app", "x"),
            &filters
        ));
    }

    #[test]
    fn origin_column_separates_browser_from_page() {
        let render = RenderOpts {
            json: false,
            pretty: false,
            show_origin: true,
            show_appid: false,
        };
        let page = render_entry(&entry("lxview", "com.demo.app", "pages/home"), render).unwrap();
        let tab = render_entry(
            &entry("browser", "app.lingxia.browser", "https://example.com/"),
            render,
        )
        .unwrap();
        assert!(page.contains("lxview"), "{page}");
        assert!(tab.contains("browser"), "{tab}");
    }

    #[test]
    fn host_session_context_includes_appid() {
        let render = RenderOpts {
            json: false,
            pretty: false,
            show_origin: true,
            show_appid: true,
        };
        let line = render_entry(&entry("lxview", "com.demo.app", "pages/home"), render).unwrap();
        assert!(line.contains("com.demo.app/pages/home"), "{line}");
    }

    #[test]
    fn selected_origin_is_not_repeated() {
        let render = RenderOpts {
            json: false,
            pretty: false,
            show_origin: false,
            show_appid: false,
        };
        let line = render_entry(&entry("service.api", "com.demo.app", ""), render).unwrap();
        assert!(!line.contains("service.api"), "{line}");
    }

    #[test]
    fn app_filter_matches_exact_appid() {
        let mut filters = no_filters();
        filters.app = Some("app.lingxia.browser".to_string());
        assert!(matches_filters(
            &entry("browser", "app.lingxia.browser", "x"),
            &filters
        ));
        assert!(!matches_filters(
            &entry("lxview", "com.demo.app", "x"),
            &filters
        ));
    }

    fn current_session() -> SessionInfo {
        SessionInfo {
            session_id: "test".into(),
            project_root: String::new(),
            content: None,
            target: "runner".into(),
            pid: std::process::id(),
            started_at: 0,
            executable: std::env::current_exe().unwrap().display().to_string(),
            ws_url: String::new(),
            log_file: String::new(),
            name: None,
            build: None,
            extra: Default::default(),
        }
    }

    #[test]
    fn session_owner_matches_process_identity() {
        let mut session = current_session();
        assert!(session_owner_alive(&session));
        session.started_at = 1;
        assert!(!session_owner_alive(&session), "reject a reused PID");
        session.started_at = 0;
        session.executable = "/not/the/session/owner".into();
        assert!(!session_owner_alive(&session));
        session.pid = u32::MAX;
        assert!(!session_owner_alive(&session));
    }

    #[cfg(unix)]
    #[test]
    fn exited_unreaped_owner_does_not_keep_logs_alive() {
        let mut child = std::process::Command::new("sh")
            .args(["-c", "exit 0"])
            .spawn()
            .unwrap();
        let mut session = current_session();
        session.pid = child.id();
        session.executable.clear();
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while session_owner_alive(&session) && std::time::Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        let alive = session_owner_alive(&session);
        child.wait().unwrap();
        assert!(
            !alive,
            "an unreaped exited process must not keep logs -f polling"
        );
    }

    #[test]
    fn origin_filter_selects_browser_only() {
        let mut filters = no_filters();
        filters.origin = Some("browser".to_string());
        assert!(matches_filters(
            &entry("browser", "app.lingxia.browser", "x"),
            &filters
        ));
        assert!(!matches_filters(
            &entry("lxview", "com.demo.app", "x"),
            &filters
        ));
    }
}
