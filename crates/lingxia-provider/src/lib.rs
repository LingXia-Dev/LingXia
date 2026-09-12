use std::future::Future;
use std::pin::Pin;

/// Boxed future type for dyn compatibility.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Error type for provider operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderErrorCode {
    InvalidRequest,
    NotFound,
    Network,
    Timeout,
    Server,
    PermissionDenied,
    Internal,
}

impl ProviderErrorCode {
    pub const fn biz_code(self) -> u32 {
        match self {
            Self::InvalidRequest => 1002,
            Self::NotFound => 1003,
            Self::Network => 5001,
            Self::Timeout => 5002,
            Self::Server => 5003,
            Self::PermissionDenied => 3000,
            Self::Internal => 1005,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::NotFound => "not_found",
            Self::Network => "network",
            Self::Timeout => "timeout",
            Self::Server => "server",
            Self::PermissionDenied => "permission_denied",
            Self::Internal => "internal",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderError {
    code: ProviderErrorCode,
    detail: String,
}

impl ProviderError {
    pub fn new(code: ProviderErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }

    pub fn invalid_request(detail: impl Into<String>) -> Self {
        Self::new(ProviderErrorCode::InvalidRequest, detail)
    }

    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::new(ProviderErrorCode::NotFound, detail)
    }

    pub fn network(detail: impl Into<String>) -> Self {
        Self::new(ProviderErrorCode::Network, detail)
    }

    pub fn timeout(detail: impl Into<String>) -> Self {
        Self::new(ProviderErrorCode::Timeout, detail)
    }

    pub fn server(detail: impl Into<String>) -> Self {
        Self::new(ProviderErrorCode::Server, detail)
    }

    pub fn permission_denied(detail: impl Into<String>) -> Self {
        Self::new(ProviderErrorCode::PermissionDenied, detail)
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new(ProviderErrorCode::Internal, detail)
    }

    pub const fn code(&self) -> ProviderErrorCode {
        self.code
    }

    pub const fn biz_code(&self) -> u32 {
        self.code.biz_code()
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code.as_str(), self.detail)
    }
}

impl std::error::Error for ProviderError {}

/// Error type for fingerprint operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FingerprintError {
    /// Device ID cannot be loaded/generated on current runtime.
    DeviceIdUnavailable,
}

impl std::fmt::Display for FingerprintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DeviceIdUnavailable => write!(f, "device_id_unavailable"),
        }
    }
}

impl std::error::Error for FingerprintError {}

/// Trait for device fingerprint.
pub trait FingerprintProvider: Send + Sync + 'static {
    /// Get the device fingerprint ID.
    fn get_fingerprint(&self) -> Result<String, FingerprintError> {
        Err(FingerprintError::DeviceIdUnavailable)
    }
}

/// Trait for push token binding.
pub trait PushNotificationProvider: Send + Sync + 'static {
    /// Bind push token to cloud side.
    fn bind_push_token<'a>(&'a self, _token: String) -> BoxFuture<'a, Result<(), ProviderError>> {
        Box::pin(async { Ok(()) })
    }
}

/// Server-owned lifecycle state of an lxapp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LxAppStatus {
    /// The registry reported nothing for this app — an older server, an app it
    /// does not know, or a check that never reached it.
    #[default]
    Unknown,
    Published,
    /// Temporarily unavailable while the operator works on it. Must not open,
    /// but says something different to the user than `Suspended` does: one is
    /// "come back later", the other is "this is not yours to open".
    Maintain,
    /// No longer offered. An already-installed copy keeps working.
    Delisted,
    /// Blocked by the operator. Must not open, installed or not.
    Suspended,
}

