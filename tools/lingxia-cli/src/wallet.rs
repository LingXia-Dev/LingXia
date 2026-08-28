//! Identity-keyed credential wallet under `<state root>/credentials/`.
//!
//! Credentials are stored per verified provider identity (Apple: Team ID;
//! Harmony: AGC client id), one file per mechanism — no aliases, no global
//! "current account". Legacy single-slot files are never read; re-login is
//! the migration path.

use anyhow::{Context, Result, anyhow, bail};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::platform::apple::auth::{AuthCredentials, DeveloperIdCredentials};
use crate::platform::harmony::AgcApiCredentials;

const ASC_FILE: &str = "asc.json";
const APPLE_ID_FILE: &str = "apple-id.json";
const DEVELOPER_ID_FILE: &str = "developer-id.json";
const AGC_FILE: &str = "agc.json";
const STORE_SLOT_FILE: &str = "creds.json";
const LEGACY_NOTICE_MARKER: &str = ".legacy-noticed";

/// On-disk store slot: the provider credentials plus the display identity
/// (the directory name may be a hash when the identity is not path-safe).
#[derive(serde::Serialize)]
struct StoreSlot<'a, T: serde::Serialize> {
    identity: String,
    #[serde(flatten)]
    creds: &'a T,
}

#[derive(serde::Deserialize)]
struct StoreSlotIdentity {
    identity: String,
}

#[derive(serde::Deserialize)]
#[serde(bound = "T: serde::de::DeserializeOwned")]
struct StoreSlotOwned<T> {
    #[allow(dead_code)]
    identity: String,
    #[serde(flatten)]
    creds: T,
}

/// Which Apple mechanism slots exist for one team.
#[derive(Debug, Clone)]
pub struct AppleTeamSlots {
    pub team_id: String,
    pub has_asc: bool,
    pub has_apple_id: bool,
    pub has_developer_id: bool,
}

impl AppleTeamSlots {
    pub fn has_auth(&self) -> bool {
        self.has_asc || self.has_apple_id
    }

    /// Short mechanism summary for candidate listings, e.g. `ASC key, Apple ID`.
    pub fn mechanisms(&self) -> String {
        let mut parts = Vec::new();
        if self.has_asc {
            parts.push("ASC key");
        }
        if self.has_apple_id {
            parts.push("Apple ID");
        }
        if self.has_developer_id {
            parts.push("Developer ID");
        }
        parts.join(", ")
    }
}

pub struct Wallet {
    root: PathBuf,
    state_root: PathBuf,
}

impl Wallet {
    pub fn open() -> Result<Self> {
        Ok(Self::at(crate::state_root::lingxia_dir()?))
    }

    pub fn at(state_root: impl Into<PathBuf>) -> Self {
        let state_root = state_root.into();
        Self {
            root: state_root.join("credentials"),
            state_root,
        }
    }

    fn apple_root(&self) -> PathBuf {
        self.root.join("apple")
    }

    fn apple_team_dir(&self, team_id: &str) -> Result<PathBuf> {
        validate_path_component(team_id)?;
        Ok(self.apple_root().join(team_id))
    }

