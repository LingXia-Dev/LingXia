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
const LEGACY_NOTICE_MARKER: &str = ".legacy-noticed";

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
    pub fn harmony_signing_dir(&self, client_id: &str) -> Result<PathBuf> {
        Ok(self.harmony_identity_dir(client_id)?.join("signing"))
    }

    /// One-time notice for pre-wallet credential files, which are no longer
    /// read. Prints at most once per state root.
    pub fn notice_legacy_files(&self) {
        let legacy = [
            self.state_root.join("apple").join("credentials.json"),
            self.state_root.join("apple").join("developer-id.json"),
            self.state_root.join("harmony").join("agc_credentials.json"),
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
    fn masking() {
        assert_eq!(mask("michael@example.com"), "m***@example.com");
        assert_eq!(mask("ABC123DEF4"), "ABC1…F4");
        assert_eq!(mask("ab"), "…");
    }
}