impl LxAppStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Published => "published",
            Self::Maintain => "maintain",
            Self::Delisted => "delisted",
            Self::Suspended => "suspended",
        }
    }

    /// Unrecognized values read as `Unknown` so a newer server cannot brick an
    /// older client by inventing a state it never blocks on.
    ///
    /// Case- and whitespace-insensitive: `Unknown` does not block, so a server
    /// sending `"Suspended"` against a case-sensitive match would degrade in
    /// the unsafe direction on the one field that gates opening.
    pub fn from_str_lossy(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "published" => Self::Published,
            "maintain" => Self::Maintain,
            "delisted" => Self::Delisted,
            "suspended" => Self::Suspended,
            _ => Self::Unknown,
        }
    }

    /// Whether opening must be refused.
    ///
    /// `Delisted` does not: it means the app is no longer offered, while an
    /// installed copy keeps working. `Maintain` does, because the operator has
    /// taken it down on purpose and a half-working app is worse than a clear
    /// message.
    pub const fn blocks_open(self) -> bool {
        matches!(self, Self::Suspended | Self::Maintain)
    }
}

impl std::fmt::Display for LxAppStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The registry's record for one lxapp: the facts the server owns.
///
/// Deliberately carries nothing about a *package* — version, url, checksum,
/// `minRuntimeVersion` all belong to `UpdatePackageInfo` and travel the update
/// path. Server-owned facts in, package facts out; the two must never become
/// two answers to the same question.
/// The request names the app; this describes it. Fill in what the registry
/// knows and leave the rest — `..Default::default()` covers it.
#[derive(Debug, Clone, Default)]
pub struct LxAppRegistryInfo {
    /// Display name as the backend defined it.
    pub name: Option<String>,
    pub description: Option<String>,
    /// Where the icon lives. Also the cache key: the client re-fetches when
    /// this changes and not otherwise, so a server that edits the artwork
    /// behind a stable URL will never be picked up. Change the URL — a content
    /// path, or a version query — when the image changes.
    pub icon_url: Option<String>,
    pub status: LxAppStatus,
    /// What this app may reach and do. `None` is the answer for every app the
    /// registry has no policy for: public network and every privilege class.
    /// Only an explicit `Some` restricts anything.
    pub permissions: Option<LxAppPermissions>,
}

/// Grant for one app. Built through constructors, not a struct literal.
///
/// Each field is independent. `None` means the registry does not constrain that
/// half (public network / every privilege class). `Some` is an allowlist: empty
/// denies that half, `["*"]` allows it all. Fill in only the half you have a
/// policy for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct LxAppPermissions {
    pub network: Option<LxAppNetworkPermission>,
    pub privileges: Option<LxAppPrivilegePermission>,
}

impl LxAppPermissions {
    /// Unconstrained: public network and every privilege class.
    pub fn all() -> Self {
        Self::default()
    }

    /// Restrict network to these hosts. Privileges stay unconstrained.
    pub fn network(trusted_domains: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            network: Some(LxAppNetworkPermission::new(trusted_domains)),
            privileges: None,
        }
    }

    /// Restrict privileges to these classes. Network stays unconstrained.
    pub fn privileges(granted: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            network: None,
            privileges: Some(LxAppPrivilegePermission::new(granted)),
        }
    }

    pub fn with_network(
        mut self,
        trusted_domains: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.network = Some(LxAppNetworkPermission::new(trusted_domains));
        self
    }

    pub fn with_privileges(mut self, granted: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.privileges = Some(LxAppPrivilegePermission::new(granted));
        self
    }
}

/// Approved hosts, without scheme, port or path. `*` allows every public host;
/// `*.example.com` matches subdomains. The runtime still blocks non-public
/// addresses outside a dev session.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct LxAppNetworkPermission {
    pub trusted_domains: Vec<String>,
}

impl LxAppNetworkPermission {
    pub fn new(trusted_domains: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            trusted_domains: trusted_domains.into_iter().map(Into::into).collect(),
        }
    }
}

/// Approved privilege ids (`downloads`, `process`, `automation`, `host`, …).
/// `*` allows every class.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct LxAppPrivilegePermission {
    pub granted: Vec<String>,
}

impl LxAppPrivilegePermission {
    pub fn new(granted: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            granted: granted.into_iter().map(Into::into).collect(),
        }
    }
}

