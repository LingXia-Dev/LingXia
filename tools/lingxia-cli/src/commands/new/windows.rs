use super::locate_templates_dir;
use super::template::process_template_dir;
use super::types::ProjectConfig;
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::fs;

pub(super) const WINDOWS_RS_REV: &str = "a1e9fce43c026221f62f0a149267cb6d7d3c607b";

pub(super) fn create_windows_project(config: &ProjectConfig) -> Result<()> {
    let windows_dir = config.target_dir.join("windows");
    fs::create_dir_all(&windows_dir)?;

    let templates_base = locate_templates_dir()?;
    let template_dir = templates_base.join("windows");
    if !template_dir.exists() {
        return Err(anyhow!(
            "Windows template not found at: {}",
            template_dir.display()
        ));
    }

    // The native lib crate's package name and directory are both `native`.
    let host_crate_name = super::RUST_LIB_DIR_NAME.to_string();
    let windows_crate_name = format!("{}-windows", config.name);

    let mut vars = HashMap::new();
    vars.insert("PROJECT_NAME".to_string(), config.name.clone());
    vars.insert("HOST_CRATE_NAME".to_string(), host_crate_name);
    vars.insert("WINDOWS_CRATE_NAME".to_string(), windows_crate_name);
    vars.insert("WINDOWS_EXECUTABLE_NAME".to_string(), config.name.clone());
    vars.insert(
        "LINGXIA_VERSION".to_string(),
        crate::versions::cargo_compat_req(),
    );
    vars.insert(
        "LINGXIA_WINDOWS_SDK_GIT_REF".to_string(),
        crate::versions::windows_sdk_git_ref(),
    );
    vars.insert("WINDOWS_RS_REV".to_string(), WINDOWS_RS_REV.to_string());

    process_template_dir(&template_dir, &windows_dir, &vars)?;
    println!("  Created Windows host project: windows/");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::WINDOWS_RS_REV;

    #[test]
    fn windows_sdk_git_ref_is_valid_inline_table_fragment() {
        let fragment = crate::versions::windows_sdk_git_ref();
        assert!(
            fragment.starts_with("rev = \"") || fragment.starts_with("tag = \"lingxia-crates-v"),
            "{fragment}"
        );
    }

    #[test]
    fn windows_rs_patch_matches_the_sdk_revision() {
        let sdk_manifest = include_str!("../../../../../crates/lingxia-windows-sdk/Cargo.toml");
        let expected = format!("rev = \"{WINDOWS_RS_REV}\"");
        assert!(sdk_manifest.contains(&expected));

        let template = include_str!("../../../templates/windows/Cargo.toml.template");
        let patch_lines = template
            .lines()
            .filter(|line| line.contains("microsoft/windows-rs.git"))
            .collect::<Vec<_>>();
        assert!(!patch_lines.is_empty());
        assert!(
            patch_lines
                .iter()
                .all(|line| line.contains("{{WINDOWS_RS_REV}}"))
        );
    }
}
