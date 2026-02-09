use super::{BuildArtifacts, BuildConfig, Device, InstallConfig, Platform, RunConfig};
use anyhow::Result;
use colored::Colorize;

mod build;
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
pub use credentials::AgcCredentialStorage;
pub use doctor::doctor_checks;
pub use project::{
    generate_icons, read_bundle_name, resolve_harmony_dir, resolve_harmony_rawfile_dir,
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
        let harmony_config = config
            .lingxia_config
            .as_ref()
            .and_then(|c| c.harmony.as_ref());
        let harmony_dir = resolve_harmony_dir(&config.project_root, harmony_config)?;

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
