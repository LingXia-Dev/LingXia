use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Local};
use clap::Args;
use lingxia_devtool_protocol::broker::{SessionContent, SessionInfo};
use lingxia_devtool_protocol::{DevSessionEvent, DevSessionLog, DevSessionLogLevel};
use owo_colors::OwoColorize;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;
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
    #[arg(long, conflicts_with = "pretty")]
    pub json: bool,

    /// Keep running and stream new matching entries as they are appended
    #[arg(long, short = 'f')]
    pub follow: bool,

    /// Colorize output by level (TTY decoration; not for machine consumption)
    #[arg(long)]
    pub pretty: bool,
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
        return list_origins(log_file, options.json || options.pretty, options.pretty);
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
        pretty: options.pretty,
        show_origin: options.origin.is_none(),
        show_appid: matches!(session.content, Some(SessionContent::Host { .. })),
    };

    let end_offset = drain_backlog(log_file, &filters, options.limit, render, options.follow)?;

    if options.follow {
        if render.pretty {
            println!("{}", "── live (Ctrl+C to exit) ──".dimmed());
        }
        tail_loop(log_file, end_offset, &filters, render)?;
    }
    Ok(())
}

fn list_origins(log_file: &Path, json: bool, pretty: bool) -> Result<()> {
    let file =
        File::open(log_file).with_context(|| format!("Failed to open {}", log_file.display()))?;
    let mut origins = std::collections::BTreeSet::new();
    for line in BufReader::new(file).lines() {
        let line = line.context("Failed to read session event line")?;
        if line.trim().is_empty() {
            continue;
        }
        let event: DevSessionEvent =
            serde_json::from_str(&line).context("Failed to parse session event JSON line")?;
        if event.kind == lingxia_devtool_protocol::event_kinds::LOG {
            origins.insert(event.origin);
        }
    }

    if json {
        let encoded = if pretty {
            serde_json::to_string_pretty(&origins)
        } else {
            serde_json::to_string(&origins)
        }
        .context("Failed to encode session origins")?;
        println!("{encoded}");
    } else {
        for origin in origins {
            println!("{origin}");
        }
    }
    Ok(())
}

fn drain_backlog(
    log_file: &Path,
    filters: &Filters,
    limit: usize,
    render: RenderOpts,
    follow: bool,
) -> Result<u64> {
    let mut file =
        File::open(log_file).with_context(|| format!("Failed to open {}", log_file.display()))?;
    let reader = BufReader::new(&file);

    if follow && limit == 0 {
        let end = file.seek(SeekFrom::End(0))?;
        return Ok(end);
    }

    let mut matches = Vec::new();
    for line in reader.lines() {
        let line = line.context("Failed to read log line")?;
        if let Some(entry) = parse_and_filter(&line, filters)? {
            matches.push(entry);
        }
    }

    let start = matches.len().saturating_sub(limit);
    for entry in matches.into_iter().skip(start) {
        println!("{}", render_entry(&entry, render)?);
    }

    let end = file.seek(SeekFrom::End(0))?;
    Ok(end)
}

fn tail_loop(
    log_file: &Path,
    mut offset: u64,
    filters: &Filters,
    render: RenderOpts,
) -> Result<()> {
    let mut pending = String::new();
    loop {
        let mut file = match File::open(log_file) {
            Ok(f) => f,
            Err(_) => {
                thread::sleep(MISSING_FILE_BACKOFF);
                continue;
            }
        };

        let len = file.metadata()?.len();
        if len < offset {
            // Truncation / rotation: replay from the start.
            offset = 0;
            pending.clear();
        }

        if len > offset {
            file.seek(SeekFrom::Start(offset))?;
            let mut reader = BufReader::new(&file);
            loop {
                let mut buf = String::new();
                let read = reader.read_line(&mut buf)?;
                if read == 0 {
                    break;
                }
                pending.push_str(&buf);
                offset += read as u64;
                if !pending.ends_with('\n') {
                    // Half-line; wait for the rest before parsing.
                    break;
                }
                let line = std::mem::take(&mut pending);
                if let Some(entry) = parse_and_filter(line.trim_end_matches('\n'), filters)? {
                    println!("{}", render_entry(&entry, render)?);
                }
            }
        }

        thread::sleep(POLL_INTERVAL);
    }
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
