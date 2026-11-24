use anyhow::{Result, bail};
use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct TypesConfig {
    /// The lxapp directory, the one holding `lxapp.json`.
    #[arg(long, default_value = ".")]
    pub dir: PathBuf,
}

/// The `.lingxia/` types and native clients a build writes, for a fresh checkout or CI
/// type-checking before any build.
pub fn run(config: TypesConfig) -> Result<()> {
    if !config.dir.join("lxapp.json").is_file() {
        bail!(
            "{}: no lxapp.json; run in the lxapp directory or pass --dir",
            config.dir.display()
        );
    }
    let project = crate::lxapp::Project::discover(&config.dir)?;
    crate::lxapp::generate_types(&project)
}
