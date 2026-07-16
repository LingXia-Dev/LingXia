use crate::lxapp::framework::ProjectFramework;
use anyhow::{Result, anyhow};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressMode {
    Task,
    Plain,
}

#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub dev_session: bool,
    pub release: bool,
    pub package: bool,
    pub framework: Option<ProjectFramework>,
    pub progress: ProgressMode,
}

impl BuildOptions {
    pub fn parse(args: &[String]) -> Result<Self> {
        if args.is_empty() {
            return Err(anyhow!("Missing lxapp subcommand"));
        }
        if args[0] != "build" {
            return Err(anyhow!(
                "Unsupported lxapp subcommand {:?}; only \"build\" is supported for now",
                args[0]
            ));
        }

        let release = args.iter().any(|arg| arg == "--release");
        let package = args.iter().any(|arg| arg == "--package");
        let framework = parse_framework_arg(args)?;
        let progress = parse_progress_arg(args)?;

        Ok(Self {
            dev_session: false,
            release,
            package,
            framework,
            progress,
        })
    }
}

fn parse_framework_arg(args: &[String]) -> Result<Option<ProjectFramework>> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--framework" {
            let value = iter
                .next()
                .ok_or_else(|| anyhow!("--framework requires a value"))?;
            return Ok(Some(parse_framework_value(value)?));
        }
        if let Some(value) = arg.strip_prefix("--framework=") {
            return Ok(Some(parse_framework_value(value)?));
        }
    }
    Ok(None)
}

fn parse_framework_value(value: &str) -> Result<ProjectFramework> {
    match value {
        "react" => Ok(ProjectFramework::React),
        "vue" => Ok(ProjectFramework::Vue),
        "html" => Ok(ProjectFramework::Html),
        _ => Err(anyhow!(
            "Unsupported framework {value:?}; expected react, vue, or html"
        )),
    }
}

fn parse_progress_arg(args: &[String]) -> Result<ProgressMode> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--progress" {
            let value = iter
                .next()
                .ok_or_else(|| anyhow!("--progress requires a value"))?;
            return parse_progress_value(value);
        }
        if let Some(value) = arg.strip_prefix("--progress=") {
            return parse_progress_value(value);
        }
    }
    Ok(ProgressMode::Task)
}

fn parse_progress_value(value: &str) -> Result<ProgressMode> {
    match value {
        "task" => Ok(ProgressMode::Task),
        "plain" => Ok(ProgressMode::Plain),
        _ => Err(anyhow!(
            "Unsupported progress mode {value:?}; expected task or plain"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn parses_build_defaults() {
        let options = BuildOptions::parse(&args(&["build"])).unwrap();
        assert!(!options.dev_session);
        assert!(!options.release);
        assert!(!options.package);
        assert_eq!(options.framework, None);
        assert_eq!(options.progress, ProgressMode::Task);
    }

    #[test]
    fn parses_framework_and_progress_flags() {
        let options = BuildOptions::parse(&args(&[
            "build",
            "--release",
            "--package",
            "--framework=vue",
            "--progress",
            "plain",
        ]))
        .unwrap();

        assert!(!options.dev_session);
        assert!(options.release);
        assert!(options.package);
        assert_eq!(options.framework, Some(ProjectFramework::Vue));
        assert_eq!(options.progress, ProgressMode::Plain);
    }

    #[test]
    fn rejects_invalid_progress_mode() {
        let error = BuildOptions::parse(&args(&["build", "--progress", "dynamic"]))
            .unwrap_err()
            .to_string();
        assert!(error.contains("expected task or plain"));
    }

    #[test]
    fn rejects_missing_framework_value() {
        let error = BuildOptions::parse(&args(&["build", "--framework"]))
            .unwrap_err()
            .to_string();
        assert!(error.contains("--framework requires a value"));
    }
}
