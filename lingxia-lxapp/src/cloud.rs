//! Cloud provider traits and registration.

mod update;

pub use update::{
    BoxFuture, CloudError, CloudProvider, CloudUpdateProvider, NoOpCloudProvider,
    UpdateCheckResult, UpdatePackageInfo, register_cloud_provider,
};

pub(crate) use update::get_cloud_provider;
