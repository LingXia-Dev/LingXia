use anyhow::{anyhow, Result};
use std::env;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

const JS_CLI_ENV: &str = "LINGXIA_JS_CLI";

pub fn run(args: &[String]) -> Result<()> {
    run_in_dir(args, &env::current_dir()?)
}

pub fn run_in_dir(args: &[String], cwd: &Path) -> Result<()> {
    let js_cli = locate_js_cli()?;
    let status = Command::new("node")
        .arg(&js_cli)
        .args(args)
        .current_dir(cwd)
        .status()
        .map_err(|e| anyhow!("Failed to execute node: {}", e))?;

    if !status.success() {
        return Err(anyhow!("JS CLI exited with status {}", status));
    }
    Ok(())
}

fn locate_js_cli() -> Result<PathBuf> {
    if let Ok(path) = env::var(JS_CLI_ENV) {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
        return Err(anyhow!(
            "{} points to missing path: {}",
            JS_CLI_ENV,
            path.display()
        ));
    }

    Err(anyhow!(
        "JS CLI not found. Set {} to the path of dist/index.js",
        JS_CLI_ENV
    ))
}
