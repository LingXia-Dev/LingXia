mod build;
mod framework;
mod logic;
mod options;
mod package;
mod project;
mod view;

use anyhow::Result;
use std::env;
use std::path::Path;

pub(crate) use framework::ProjectFramework;

pub fn run(args: &[String]) -> Result<()> {
    let cwd = env::current_dir()?;
    run_in_dir(args, &cwd)
}

pub fn run_in_dir(args: &[String], cwd: &Path) -> Result<()> {
    build::run(args, cwd)
}
