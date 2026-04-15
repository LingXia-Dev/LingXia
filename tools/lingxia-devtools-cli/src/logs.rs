use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Local};
use clap::Args;
use lingxia_devtool_protocol::{DevtoolsLogLevel, DevtoolsLogMessage, DevtoolsLogSource};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Args, Clone)]
pub struct LogsOptions {
    /// Only include entries whose message/path/appid contains this text
    #[arg(long)]
    pub grep: Option<String>,

    /// Only include entries at this level
    #[arg(long, value_parser = ["verbose", "debug", "info", "warn", "error"])]
    pub level: Option<String>,

    /// Only include entries for this source
    #[arg(
        long,
        alias = "tag",
        value_parser = ["native", "webview", "logic", "web_view_console", "lx_app_service_console"]
    )]
    pub source: Option<String>,

    /// Only include entries whose page path contains this text
    #[arg(long)]
    pub path: Option<String>,

    /// Show only the most recent N matching entries
    #[arg(long, default_value_t = 200)]
    pub limit: usize,

    /// Print matching entries as JSONL
    #[arg(long)]
    pub json: bool,
}

pub fn execute(log_file: &Path, options: LogsOptions) -> Result<()> {
    let level_filter = options.level.as_deref().map(parse_level).transpose()?;
    let source_filter = options.source.as_deref().map(parse_source).transpose()?;
    let grep = options.grep.as_deref().map(str::to_lowercase);
    let path = options.path.as_deref().map(str::to_lowercase);

    let file =
        File::open(log_file).with_context(|| format!("Failed to open {}", log_file.display()))?;
    let reader = BufReader::new(file);
    let mut matches = Vec::new();

    for line in reader.lines() {
        let line = line.context("Failed to read log line")?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: DevtoolsLogMessage =
            serde_json::from_str(&line).context("Failed to parse log JSON line")?;
        if !matches_filters(
            &entry,
            level_filter,
            source_filter,
            grep.as_deref(),
            path.as_deref(),
        ) {
            continue;
        }
        matches.push(entry);
    }

    let start = matches.len().saturating_sub(options.limit);
    for entry in matches.into_iter().skip(start) {
        if options.json {
            println!(
                "{}",
                serde_json::to_string(&entry).context("Failed to encode log JSON")?
            );
        } else {
            println!("{}", format_entry(&entry)?);
        }
    }

    Ok(())
}

fn matches_filters(
    entry: &DevtoolsLogMessage,
    level_filter: Option<DevtoolsLogLevel>,
    source_filter: Option<DevtoolsLogSource>,
    grep: Option<&str>,
    path_filter: Option<&str>,
) -> bool {
    if let Some(level_filter) = level_filter
        && entry.level != level_filter
    {
        return false;
    }
    if let Some(source_filter) = source_filter
        && entry.source != source_filter
    {
        return false;
    }
    if let Some(path_filter) = path_filter {
        let hay = entry.path.as_deref().unwrap_or("").to_lowercase();
        if !hay.contains(path_filter) {
            return false;
        }
    }
    if let Some(grep) = grep {
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

fn format_entry(entry: &DevtoolsLogMessage) -> Result<String> {
    let dt = DateTime::from_timestamp_millis(entry.timestamp_ms as i64)
        .ok_or_else(|| anyhow!("Invalid log timestamp: {}", entry.timestamp_ms))?
        .with_timezone(&Local);
    let mut prefix = format!(
        "{} {:<7} {:<22}",
        dt.format("%H:%M:%S%.3f"),
        format_level(entry.level),
        format_source(entry.source)
    );
    if let Some(path) = entry.path.as_deref()
        && !path.is_empty()
    {
        prefix.push(' ');
        prefix.push_str(path);
    }
    Ok(format!("{prefix} {}", entry.message))
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
        "webview" | "web_view_console" => Ok(DevtoolsLogSource::WebViewConsole),
        "logic" | "lx_app_service_console" => Ok(DevtoolsLogSource::LxAppServiceConsole),
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
        DevtoolsLogSource::WebViewConsole => "webview",
        DevtoolsLogSource::LxAppServiceConsole => "logic",
    }
}
