use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Local};
use clap::Args;
use lingxia_devtool_protocol::{DevtoolsLogLevel, DevtoolsLogMessage, DevtoolsLogSource};
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
    /// Only include entries whose message/path/appid contains this text
    #[arg(long)]
    pub grep: Option<String>,

    /// Only include entries at this level
    #[arg(long, value_parser = ["verbose", "debug", "info", "warn", "error"])]
    pub level: Option<String>,

    /// Only include entries for this source
    #[arg(long, alias = "tag", value_parser = ["native", "lxview", "lxlogic", "browser", "automation"])]
    pub source: Option<String>,

    /// Only include entries for this app id (exact match)
    #[arg(long)]
    pub app: Option<String>,

    /// Only include entries whose page path contains this text
    #[arg(long)]
    pub path: Option<String>,

    /// Prefix the origin column with the app id (useful when several lxapps
    /// or the built-in browser share one session)
    #[arg(long)]
    pub wide: bool,

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
    level: Option<DevtoolsLogLevel>,
    source: Option<DevtoolsLogSource>,
    app: Option<String>,
    grep: Option<String>,
    path: Option<String>,
}

#[derive(Clone, Copy)]
struct RenderOpts {
    json: bool,
    pretty: bool,
    wide: bool,
}

pub fn execute(log_file: &Path, options: LogsOptions) -> Result<()> {
    let filters = Filters {
        level: options.level.as_deref().map(parse_level).transpose()?,
        source: options.source.as_deref().map(parse_source).transpose()?,
        app: options.app.as_deref().map(str::to_lowercase),
        grep: options.grep.as_deref().map(str::to_lowercase),
        path: options.path.as_deref().map(str::to_lowercase),
    };
    let render = RenderOpts {
        json: options.json,
        pretty: options.pretty,
        wide: options.wide,
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

fn parse_and_filter(line: &str, filters: &Filters) -> Result<Option<DevtoolsLogMessage>> {
    if line.trim().is_empty() {
        return Ok(None);
    }
    let entry: DevtoolsLogMessage =
        serde_json::from_str(line).context("Failed to parse log JSON line")?;
    Ok(matches_filters(&entry, filters).then_some(entry))
}

fn matches_filters(entry: &DevtoolsLogMessage, filters: &Filters) -> bool {
    if let Some(level) = filters.level
        && entry.level != level
    {
        return false;
    }
    if let Some(source) = filters.source
        && entry.source != source
    {
        return false;
    }
    if let Some(app_filter) = filters.app.as_deref() {
        let hay = entry.appid.as_deref().unwrap_or("").to_lowercase();
        if hay != app_filter {
            return false;
        }
    }
    if let Some(path_filter) = filters.path.as_deref() {
        let hay = entry.path.as_deref().unwrap_or("").to_lowercase();
        if !hay.contains(path_filter) {
            return false;
        }
    }
    if let Some(grep) = filters.grep.as_deref() {
        let mut haystacks = vec![entry.message.to_lowercase()];
        if let Some(path) = entry.path.as_deref() {
            haystacks.push(path.to_lowercase());
        }
        if let Some(appid) = entry.appid.as_deref() {
            haystacks.push(appid.to_lowercase());
        }
        if !haystacks.iter().any(|hay| hay.contains(grep)) {
            return false;
        }
    }
    true
}

fn render_entry(entry: &DevtoolsLogMessage, render: RenderOpts) -> Result<String> {
    if render.json {
        return serde_json::to_string(entry).context("Failed to encode log JSON");
    }
    let dt = DateTime::from_timestamp_millis(entry.timestamp_ms as i64)
        .ok_or_else(|| anyhow!("Invalid log timestamp: {}", entry.timestamp_ms))?
        .with_timezone(&Local);
    let timestamp = dt.format("%H:%M:%S%.3f").to_string();
    let level = format_level(entry.level);
    let source = format_source(entry.source);
    let origin = origin_column(entry, render.wide);

    if render.pretty {
        let level_field = format!("{level:<7}");
        let source_field = format!("{source:<7}");
        let level_colored = match entry.level {
            DevtoolsLogLevel::Error => level_field.red().bold().to_string(),
            DevtoolsLogLevel::Warn => level_field.yellow().bold().to_string(),
            DevtoolsLogLevel::Info => level_field.clone(),
            DevtoolsLogLevel::Debug | DevtoolsLogLevel::Verbose => level_field.dimmed().to_string(),
        };
        let mut line = format!(
            "{} {} {}",
            timestamp.dimmed(),
            level_colored,
            source_field.dimmed()
        );
        if !origin.is_empty() {
            line.push(' ');
            line.push_str(&origin.dimmed().to_string());
        }
        line.push(' ');
        line.push_str(&entry.message);
        Ok(line)
    } else {
        let mut prefix = format!("{timestamp} {level:<7} {source:<7}");
        if !origin.is_empty() {
            prefix.push(' ');
            prefix.push_str(&origin);
        }
        Ok(format!("{prefix} {}", entry.message))
    }
}

/// The origin column: the page path, optionally prefixed with the app id when
/// `--wide` is set so entries from different lxapps (or the built-in browser)
/// are distinguishable on the line.
fn origin_column(entry: &DevtoolsLogMessage, wide: bool) -> String {
    let path = entry.path.as_deref().unwrap_or("").trim();
    let appid = entry.appid.as_deref().unwrap_or("").trim();
    match (wide, appid.is_empty(), path.is_empty()) {
        (true, false, false) => format!("{appid}:{path}"),
        (true, false, true) => appid.to_string(),
        _ => path.to_string(),
    }
}

fn parse_level(value: &str) -> Result<DevtoolsLogLevel> {
    match value {
        "verbose" => Ok(DevtoolsLogLevel::Verbose),
        "debug" => Ok(DevtoolsLogLevel::Debug),
        "info" => Ok(DevtoolsLogLevel::Info),
        "warn" => Ok(DevtoolsLogLevel::Warn),
        "error" => Ok(DevtoolsLogLevel::Error),
        _ => Err(anyhow!("Unsupported log level: {}", value)),
    }
}

fn parse_source(value: &str) -> Result<DevtoolsLogSource> {
    match value {
        "native" => Ok(DevtoolsLogSource::Native),
        "lxview" => Ok(DevtoolsLogSource::WebViewConsole),
        "lxlogic" => Ok(DevtoolsLogSource::LxAppServiceConsole),
        "browser" => Ok(DevtoolsLogSource::BrowserConsole),
        "automation" => Ok(DevtoolsLogSource::Automation),
        _ => Err(anyhow!("Unsupported log source: {}", value)),
    }
}

fn format_level(level: DevtoolsLogLevel) -> &'static str {
    match level {
        DevtoolsLogLevel::Verbose => "VERBOSE",
        DevtoolsLogLevel::Debug => "DEBUG",
        DevtoolsLogLevel::Info => "INFO",
        DevtoolsLogLevel::Warn => "WARN",
        DevtoolsLogLevel::Error => "ERROR",
    }
}

fn format_source(source: DevtoolsLogSource) -> &'static str {
    match source {
        DevtoolsLogSource::Native => "native",
        DevtoolsLogSource::WebViewConsole => "lxview",
        DevtoolsLogSource::LxAppServiceConsole => "lxlogic",
        DevtoolsLogSource::BrowserConsole => "browser",
        DevtoolsLogSource::Automation => "automation",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(source: DevtoolsLogSource, appid: &str, path: &str) -> DevtoolsLogMessage {
        DevtoolsLogMessage {
            timestamp_ms: 0,
            source,
            level: DevtoolsLogLevel::Info,
            appid: Some(appid.to_string()),
            path: Some(path.to_string()),
            message: "hi".to_string(),
        }
    }

    fn no_filters() -> Filters {
        Filters {
            level: None,
            source: None,
            app: None,
            grep: None,
            path: None,
        }
    }

    #[test]
    fn source_names_are_the_five_canonical_values() {
        assert_eq!(parse_source("native").unwrap(), DevtoolsLogSource::Native);
        assert_eq!(
            parse_source("lxview").unwrap(),
            DevtoolsLogSource::WebViewConsole
        );
        assert_eq!(
            parse_source("lxlogic").unwrap(),
            DevtoolsLogSource::LxAppServiceConsole
        );
        assert_eq!(
            parse_source("browser").unwrap(),
            DevtoolsLogSource::BrowserConsole
        );
        assert_eq!(
            parse_source("automation").unwrap(),
            DevtoolsLogSource::Automation
        );
        assert!(parse_source("webview").is_err());
        assert!(parse_source("browser_console").is_err());
        assert_eq!(format_source(DevtoolsLogSource::WebViewConsole), "lxview");
        assert_eq!(
            format_source(DevtoolsLogSource::LxAppServiceConsole),
            "lxlogic"
        );
        assert_eq!(format_source(DevtoolsLogSource::BrowserConsole), "browser");
    }

    #[test]
    fn source_column_separates_browser_from_page() {
        let render = RenderOpts {
            json: false,
            pretty: false,
            wide: false,
        };
        let page = render_entry(
            &entry(
                DevtoolsLogSource::WebViewConsole,
                "com.demo.app",
                "pages/home",
            ),
            render,
        )
        .unwrap();
        let tab = render_entry(
            &entry(
                DevtoolsLogSource::BrowserConsole,
                "app.lingxia.browser",
                "https://example.com/",
            ),
            render,
        )
        .unwrap();
        assert!(page.contains("lxview"), "{page}");
        assert!(tab.contains("browser"), "{tab}");
    }

    #[test]
    fn wide_prefixes_appid_onto_origin() {
        let render = RenderOpts {
            json: false,
            pretty: false,
            wide: true,
        };
        let line = render_entry(
            &entry(
                DevtoolsLogSource::WebViewConsole,
                "com.demo.app",
                "pages/home",
            ),
            render,
        )
        .unwrap();
        assert!(line.contains("com.demo.app:pages/home"), "{line}");
    }

    #[test]
    fn app_filter_matches_exact_appid() {
        let mut filters = no_filters();
        filters.app = Some("app.lingxia.browser".to_string());
        assert!(matches_filters(
            &entry(
                DevtoolsLogSource::BrowserConsole,
                "app.lingxia.browser",
                "x"
            ),
            &filters
        ));
        assert!(!matches_filters(
            &entry(DevtoolsLogSource::WebViewConsole, "com.demo.app", "x"),
            &filters
        ));
    }

    #[test]
    fn source_filter_selects_browser_only() {
        let mut filters = no_filters();
        filters.source = Some(DevtoolsLogSource::BrowserConsole);
        assert!(matches_filters(
            &entry(
                DevtoolsLogSource::BrowserConsole,
                "app.lingxia.browser",
                "x"
            ),
            &filters
        ));
        assert!(!matches_filters(
            &entry(DevtoolsLogSource::WebViewConsole, "com.demo.app", "x"),
            &filters
        ));
    }
}