    /// All Apple teams present in the wallet, sorted by team id.
    pub fn apple_teams(&self) -> Result<Vec<AppleTeamSlots>> {
        let root = self.apple_root();
        let mut teams = Vec::new();
        let entries = match fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(teams),
            Err(e) => return Err(e).with_context(|| format!("read {}", root.display())),
        };
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let Some(team_id) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if validate_path_component(&team_id).is_err() {
                continue;
            }
            let dir = entry.path();
            let slots = AppleTeamSlots {
                team_id,
                has_asc: dir.join(ASC_FILE).is_file(),
                has_apple_id: dir.join(APPLE_ID_FILE).is_file(),
                has_developer_id: dir.join(DEVELOPER_ID_FILE).is_file(),
            };
            if slots.has_asc || slots.has_apple_id || slots.has_developer_id {
                teams.push(slots);
            }
        }
        teams.sort_by(|a, b| a.team_id.cmp(&b.team_id));
        Ok(teams)
    }

    pub fn apple_team(&self, team_id: &str) -> Result<Option<AppleTeamSlots>> {
        let dir = self.apple_team_dir(team_id)?;
        if !dir.is_dir() {
            return Ok(None);
        }
        let slots = AppleTeamSlots {
            team_id: team_id.to_string(),
            has_asc: dir.join(ASC_FILE).is_file(),
            has_apple_id: dir.join(APPLE_ID_FILE).is_file(),
            has_developer_id: dir.join(DEVELOPER_ID_FILE).is_file(),
        };
        if slots.has_asc || slots.has_apple_id || slots.has_developer_id {
            Ok(Some(slots))
        } else {
            Ok(None)
        }
    }

    /// Load the auth slot for `team_id`, ASC first (it has strictly more
    /// capability than an Apple ID session).
    pub fn load_apple_auth(&self, team_id: &str) -> Result<Option<AuthCredentials>> {
        if let Some(asc) = self.load_apple_asc(team_id)? {
            return Ok(Some(asc));
        }
        self.load_apple_slot(team_id, APPLE_ID_FILE)
    }

    pub fn load_apple_asc(&self, team_id: &str) -> Result<Option<AuthCredentials>> {
        self.load_apple_slot(team_id, ASC_FILE)
    }

    pub fn load_apple_id(&self, team_id: &str) -> Result<Option<AuthCredentials>> {
        self.load_apple_slot(team_id, APPLE_ID_FILE)
    }

    fn load_apple_slot(&self, team_id: &str, file: &str) -> Result<Option<AuthCredentials>> {
        let path = self.apple_team_dir(team_id)?.join(file);
        if !path.is_file() {
            return Ok(None);
        }
        let content =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let creds = serde_json::from_str(&content).with_context(|| {
            format!(
                "parse {}. Re-run `lingxia auth login apple` to refresh it.",
                path.display()
            )
        })?;
        Ok(Some(creds))
    }

    /// Save an auth credential into its mechanism slot; the team comes from
    /// the credential itself. Returns the written path.
    pub fn save_apple_auth(&self, creds: &AuthCredentials) -> Result<PathBuf> {
        let file = match creds {
            AuthCredentials::AppStoreConnect { .. } => ASC_FILE,
            AuthCredentials::AppleId { .. } => APPLE_ID_FILE,
        };
        let path = self.apple_team_dir(creds.team_id())?.join(file);
        write_secret(&path, serde_json::to_string_pretty(creds)?.as_bytes())?;
        Ok(path)
    }

    pub fn load_apple_developer_id(&self, team_id: &str) -> Result<Option<DeveloperIdCredentials>> {
        let path = self.apple_team_dir(team_id)?.join(DEVELOPER_ID_FILE);
        if !path.is_file() {
            return Ok(None);
        }
        let content =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let creds = serde_json::from_str(&content).with_context(|| {
            format!(
                "parse {}. Re-run `lingxia auth login apple --mode developer-id` to refresh it.",
                path.display()
            )
        })?;
        Ok(Some(creds))
    }

    pub fn save_apple_developer_id(
        &self,
        team_id: &str,
        creds: &DeveloperIdCredentials,
    ) -> Result<PathBuf> {
        let path = self.apple_team_dir(team_id)?.join(DEVELOPER_ID_FILE);
        write_secret(&path, serde_json::to_string_pretty(creds)?.as_bytes())?;
        Ok(path)
    }

    /// Remove every slot of one Apple team. Returns whether anything existed.
    pub fn delete_apple_team(&self, team_id: &str) -> Result<bool> {
        let dir = self.apple_team_dir(team_id)?;
        if !dir.exists() {
            return Ok(false);
        }
        fs::remove_dir_all(&dir).with_context(|| format!("remove {}", dir.display()))?;
        Ok(true)
    }

    fn harmony_identity_dir(&self, client_id: &str) -> Result<PathBuf> {
        validate_path_component(client_id)?;
        Ok(self.root.join("harmony").join(client_id))
    }

    /// All Harmony AGC identities (client ids) in the wallet, sorted.
    pub fn harmony_identities(&self) -> Result<Vec<String>> {
        let root = self.root.join("harmony");
        let mut identities = Vec::new();
        let entries = match fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(identities),
            Err(e) => return Err(e).with_context(|| format!("read {}", root.display())),
        };
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let Some(client_id) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if validate_path_component(&client_id).is_ok() && entry.path().join(AGC_FILE).is_file()
            {
                identities.push(client_id);
            }
        }
        identities.sort();
        Ok(identities)
    }

    pub fn load_harmony_agc(&self, client_id: &str) -> Result<Option<AgcApiCredentials>> {
        let path = self.harmony_identity_dir(client_id)?.join(AGC_FILE);
        if !path.is_file() {
            return Ok(None);
        }
        let content =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let creds = serde_json::from_str(&content).with_context(|| {
            format!(
                "parse {}. Re-run `lingxia auth login harmony` to refresh it.",
                path.display()
            )
        })?;
        Ok(Some(creds))
    }

    /// Save AGC credentials into their identity slot (keyed by `client_id`
    /// from the credentials). Returns the written path.
    pub fn save_harmony_agc(&self, creds: &AgcApiCredentials) -> Result<PathBuf> {
        let path = self.harmony_identity_dir(&creds.client_id)?.join(AGC_FILE);
        write_secret(&path, serde_json::to_string_pretty(creds)?.as_bytes())?;
        Ok(path)
    }

    /// Remove one Harmony identity (credentials and its signing material).
    pub fn delete_harmony_identity(&self, client_id: &str) -> Result<bool> {
        let dir = self.harmony_identity_dir(client_id)?;
        if !dir.exists() {
            return Ok(false);
        }
        fs::remove_dir_all(&dir).with_context(|| format!("remove {}", dir.display()))?;
        Ok(true)
    }

    /// Signing material (keys, certs, profiles, keystores) lives next to the
    /// identity that minted it, so organizations never share certificates.
    #[cfg(not(target_os = "windows"))]
    pub fn harmony_signing_dir(&self, client_id: &str) -> Result<PathBuf> {
        Ok(self.harmony_identity_dir(client_id)?.join("signing"))
    }

    fn store_provider_root(&self, provider: &str) -> Result<PathBuf> {
        validate_path_component(provider)?;
        Ok(self.root.join("stores").join(provider))
    }

    fn store_identity_dir(&self, provider: &str, identity: &str) -> Result<PathBuf> {
        // Identities that are not path-safe (e.g. service-account emails) get
        // a short hash directory; the real identity lives inside the slot.
        let component = match validate_path_component(identity) {
            Ok(()) => identity.to_string(),
            Err(_) if !identity.is_empty() => {
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(identity.as_bytes());
                hasher.finalize()[..8]
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect()
            }
            Err(e) => return Err(e),
        };
        Ok(self.store_provider_root(provider)?.join(component))
    }

    /// All identities stored for one OS-store provider, sorted.
    pub fn store_identities(&self, provider: &str) -> Result<Vec<String>> {
        let root = self.store_provider_root(provider)?;
        let mut identities = Vec::new();
        let entries = match fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(identities),
            Err(e) => return Err(e).with_context(|| format!("read {}", root.display())),
        };
        for entry in entries {
            let path = entry?.path().join(STORE_SLOT_FILE);
            if !path.is_file() {
                continue;
            }
            let content =
                fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
            if let Ok(slot) = serde_json::from_str::<StoreSlotIdentity>(&content) {
                identities.push(slot.identity);
            }
        }
        identities.sort();
        Ok(identities)
    }

    pub fn load_store_creds<T: serde::de::DeserializeOwned>(
        &self,
        provider: &str,
        identity: &str,
    ) -> Result<Option<T>> {
        let path = self
            .store_identity_dir(provider, identity)?
            .join(STORE_SLOT_FILE);
        if !path.is_file() {
            return Ok(None);
        }
        let content =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let slot: StoreSlotOwned<T> = serde_json::from_str(&content).with_context(|| {
            format!(
                "parse {}. Re-run `lingxia auth login {provider}` to refresh it.",
                path.display()
            )
        })?;
        Ok(Some(slot.creds))
    }

    pub fn save_store_creds<T: serde::Serialize>(
        &self,
        provider: &str,
        identity: &str,
        creds: &T,
    ) -> Result<PathBuf> {
        let path = self
            .store_identity_dir(provider, identity)?
            .join(STORE_SLOT_FILE);
        let slot = StoreSlot {
            identity: identity.to_string(),
            creds,
        };
        write_secret(&path, serde_json::to_string_pretty(&slot)?.as_bytes())?;
        Ok(path)
    }

    pub fn delete_store_identity(&self, provider: &str, identity: &str) -> Result<bool> {
        let dir = self.store_identity_dir(provider, identity)?;
        if !dir.exists() {
            return Ok(false);
        }
        fs::remove_dir_all(&dir).with_context(|| format!("remove {}", dir.display()))?;
        Ok(true)
    }

    fn publish_env_path(&self, canonical_server: &str, env: &str) -> Result<PathBuf> {
        validate_path_component(env)?;
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(canonical_server.as_bytes());
        let hash: String = hasher.finalize()[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Ok(self
            .root
            .join("lingxia")
            .join(hash)
            .join(format!("{env}.json")))
    }

    /// Persist a publish token for `(canonical server, env)`; saving the same
    /// key again is a token rotation (atomic replace).
    pub fn save_publish_token(
        &self,
        canonical_server: &str,
        env: &str,
        token: &str,
    ) -> Result<PathBuf> {
        let path = self.publish_env_path(canonical_server, env)?;
        let slot = serde_json::json!({ "server": canonical_server, "token": token });
        write_secret(&path, serde_json::to_string_pretty(&slot)?.as_bytes())?;
        Ok(path)
    }

    pub fn load_publish_token(&self, canonical_server: &str, env: &str) -> Result<Option<String>> {
        let path = self.publish_env_path(canonical_server, env)?;
        if !path.is_file() {
            return Ok(None);
        }
        let content =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let slot: serde_json::Value =
            serde_json::from_str(&content).with_context(|| format!("parse {}", path.display()))?;
        Ok(slot
            .get("token")
            .and_then(|v| v.as_str())
            .map(str::to_string))
    }

    pub fn delete_publish_token(&self, canonical_server: &str, env: &str) -> Result<bool> {
        let path = self.publish_env_path(canonical_server, env)?;
        if !path.exists() {
            return Ok(false);
        }
        fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
        Ok(true)
    }

    /// All stored publish tokens as `(server, env)` pairs, for status output.
    pub fn publish_entries(&self) -> Result<Vec<(String, String)>> {
        let root = self.root.join("lingxia");
        let mut entries = Vec::new();
        let dirs = match fs::read_dir(&root) {
            Ok(dirs) => dirs,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(entries),
            Err(e) => return Err(e).with_context(|| format!("read {}", root.display())),
        };
        for dir in dirs {
            let dir = dir?.path();
            if !dir.is_dir() {
                continue;
            }
            for file in fs::read_dir(&dir)? {
                let path = file?.path();
                let Some(env) = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(str::to_string)
                else {
                    continue;
                };
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let Ok(content) = fs::read_to_string(&path) else {
                    continue;
                };
                if let Some(server) = serde_json::from_str::<serde_json::Value>(&content)
                    .ok()
                    .and_then(|v| v.get("server").and_then(|s| s.as_str()).map(str::to_string))
                {
                    entries.push((server, env));
                }
            }
        }
        entries.sort();
        Ok(entries)
    }

    /// One-time notice for pre-wallet credential files, which are no longer
    /// read. Prints at most once per state root.
    pub fn notice_legacy_files(&self) {
        let legacy = [
            self.state_root.join("apple").join("credentials.json"),
            self.state_root.join("apple").join("developer-id.json"),
            self.state_root.join("harmony").join("agc_credentials.json"),
            self.state_root.join("store").join("credentials.toml"),
        ];
        let present: Vec<&Path> = legacy
            .iter()
            .map(PathBuf::as_path)
            .filter(|p| p.is_file())
            .collect();
        if present.is_empty() {
            return;
        }
        let marker = self.root.join(LEGACY_NOTICE_MARKER);
        if marker.exists() {
            return;
        }
        eprintln!(
            "The pre-wallet credential format is no longer read; you'll be asked to log in again."
        );
        eprintln!("Old files can be deleted:");
        for path in present {
            eprintln!("  {}", path.display());
        }
        if fs::create_dir_all(&self.root).is_ok() {
            let _ = fs::write(&marker, b"");
        }
    }
}