/// Lookup of registry records, separate from `UpdateProvider` on purpose: an
/// app's name, icon, status, and permissions change without any package
/// changing, and the update path is gated (OTA-managed only, deduped,
/// force-update aware) in ways that would silently strand them.
///
/// One record answers what the app *is* and what it *may do*, because both are
/// facts the server owns about an app id and both are wanted at the same
/// moment — opening it. A second round trip for permissions alone would buy
/// nothing.
pub trait LxAppRegistryProvider: Send + Sync + 'static {
    /// Resolve one app's registry record.
    ///
    /// `name` and `description` are the strings the backend stored. The client
    /// does not send a locale; localization, if any, is a server concern.
    ///
    /// `Ok(None)` means the registry does not know the app (HTTP 404). That is
    /// a negative listing, not a transport failure, and it does not restrict
    /// the app — nothing but an explicit `permissions` grant does. An error or
    /// a timeout leaves the app on its last known grant, or unrestricted if it
    /// never had one; an unreachable registry is not evidence of anything.
    ///
    /// Scope remote lookups to the host account/tenant; an app id alone is not
    /// an authorization credential.
    fn fetch_registry_info<'a>(
        &'a self,
        _app: LxAppRegistryRequest<'a>,
    ) -> BoxFuture<'a, Result<Option<LxAppRegistryInfo>, ProviderError>> {
        Box::pin(async { Ok(None) })
    }
}

/// Which app the runtime is asking about. Later inputs arrive here rather than
/// as a new method, so match on the fields you need.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct LxAppRegistryRequest<'a> {
    pub appid: &'a str,
    /// Answer per channel: a draft of an app id is not the app the
    /// release grant was written for.
    pub channel: LxAppChannel,
}

impl<'a> LxAppRegistryRequest<'a> {
    pub fn new(appid: &'a str, channel: LxAppChannel) -> Self {
        Self { appid, channel }
    }
}

/// The channel a build was published on.
///
/// An enum rather than the wire string: an implementor that misspells a channel
/// in a `match` would answer "not my app", which restricts nothing — a typo
/// must not widen an app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LxAppChannel {
    #[default]
    Release,
    Preview,
    Draft,
}

impl LxAppChannel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Preview => "preview",
            Self::Draft => "draft",
        }
    }
}

impl std::fmt::Display for LxAppChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod registry_tests {
    use super::LxAppStatus;

    #[test]
    fn only_the_states_that_mean_do_not_open_block() {
        // Two states block, and they say different things to a user: one is
        // "come back later", the other is "this is not yours to open".
        assert!(LxAppStatus::Suspended.blocks_open());
        assert!(LxAppStatus::Maintain.blocks_open());
        // Delisted is not offered any more, but an installed copy keeps working.
        assert!(!LxAppStatus::Delisted.blocks_open());
        assert!(!LxAppStatus::Published.blocks_open());
        // An unrecognized state must never lock a user out.
        assert!(!LxAppStatus::Unknown.blocks_open());
        assert_eq!(
            LxAppStatus::from_str_lossy("maintain"),
            LxAppStatus::Maintain
        );
    }

    #[test]
    fn status_parsing_is_case_insensitive_because_unknown_never_blocks() {
        assert_eq!(
            LxAppStatus::from_str_lossy("suspended"),
            LxAppStatus::Suspended
        );
        assert_eq!(
            LxAppStatus::from_str_lossy("Suspended"),
            LxAppStatus::Suspended
        );
        assert_eq!(
            LxAppStatus::from_str_lossy(" SUSPENDED "),
            LxAppStatus::Suspended
        );
        assert!(LxAppStatus::from_str_lossy("SUSPENDED").blocks_open());

        assert_eq!(
            LxAppStatus::from_str_lossy("Delisted"),
            LxAppStatus::Delisted
        );
        assert_eq!(LxAppStatus::from_str_lossy(""), LxAppStatus::Unknown);
        assert_eq!(LxAppStatus::from_str_lossy("retired"), LxAppStatus::Unknown);
    }
}
