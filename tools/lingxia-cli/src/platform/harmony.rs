use super::{BuildArtifacts, BuildConfig, Device, InstallConfig, Platform, RunConfig};
use anyhow::Result;
use colored::Colorize;

mod build;
mod capabilities;
mod deploy;
mod doctor;
mod project;

mod agc;
mod auth_api;
mod credentials;
pub mod keygen;
pub mod provisioning;
pub mod signer;

pub use agc::{AgcApiCredentials, AgcConnectClient, AgcToken};
pub use auth_api::HarmonyAuthService;
pub use capabilities::resolve_effective_acl_permissions;
pub use credentials::AgcCredentialStorage;
pub use doctor::doctor_checks;
pub use project::{
    generate_icons, read_bundle_name, resolve_harmony_dir, resolve_harmony_rawfile_dir,
    sync_acl_permissions, sync_app_links,
};
pub use provisioning::{ProvisioningManager, SigningMode};
pub use signer::{HarmonySigner, SigningConfig};

pub(crate) const OHOS_TARGET: &str = "aarch64-unknown-linux-ohos";
const DEFAULT_ABILITY_NAME: &str = "EntryAbility";

pub struct HarmonyPlatform;

impl HarmonyPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl Platform for HarmonyPlatform {
    fn build(&self, config: &BuildConfig) -> Result<BuildArtifacts> {
        let lingxia_config = config.lingxia_config.as_ref();
        let harmony_config = lingxia_config.and_then(|c| c.harmony.as_ref());
        let harmony_dir = resolve_harmony_dir(&config.project_root, harmony_config)?;
        let package_name = read_bundle_name(&harmony_dir)?;
        let resolution = resolve_effective_acl_permissions(&package_name);
        let effective_acl_permissions = resolution.effective_permissions;

        if !resolution.missing_permissions.is_empty() {
            eprintln!(
                "{} Harmony restricted ACL permissions not granted for `{}`: {}",
                "Warning:".yellow(),
                package_name,
                resolution.missing_permissions.join(", ")
            );
        }

        if resolution.can_sync_managed_permissions {
            if sync_acl_permissions(&harmony_dir, &effective_acl_permissions)? {
                println!(
                    "{} Synced Harmony ACL permissions to module.json5",
                    "[Harmony]".cyan()
                );
            }
        } else {
            eprintln!(
                "{} Skip syncing managed Harmony ACL permissions because approvals are not verified.",
                "Warning:".yellow()
            );
        }

        let app_link_hosts = lingxia_config
            .and_then(|config| config.app_links.as_ref())
            .map(|app_links| app_links.hosts.as_slice())
            .unwrap_or(&[]);
        if sync_app_links(&harmony_dir, app_link_hosts)? {
            println!(
                "{} Synced Harmony AppLinks to module.json5",
                "[Harmony]".cyan()
            );
        }

        println!(
            "{} Building HarmonyOS app from {}",
            "[Harmony]".cyan(),
            harmony_dir.display()
        );

        self.build_impl(config, &harmony_dir)
    }

    fn install(&self, config: &InstallConfig) -> Result<()> {
        self.install_impl(config)
    }

    fn uninstall(&self, package_id: &str, device_id: Option<&str>) -> Result<()> {
        self.uninstall_impl(package_id, device_id)
    }

    fn run(&self, config: &RunConfig) -> Result<()> {
        self.run_impl(config)
    }

    fn list_devices(&self) -> Result<Vec<Device>> {
        self.list_devices_impl()
    }
}
