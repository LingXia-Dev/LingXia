use super::locate_templates_dir;
use super::template::process_template_dir;
use super::types::ProjectConfig;
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::fs;

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

    process_template_dir(&template_dir, &windows_dir, &vars)?;
    println!("  Created Windows host project: windows/");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn windows_host_takes_sdk_and_windows_rs_from_crates_io() {
        let sdk_manifest = include_str!("../../../../../crates/lingxia-windows-sdk/Cargo.toml");
        assert!(
            !sdk_manifest.contains("microsoft/windows-rs.git"),
            "windows-sdk must take windows-rs from crates.io"
        );

        let template = include_str!("../../../templates/windows/Cargo.toml.template");
        assert!(
            !template.contains("microsoft/windows-rs.git"),
            "generated Windows hosts must not git-pin windows-rs"
        );
        assert!(!template.contains("{{WINDOWS_RS_REV}}"));
        assert!(
            !template.contains("LingXia-Dev/LingXia.git"),
            "generated Windows hosts must take lingxia-windows-sdk from crates.io"
        );
        assert!(template.contains("version = \"{{LINGXIA_VERSION}}\""));
    }

    #[test]
    fn windows_registers_the_host_before_product_cli_parsing() {
        let source = include_str!("../../../templates/windows/src/main.rs");
        let register = source
            .find("host::lingxia_register_host_addon();")
            .expect("Windows entrypoint must register its host addon");
        let parse = source
            .find("host::run_cli_if_invoked()")
            .expect("Windows entrypoint must classify product CLI invocations");
        assert!(register < parse);
    }
}
