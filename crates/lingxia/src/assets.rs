//! Host assets packaged by the CLI.
//!
//! `assets:` in `lingxia.yaml` names a project directory; the CLI packages
//! it into every platform build through the platform's own asset pipeline
//! (APK assets, app bundle resources, HarmonyOS rawfile), and this module
//! reads it back by the same relative paths.
//!
//! Available once the runtime is initialized. The one launch-time consumer
//! that runs earlier — splash selection — reads the cover store instead;
//! acquisition work is the place to move a packaged cover into it.

use std::io::Read;

/// Read a host asset by its path relative to the project's `assets:`
/// directory.
pub fn read(path: &str) -> crate::Result<Vec<u8>> {
    use lingxia_platform::traits::app_runtime::AppRuntime;

    let platform = crate::runtime::platform()?;
    let mut reader = platform
        .read_asset(&format!("hostassets/{path}"))
        .map_err(|e| crate::Error::internal(format!("host asset '{path}': {e}")))?;
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|e| crate::Error::internal(format!("host asset '{path}': {e}")))?;
    Ok(bytes)
}