/// Reject identifiers that could escape the wallet directory.
fn validate_path_component(value: &str) -> Result<()> {
    if value.is_empty() || value.len() > 64 {
        bail!("invalid credential identifier: {value:?}");
    }
    if !value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("invalid credential identifier: {value:?}");
    }
    Ok(())
}

/// Atomic secret write: same-dir temp file, 0600, fsync, rename; parents 0700.
fn write_secret(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("no parent for {}", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
    }
    let mut tmp = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("create temp file in {}", parent.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tmp.as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Canonicalize a publish server URL for use as a wallet key: lowercase
/// scheme/host, default port stripped, trailing `/` trimmed, base path kept.
/// Userinfo, query, and fragment are rejected.
pub fn canonical_publish_server(server: &str) -> Result<String> {
    let url = url::Url::parse(server.trim())
        .with_context(|| format!("invalid publish server URL: {server}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        bail!("publish server must be http(s): {server}");
    }
    if !url.username().is_empty() || url.password().is_some() {
        bail!("publish server URL must not contain userinfo: {server}");
    }
    if url.query().is_some() || url.fragment().is_some() {
        bail!("publish server URL must not contain a query or fragment: {server}");
    }
    let host = url
        .host_str()
        .with_context(|| format!("publish server URL has no host: {server}"))?;
    let path = url.path().trim_end_matches('/');
    Ok(match url.port() {
        Some(port) => format!("{}://{host}:{port}{path}", url.scheme()),
        None => format!("{}://{host}{path}", url.scheme()),
    })
}

/// Masked display for a sensitive identifier, e.g. `AB12…F4` / `m***@example.com`.
pub fn mask(value: &str) -> String {
    if let Some((local, domain)) = value.split_once('@') {
        let head = local.chars().next().unwrap_or('*');
        return format!("{head}***@{domain}");
    }
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 4 {
        return "…".to_string();
    }
    let head: String = chars[..4].iter().collect();
    let tail: String = chars[chars.len() - 2..].iter().collect();
    format!("{head}…{tail}")
}

/// Stable full digest used to compare credential material without displaying
/// or persisting the secret itself.
pub fn credential_fingerprint<T: serde::Serialize>(value: &T) -> Result<String> {
    use sha2::{Digest, Sha256};

    let encoded = serde_json::to_vec(value).context("serialize credential fingerprint")?;
    Ok(Sha256::digest(encoded)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub fn display_fingerprint(fingerprint: &str) -> String {
    format!("sha256:{}", &fingerprint[..fingerprint.len().min(12)])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asc(team: &str) -> AuthCredentials {
        AuthCredentials::AppStoreConnect {
            key_id: "ABC123DEF4".into(),
            issuer_id: "12345678-1234-1234-1234-123456789012".into(),
            private_key_pem: "-----BEGIN PRIVATE KEY-----\nx\n-----END PRIVATE KEY-----".into(),
            team_id: team.into(),
            cached_signing_identity: None,
        }
    }

    fn apple_id(team: &str) -> AuthCredentials {
        AuthCredentials::AppleId {
            adsid: "adsid".into(),
            token: "t".into(),
            app_token: "at".into(),
            team_id: team.into(),
            expiry: chrono::Utc::now() + chrono::Duration::hours(1),
        }
    }

    #[test]
    fn roundtrip_and_slots() {
        let tmp = tempfile::tempdir().unwrap();
        let wallet = Wallet::at(tmp.path());

        wallet.save_apple_auth(&asc("TEAMAAAAAA")).unwrap();
        wallet.save_apple_auth(&apple_id("TEAMAAAAAA")).unwrap();
        wallet.save_apple_auth(&apple_id("TEAMBBBBBB")).unwrap();

        let teams = wallet.apple_teams().unwrap();
        assert_eq!(teams.len(), 2);
        assert!(teams[0].has_asc && teams[0].has_apple_id && !teams[0].has_developer_id);
        assert!(!teams[1].has_asc && teams[1].has_apple_id);

        // ASC preferred over Apple ID for the same team.
        let auth = wallet.load_apple_auth("TEAMAAAAAA").unwrap().unwrap();
        assert!(matches!(auth, AuthCredentials::AppStoreConnect { .. }));
        let auth = wallet.load_apple_id("TEAMAAAAAA").unwrap().unwrap();
        assert!(matches!(auth, AuthCredentials::AppleId { .. }));
        let auth = wallet.load_apple_auth("TEAMBBBBBB").unwrap().unwrap();
        assert!(matches!(auth, AuthCredentials::AppleId { .. }));
        assert!(wallet.load_apple_asc("TEAMBBBBBB").unwrap().is_none());

        assert!(wallet.delete_apple_team("TEAMAAAAAA").unwrap());
        assert!(wallet.apple_team("TEAMAAAAAA").unwrap().is_none());
    }

    #[test]
    fn same_slot_login_is_rotation() {
        let tmp = tempfile::tempdir().unwrap();
        let wallet = Wallet::at(tmp.path());
        wallet.save_apple_auth(&asc("TEAMAAAAAA")).unwrap();
        let mut rotated = asc("TEAMAAAAAA");
        if let AuthCredentials::AppStoreConnect { key_id, .. } = &mut rotated {
            *key_id = "NEWKEY1234".into();
        }
        wallet.save_apple_auth(&rotated).unwrap();
        let auth = wallet.load_apple_asc("TEAMAAAAAA").unwrap().unwrap();
        if let AuthCredentials::AppStoreConnect { key_id, .. } = auth {
            assert_eq!(key_id, "NEWKEY1234");
        }
        assert_eq!(wallet.apple_teams().unwrap().len(), 1);
    }

    #[test]
    fn rejects_path_escapes() {
        let tmp = tempfile::tempdir().unwrap();
        let wallet = Wallet::at(tmp.path());
        assert!(wallet.load_apple_auth("../evil").is_err());
        assert!(wallet.load_apple_auth("a/b").is_err());
        assert!(wallet.load_apple_auth("").is_err());
    }

    #[test]
    fn secret_files_are_0600() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let tmp = tempfile::tempdir().unwrap();
            let wallet = Wallet::at(tmp.path());
            let path = wallet.save_apple_auth(&asc("TEAMAAAAAA")).unwrap();
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
            let dir_mode = fs::metadata(path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(dir_mode, 0o700);
        }
    }

    #[test]
    fn canonical_publish_server_normalizes() {
        assert_eq!(
            canonical_publish_server("HTTPS://LX.Example.com:443/base/").unwrap(),
            "https://lx.example.com/base"
        );
        assert_eq!(
            canonical_publish_server("http://localhost:8080").unwrap(),
            "http://localhost:8080"
        );
        assert!(canonical_publish_server("https://u:p@x.com").is_err());
        assert!(canonical_publish_server("https://x.com/a?b=1").is_err());
        assert!(canonical_publish_server("https://x.com/#frag").is_err());
        assert!(canonical_publish_server("ftp://x.com").is_err());
    }

    #[test]
    fn publish_token_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let wallet = Wallet::at(tmp.path());
        let server = "https://lx.example.com/base";

        assert!(
            wallet
                .load_publish_token(server, "release")
                .unwrap()
                .is_none()
        );
        let path = wallet
            .save_publish_token(server, "release", "tok1")
            .unwrap();
        assert!(path.starts_with(tmp.path().join("credentials").join("lingxia")));
        assert_eq!(
            wallet
                .load_publish_token(server, "release")
                .unwrap()
                .as_deref(),
            Some("tok1")
        );

        // Same key again is a rotation.
        wallet
            .save_publish_token(server, "release", "tok2")
            .unwrap();
        assert_eq!(
            wallet
                .load_publish_token(server, "release")
                .unwrap()
                .as_deref(),
            Some("tok2")
        );

        // Envs and servers are independent keys.
        wallet
            .save_publish_token(server, "developer", "dev")
            .unwrap();
        wallet
            .save_publish_token("https://other.example.com", "release", "o")
            .unwrap();
        assert_eq!(wallet.publish_entries().unwrap().len(), 3);

        assert!(wallet.delete_publish_token(server, "release").unwrap());
        assert!(
            wallet
                .load_publish_token(server, "release")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn masking() {
        assert_eq!(mask("michael@example.com"), "m***@example.com");
        assert_eq!(mask("ABC123DEF4"), "ABC1…F4");
        assert_eq!(mask("ab"), "…");
    }

    #[test]
    fn fingerprints_do_not_collapse_values_with_the_same_mask() {
        let left = "ABCD-one-EF";
        let right = "ABCD-two-EF";
        assert_eq!(mask(left), mask(right));
        assert_ne!(
            credential_fingerprint(&left).unwrap(),
            credential_fingerprint(&right).unwrap()
        );
    }
}
