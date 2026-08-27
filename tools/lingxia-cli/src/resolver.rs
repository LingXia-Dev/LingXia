//! Automatic credential resolution: project constraints → CI env → binding
//! cache → wallet, asking the user only when nothing can be derived safely.
//!
//! Stable error codes prefix every failure so scripts can match on them; the
//! human text always ends in the command that fixes the situation.

use anyhow::{Context, Result, anyhow, bail};
use colored::Colorize;
use std::io::IsTerminal;
use std::path::PathBuf;

use crate::binding::BindingStore;
use crate::config::LingXiaConfig;
use crate::platform::apple::auth::AuthCredentials;
use crate::platform::harmony::AgcApiCredentials;
use crate::wallet::{AppleTeamSlots, Wallet};

pub mod codes {
    pub const CREDENTIALS_MISSING: &str = "CREDENTIALS_MISSING";
    pub const CREDENTIAL_CAPABILITY_MISSING: &str = "CREDENTIAL_CAPABILITY_MISSING";
    pub const CREDENTIAL_SELECTION_REQUIRED: &str = "CREDENTIAL_SELECTION_REQUIRED";
    pub const CREDENTIAL_IDENTITY_MISMATCH: &str = "CREDENTIAL_IDENTITY_MISMATCH";
    pub const CREDENTIAL_ENV_INCOMPLETE: &str = "CREDENTIAL_ENV_INCOMPLETE";
}

const ENV_KEY_PATH: &str = "LINGXIA_APPLE_KEY_PATH";
const ENV_KEY_ID: &str = "LINGXIA_APPLE_KEY_ID";
const ENV_ISSUER_ID: &str = "LINGXIA_APPLE_ISSUER_ID";
const ENV_TEAM_ID: &str = "LINGXIA_APPLE_TEAM_ID";
/// Retired spellings that are no longer read; hitting them must explain the
/// canonical name instead of silently ignoring the value.
const LEGACY_ENV: &[(&str, &str)] = &[
    ("LINGXIA_ASC_KEY_PATH", ENV_KEY_PATH),
    ("LINGXIA_ASC_KEY_ID", ENV_KEY_ID),
    ("LINGXIA_ASC_ISSUER_ID", ENV_ISSUER_ID),
    ("LINGXIA_APPLE_NOTARY_KEY", ENV_KEY_PATH),
    ("LINGXIA_APPLE_NOTARY_KEY_ID", ENV_KEY_ID),
    ("LINGXIA_APPLE_NOTARY_ISSUER_ID", ENV_ISSUER_ID),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AppleChannel {
    Ios,
    Macos,
}

impl AppleChannel {
    pub fn as_str(self) -> &'static str {
        match self {
            AppleChannel::Ios => "ios",
            AppleChannel::Macos => "macos",
        }
    }
}

/// What the current operation needs from an Apple credential.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AppleNeed {
    /// Any auth session (ASC preferred, Apple ID accepted).
    Auth,
    /// An App Store Connect API key specifically.
    Asc,
    /// A Developer ID Application certificate.
    DeveloperId,
}

impl AppleNeed {
    fn satisfied_by(self, slots: &AppleTeamSlots) -> bool {
        match self {
            AppleNeed::Auth => slots.has_auth(),
            AppleNeed::Asc => slots.has_asc,
            AppleNeed::DeveloperId => slots.has_developer_id,
        }
    }

    fn describe(self) -> &'static str {
        match self {
            AppleNeed::Auth => "an Apple login",
            AppleNeed::Asc => "an App Store Connect API key",
            AppleNeed::DeveloperId => "a Developer ID certificate",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthSource {
    /// Built from CI environment variables; never touches bindings.
    Env,
    /// Loaded from the wallet for `team_id`.
    Wallet,
}

pub struct ResolvedAppleAuth {
    pub auth: AuthCredentials,
    pub source: AuthSource,
}

/// The project this invocation runs in, if any (`lingxia.yaml` found by
/// walking up from the working directory).
pub struct ProjectContext {
    pub root: PathBuf,
    pub config: LingXiaConfig,
}

pub fn detect_project() -> Result<Option<ProjectContext>> {
    let Ok(mut dir) = std::env::current_dir() else {
        return Ok(None);
    };
    loop {
        if crate::config::has_host_config(&dir) {
            let config = LingXiaConfig::load(&dir)
                .with_context(|| format!("load lingxia.yaml in {}", dir.display()))?;
            return Ok(Some(ProjectContext { root: dir, config }));
        }
        if !dir.pop() {
            return Ok(None);
        }
    }
}

fn team_constraint(
    project: Option<&ProjectContext>,
    channel: Option<AppleChannel>,
) -> Option<String> {
    let config = &project?.config;
    let team = match channel? {
        AppleChannel::Ios => config.ios.as_ref()?.team_id.clone(),
        AppleChannel::Macos => config.macos.as_ref()?.team_id.clone(),
    };
    team.filter(|t| !t.trim().is_empty())
}

fn interactive() -> bool {
    std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
}

/// Whether prompting the user is possible in this invocation.
pub fn is_interactive() -> bool {
    interactive()
}

fn env_nonempty(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

/// The canonical ASC env group, complete or absent. A partial group — or a
/// retired spelling — is a hard error, never mixed with disk credentials.
fn asc_from_env(constraint: Option<&str>) -> Result<Option<AuthCredentials>> {
    let key_path = env_nonempty(ENV_KEY_PATH);
    let key_id = env_nonempty(ENV_KEY_ID);
    let issuer_id = env_nonempty(ENV_ISSUER_ID);

    if key_path.is_none() && key_id.is_none() && issuer_id.is_none() {
        let stale: Vec<String> = LEGACY_ENV
            .iter()
            .filter(|(old, _)| env_nonempty(old).is_some())
            .map(|(old, new)| format!("  {old} -> {new}"))
            .collect();
        if !stale.is_empty() {
            bail!(
                "{}: these environment variables are no longer read; rename them:\n{}",
                codes::CREDENTIAL_ENV_INCOMPLETE,
                stale.join("\n")
            );
        }
        return Ok(None);
    }

    let (Some(key_path), Some(key_id), Some(issuer_id)) = (key_path, key_id, issuer_id) else {
        let missing: Vec<&str> = [
            (ENV_KEY_PATH, env_nonempty(ENV_KEY_PATH)),
            (ENV_KEY_ID, env_nonempty(ENV_KEY_ID)),
            (ENV_ISSUER_ID, env_nonempty(ENV_ISSUER_ID)),
        ]
        .into_iter()
        .filter(|(_, v)| v.is_none())
        .map(|(k, _)| k)
        .collect();
        bail!(
            "{}: the Apple credential env group must be complete; missing: {}",
            codes::CREDENTIAL_ENV_INCOMPLETE,
            missing.join(", ")
        );
    };

    let private_key_pem = std::fs::read_to_string(&key_path)
        .with_context(|| format!("read {ENV_KEY_PATH}={key_path}"))?;
    let env_team = env_nonempty(ENV_TEAM_ID);
    if let (Some(constraint), Some(env_team)) = (constraint, env_team.as_deref())
        && constraint != env_team
    {
        bail!(
            "{}: {ENV_TEAM_ID}={env_team} does not match the project teamId {constraint}",
            codes::CREDENTIAL_IDENTITY_MISMATCH
        );
    }
    let team_id = env_team
        .or_else(|| constraint.map(str::to_string))
        .unwrap_or_default();

    Ok(Some(AuthCredentials::AppStoreConnect {
        key_id,
        issuer_id,
        private_key_pem,
        team_id,
        cached_signing_identity: None,
    }))
}

/// Everything team resolution needs, injected for testability.
struct ResolveInput<'a> {
    wallet: &'a Wallet,
    bindings: &'a BindingStore,
    /// `(project root, channel)` when the resolution is bindable.
    binding_key: Option<(&'a std::path::Path, &'a str)>,
    constraint: Option<&'a str>,
    need: AppleNeed,
    allow_login: bool,
    interactive: bool,
}

/// Resolve the Apple team for this invocation, per the zero-choice rules.
/// Returns the team id; the caller loads the slot it needs from the wallet.
fn resolve_team(input: &ResolveInput) -> Result<String> {
    let ResolveInput {
        wallet,
        bindings,
        binding_key,
        constraint,
        need,
        allow_login,
        interactive,
    } = *input;

    // Binding cache: valid → use; invalid → drop and remember what it said.
    let mut previous_identity: Option<String> = None;
    if let Some((root, channel)) = binding_key
        && let Some(binding) = bindings.load(root, channel)
    {
        let constraint_ok = constraint.is_none_or(|c| c == binding.identity);
        if constraint_ok && let Some(slots) = wallet.apple_team(&binding.identity)? {
            if need.satisfied_by(&slots) {
                return Ok(binding.identity);
            }
            // Same identity, missing capability: repair in place, never
            // switch teams behind the user's back.
            if allow_login && interactive {
                let team = crate::commands::auth::inline_login(Some(&binding.identity), need)?;
                return Ok(team);
            }
            bail!(
                "{}: team {} has no {}. Fix: lingxia auth login apple",
                codes::CREDENTIAL_CAPABILITY_MISSING,
                binding.identity,
                need.describe()
            );
        }
        previous_identity = Some(binding.identity);
    }

    let resolved = resolve_team_fresh(wallet, constraint, need, allow_login, interactive)?;

    // A dropped binding that resolves to a different organization needs one
    // explicit confirmation — never a silent switch. A committed constraint is
    // the project owner's explicit intent, so it needs no extra confirmation.
    if constraint.is_none()
        && let Some(previous) = previous_identity.filter(|p| *p != resolved)
    {
        if !interactive {
            bail!(
                "{}: this checkout previously used team {previous}, which is no longer usable; \
                 re-run interactively to confirm team {resolved}, or `lingxia auth forget` first",
                codes::CREDENTIAL_SELECTION_REQUIRED
            );
        }
        let confirmed = dialoguer::Confirm::new()
            .with_prompt(format!(
                "This checkout previously used Apple team {previous}; continue with {resolved}?"
            ))
            .default(true)
            .interact()?;
        if !confirmed {
            bail!(
                "{}: resolution cancelled; run `lingxia auth login apple` for the team you want",
                codes::CREDENTIAL_SELECTION_REQUIRED
            );
        }
    }

    if let Some((root, channel)) = binding_key {
        bindings.save(root, channel, "apple", &resolved, constraint)?;
    }
    Ok(resolved)
}

/// Wallet lookup without a binding: constraint match, sole candidate, or one
/// interactive selection.
fn resolve_team_fresh(
    wallet: &Wallet,
    constraint: Option<&str>,
    need: AppleNeed,
    allow_login: bool,
    interactive: bool,
) -> Result<String> {
    if let Some(constraint) = constraint {
        if let Some(slots) = wallet.apple_team(constraint)? {
            if need.satisfied_by(&slots) {
                return Ok(constraint.to_string());
            }
            if allow_login && interactive {
                return crate::commands::auth::inline_login(Some(constraint), need);
            }
            bail!(
                "{}: team {constraint} has no {}. Fix: lingxia auth login apple",
                codes::CREDENTIAL_CAPABILITY_MISSING,
                need.describe()
            );
        }
        if allow_login && interactive {
            return crate::commands::auth::inline_login(Some(constraint), need);
        }
        bail!(
            "{}: no credentials for team {constraint} (required by lingxia.yaml). \
             Fix: lingxia auth login apple",
            codes::CREDENTIALS_MISSING
        );
    }

    let candidates: Vec<AppleTeamSlots> = wallet
        .apple_teams()?
        .into_iter()
        .filter(|slots| need.satisfied_by(slots))
        .collect();

    match candidates.len() {
        0 => {
            if allow_login && interactive {
                return crate::commands::auth::inline_login(None, need);
            }
            bail!(
                "{}: no Apple credentials with {}. Fix: lingxia auth login apple",
                codes::CREDENTIALS_MISSING,
                need.describe()
            );
        }
        1 => Ok(candidates[0].team_id.clone()),
        _ => {
            if !interactive {
                bail!(
                    "{}: {} Apple teams can serve this operation ({}); set teamId in \
                     lingxia.yaml or provide the credential env group",
                    codes::CREDENTIAL_SELECTION_REQUIRED,
                    candidates.len(),
                    candidates
                        .iter()
                        .map(|c| c.team_id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            eprintln!("This checkout can use {} Apple teams:", candidates.len());
            let labels: Vec<String> = candidates
                .iter()
                .map(|c| format!("{}   {}", c.team_id, c.mechanisms()))
                .collect();
            let selection = dialoguer::Select::new()
                .with_prompt("Select once for this checkout")
                .items(&labels)
                .default(0)
                .interact()?;
            Ok(candidates[selection].team_id.clone())
        }
    }
}

/// Resolve an auth credential (ASC preferred, Apple ID accepted). In a TTY a
/// missing credential triggers the login flow in place; the original command
/// then continues.
pub fn resolve_apple_auth(
    channel: Option<AppleChannel>,
    need: AppleNeed,
) -> Result<ResolvedAppleAuth> {
    debug_assert!(need != AppleNeed::DeveloperId);
    let project = detect_project()?;
    let constraint = team_constraint(project.as_ref(), channel);

    if let Some(auth) = asc_from_env(constraint.as_deref())? {
        return Ok(ResolvedAppleAuth {
            auth,
            source: AuthSource::Env,
        });
    }

    let wallet = Wallet::open()?;
    wallet.notice_legacy_files();
    let bindings = BindingStore::open()?;
    let team = resolve_team(&ResolveInput {
        wallet: &wallet,
        bindings: &bindings,
        binding_key: binding_key(project.as_ref(), channel),
        constraint: constraint.as_deref(),
        need,
        allow_login: true,
        interactive: interactive(),
    })?;
    let auth = match need {
        AppleNeed::Asc => wallet.load_apple_asc(&team)?,
        _ => wallet.load_apple_auth(&team)?,
    };
    let auth = auth.ok_or_else(|| {
        anyhow!(
            "{}: credentials for team {team} disappeared during resolution",
            codes::CREDENTIALS_MISSING
        )
    })?;
    Ok(ResolvedAppleAuth {
        auth,
        source: AuthSource::Wallet,
    })
}

/// ASC key material for notarization/store use.
pub struct AscMaterial {
    pub key_id: String,
    pub issuer_id: String,
    pub private_key_pem: String,
}

/// Resolve an ASC key if one is available; `None` means "nothing configured"
/// so callers keep their skip semantics (e.g. unsigned developer builds).
pub fn try_resolve_apple_asc(channel: Option<AppleChannel>) -> Result<Option<AscMaterial>> {
    let project = detect_project()?;
    let constraint = team_constraint(project.as_ref(), channel);

    let auth = match asc_from_env(constraint.as_deref())? {
        Some(auth) => Some(auth),
        None => {
            let wallet = Wallet::open()?;
            wallet.notice_legacy_files();
            let has_any = wallet
                .apple_teams()?
                .iter()
                .any(|slots| AppleNeed::Asc.satisfied_by(slots));
            if !has_any {
                return Ok(None);
            }
            let bindings = BindingStore::open()?;
            let team = resolve_team(&ResolveInput {
                wallet: &wallet,
                bindings: &bindings,
                binding_key: binding_key(project.as_ref(), channel),
                constraint: constraint.as_deref(),
                need: AppleNeed::Asc,
                allow_login: false,
                interactive: interactive(),
            })?;
            wallet.load_apple_asc(&team)?
        }
    };

    Ok(auth.map(|auth| match auth {
        AuthCredentials::AppStoreConnect {
            key_id,
            issuer_id,
            private_key_pem,
            ..
        } => AscMaterial {
            key_id,
            issuer_id,
            private_key_pem,
        },
        AuthCredentials::AppleId { .. } => unreachable!("ASC slot holds an ASC key"),
    }))
}

/// Resolve a Developer ID certificate if one is available (wallet only; the
/// env pair is handled by the notarize module which owns the temp-file flow).
pub fn try_resolve_apple_developer_id(
    channel: Option<AppleChannel>,
) -> Result<Option<crate::platform::apple::auth::DeveloperIdCredentials>> {
    let project = detect_project()?;
    let constraint = team_constraint(project.as_ref(), channel);
    let wallet = Wallet::open()?;
    wallet.notice_legacy_files();
    let has_any = wallet
        .apple_teams()?
        .iter()
        .any(|slots| slots.has_developer_id);
    if !has_any {
        return Ok(None);
    }
    let bindings = BindingStore::open()?;
    let team = resolve_team(&ResolveInput {
        wallet: &wallet,
        bindings: &bindings,
        binding_key: binding_key(project.as_ref(), channel),
        constraint: constraint.as_deref(),
        need: AppleNeed::DeveloperId,
        allow_login: false,
        interactive: interactive(),
    })?;
    wallet.load_apple_developer_id(&team)
}

/// The `(project root, channel)` binding key, when both are known.
fn binding_key(
    project: Option<&ProjectContext>,
    channel: Option<AppleChannel>,
) -> Option<(&std::path::Path, &str)> {
    Some((project?.root.as_path(), channel?.as_str()))
}

/// Read-only status of one Apple channel: what would resolve, without writing
/// bindings or prompting.
pub struct ChannelDiagnosis {
    pub channel: &'static str,
    pub constraint: Option<String>,
    pub binding: Option<String>,
    /// Human summary of the credential that would be used.
    pub credential: Option<String>,
    pub ready: bool,
    pub error_code: Option<&'static str>,
    pub fix: Option<String>,
}

pub fn diagnose_apple_channel(
    project: &ProjectContext,
    channel: AppleChannel,
) -> Result<ChannelDiagnosis> {
    let constraint = team_constraint(Some(project), Some(channel));
    let mut diagnosis = ChannelDiagnosis {
        channel: channel.as_str(),
        constraint: constraint.clone(),
        binding: None,
        credential: None,
        ready: false,
        error_code: None,
        fix: None,
    };

    match asc_from_env(constraint.as_deref()) {
        Ok(Some(_)) => {
            diagnosis.credential = Some("environment (ASC key)".to_string());
            diagnosis.ready = true;
            return Ok(diagnosis);
        }
        Ok(None) => {}
        Err(e) => {
            let message = e.to_string();
            diagnosis.error_code = Some(
                if message.starts_with(codes::CREDENTIAL_IDENTITY_MISMATCH) {
                    codes::CREDENTIAL_IDENTITY_MISMATCH
                } else {
                    codes::CREDENTIAL_ENV_INCOMPLETE
                },
            );
            diagnosis.fix = Some(message);
            return Ok(diagnosis);
        }
    }

    let wallet = Wallet::open()?;
    let bindings = BindingStore::open()?;
    let binding = bindings.load(&project.root, channel.as_str());
    diagnosis.binding = binding.as_ref().map(|b| b.identity.clone());

    let describe = |slots: &AppleTeamSlots| format!("{} ({})", slots.team_id, slots.mechanisms());

    if let Some(binding) = &binding
        && constraint.as_deref().is_none_or(|c| c == binding.identity)
        && let Some(slots) = wallet.apple_team(&binding.identity)?
        && slots.has_auth()
    {
        diagnosis.credential = Some(describe(&slots));
        diagnosis.ready = true;
        return Ok(diagnosis);
    }

    if let Some(constraint) = &constraint {
        if let Some(slots) = wallet.apple_team(constraint)?
            && slots.has_auth()
        {
            diagnosis.credential = Some(describe(&slots));
            diagnosis.ready = true;
        } else {
            diagnosis.error_code = Some(codes::CREDENTIALS_MISSING);
            diagnosis.fix = Some("lingxia auth login apple".to_string());
        }
        return Ok(diagnosis);
    }

    let candidates: Vec<AppleTeamSlots> = wallet
        .apple_teams()?
        .into_iter()
        .filter(AppleTeamSlots::has_auth)
        .collect();
    match candidates.len() {
        0 => {
            diagnosis.error_code = Some(codes::CREDENTIALS_MISSING);
            diagnosis.fix = Some("lingxia auth login apple".to_string());
        }
        1 => {
            diagnosis.credential = Some(describe(&candidates[0]));
            diagnosis.ready = true;
        }
        _ => {
            diagnosis.error_code = Some(codes::CREDENTIAL_SELECTION_REQUIRED);
            diagnosis.fix = Some(
                "run any Apple command interactively to select once, or set teamId in lingxia.yaml"
                    .to_string(),
            );
        }
    }
    Ok(diagnosis)
}

const ENV_AGC_CLIENT_ID: &str = "LINGXIA_AGC_CLIENT_ID";
const ENV_AGC_CLIENT_SECRET: &str = "LINGXIA_AGC_CLIENT_SECRET";

pub struct ResolvedHarmonyAgc {
    pub credentials: AgcApiCredentials,
    pub source: AuthSource,
}

/// The AGC env pair, complete or absent.
fn harmony_from_env() -> Result<Option<AgcApiCredentials>> {
    match (
        env_nonempty(ENV_AGC_CLIENT_ID),
        env_nonempty(ENV_AGC_CLIENT_SECRET),
    ) {
        (Some(client_id), Some(client_secret)) => Ok(Some(AgcApiCredentials {
            client_id,
            client_secret,
            token: None,
        })),
        (None, None) => Ok(None),
        _ => bail!(
            "{}: {ENV_AGC_CLIENT_ID} and {ENV_AGC_CLIENT_SECRET} must be provided together",
            codes::CREDENTIAL_ENV_INCOMPLETE
        ),
    }
}

/// Resolve Harmony AGC credentials: env pair → binding cache → wallet. There
/// is no yaml org constraint yet (AGC has no stable public org identity), so
/// routing relies on the automatic per-checkout binding.
pub fn resolve_harmony_agc(allow_login: bool) -> Result<ResolvedHarmonyAgc> {
    if let Some(credentials) = harmony_from_env()? {
        return Ok(ResolvedHarmonyAgc {
            credentials,
            source: AuthSource::Env,
        });
    }

    let wallet = Wallet::open()?;
    wallet.notice_legacy_files();
    let project = detect_project()?;
    let bindings = BindingStore::open()?;
    let binding_key = project.as_ref().map(|p| (p.root.as_path(), "harmony"));
    let client_id =
        resolve_harmony_identity(&wallet, &bindings, binding_key, allow_login, interactive())?;
    let credentials = wallet.load_harmony_agc(&client_id)?.ok_or_else(|| {
        anyhow!(
            "{}: AGC credentials for {client_id} disappeared during resolution",
            codes::CREDENTIALS_MISSING
        )
    })?;
    Ok(ResolvedHarmonyAgc {
        credentials,
        source: AuthSource::Wallet,
    })
}

/// `Ok(None)` when nothing is configured, for callers with skip semantics.
pub fn try_resolve_harmony_agc() -> Result<Option<ResolvedHarmonyAgc>> {
    if harmony_from_env()?.is_none() && Wallet::open()?.harmony_identities()?.is_empty() {
        return Ok(None);
    }
    resolve_harmony_agc(false).map(Some)
}

fn resolve_harmony_identity(
    wallet: &Wallet,
    bindings: &BindingStore,
    binding_key: Option<(&std::path::Path, &str)>,
    allow_login: bool,
    interactive: bool,
) -> Result<String> {
    resolve_single_identity(&SingleIdentityInput {
        provider: "harmony",
        label: "Harmony AGC identity",
        login_cmd: "lingxia auth login harmony",
        identities: &wallet.harmony_identities()?,
        bindings,
        binding_key,
        interactive,
        inline_login: if allow_login {
            Some(&crate::commands::auth::harmony_inline_login)
        } else {
            None
        },
    })
}

/// Resolution for providers with one mechanism and no yaml constraint:
/// binding cache → sole identity → one interactive selection, with the same
/// never-switch-organizations-silently rule as Apple.
pub(crate) struct SingleIdentityInput<'a> {
    /// Binding provider key, e.g. `harmony`, `googleplay`.
    pub provider: &'a str,
    /// Human label for messages, e.g. `Harmony AGC identity`.
    pub label: &'a str,
    /// The login command that fixes a missing credential.
    pub login_cmd: &'a str,
    pub identities: &'a [String],
    pub bindings: &'a BindingStore,
    pub binding_key: Option<(&'a std::path::Path, &'a str)>,
    pub interactive: bool,
    /// In-place login hook; returns the identity that was logged in.
    #[allow(clippy::type_complexity)]
    pub inline_login: Option<&'a dyn Fn() -> Result<String>>,
}

pub(crate) fn resolve_single_identity(input: &SingleIdentityInput) -> Result<String> {
    let SingleIdentityInput {
        provider,
        label,
        login_cmd,
        identities,
        bindings,
        binding_key,
        interactive,
        inline_login,
    } = *input;

    let mut previous_identity: Option<String> = None;
    if let Some((root, channel)) = binding_key
        && let Some(binding) = bindings.load(root, channel)
    {
        if identities.contains(&binding.identity) {
            return Ok(binding.identity);
        }
        previous_identity = Some(binding.identity);
    }

    let resolved = match identities.len() {
        0 => match inline_login.filter(|_| interactive) {
            Some(login) => login()?,
            None => bail!(
                "{}: no {label} stored. Fix: {login_cmd}",
                codes::CREDENTIALS_MISSING
            ),
        },
        1 => identities[0].clone(),
        _ => {
            if !interactive {
                bail!(
                    "{}: {} {label} candidates ({}); provide the credential env group or \
                     select once interactively",
                    codes::CREDENTIAL_SELECTION_REQUIRED,
                    identities.len(),
                    identities.join(", ")
                );
            }
            eprintln!(
                "This checkout can use {} {label} candidates:",
                identities.len()
            );
            let selection = dialoguer::Select::new()
                .with_prompt("Select once for this checkout")
                .items(identities)
                .default(0)
                .interact()?;
            identities[selection].clone()
        }
    };

    if let Some(previous) = previous_identity.filter(|p| *p != resolved) {
        if !interactive {
            bail!(
                "{}: this checkout previously used {label} {previous}, which is no longer \
                 stored; re-run interactively to confirm {resolved}, or `lingxia auth forget` first",
                codes::CREDENTIAL_SELECTION_REQUIRED
            );
        }
        let confirmed = dialoguer::Confirm::new()
            .with_prompt(format!(
                "This checkout previously used {label} {previous}; continue with {resolved}?"
            ))
            .default(true)
            .interact()?;
        if !confirmed {
            bail!(
                "{}: resolution cancelled; run `{login_cmd}` for the identity you want",
                codes::CREDENTIAL_SELECTION_REQUIRED
            );
        }
    }

    if let Some((root, channel)) = binding_key {
        bindings.save(root, channel, provider, &resolved, None)?;
    }
    Ok(resolved)
}

/// Human-readable pointer shown before an in-place login starts.
pub fn announce_inline_login(need: AppleNeed, team: Option<&str>) {
    match team {
        Some(team) => eprintln!(
            "{} Missing {} for Apple team {team}; logging in now, then the command continues.",
            "→".cyan(),
            need.describe()
        ),
        None => eprintln!(
            "{} Missing {}; logging in now, then the command continues.",
            "→".cyan(),
            need.describe()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::apple::auth::AuthCredentials;

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

    struct Fixture {
        _state: tempfile::TempDir,
        project: tempfile::TempDir,
        wallet: Wallet,
        bindings: BindingStore,
    }

    fn fixture() -> Fixture {
        let state = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let wallet = Wallet::at(state.path());
        let bindings = BindingStore::at(state.path());
        Fixture {
            _state: state,
            project,
            wallet,
            bindings,
        }
    }

    impl Fixture {
        fn resolve(&self, constraint: Option<&str>, need: AppleNeed) -> Result<String> {
            resolve_team(&ResolveInput {
                wallet: &self.wallet,
                bindings: &self.bindings,
                binding_key: Some((self.project.path(), "ios")),
                constraint,
                need,
                allow_login: false,
                interactive: false,
            })
        }

        fn binding_identity(&self) -> Option<String> {
            self.bindings
                .load(self.project.path(), "ios")
                .map(|b| b.identity)
        }
    }

    fn code_of(err: anyhow::Error) -> String {
        let text = err.to_string();
        text.split(':').next().unwrap_or("").to_string()
    }

    #[test]
    fn sole_candidate_resolves_and_binds() {
        let f = fixture();
        f.wallet.save_apple_auth(&asc("TEAMAAAAAA")).unwrap();
        assert_eq!(f.resolve(None, AppleNeed::Auth).unwrap(), "TEAMAAAAAA");
        assert_eq!(f.binding_identity().as_deref(), Some("TEAMAAAAAA"));
    }

    #[test]
    fn constraint_picks_among_multiple() {
        let f = fixture();
        f.wallet.save_apple_auth(&asc("TEAMAAAAAA")).unwrap();
        f.wallet.save_apple_auth(&asc("TEAMBBBBBB")).unwrap();
        assert_eq!(
            f.resolve(Some("TEAMBBBBBB"), AppleNeed::Auth).unwrap(),
            "TEAMBBBBBB"
        );
        assert_eq!(f.binding_identity().as_deref(), Some("TEAMBBBBBB"));
    }

    #[test]
    fn binding_hit_wins_without_constraint() {
        let f = fixture();
        f.wallet.save_apple_auth(&asc("TEAMAAAAAA")).unwrap();
        f.wallet.save_apple_auth(&asc("TEAMBBBBBB")).unwrap();
        f.bindings
            .save(f.project.path(), "ios", "apple", "TEAMBBBBBB", None)
            .unwrap();
        assert_eq!(f.resolve(None, AppleNeed::Auth).unwrap(), "TEAMBBBBBB");
    }

    #[test]
    fn multi_candidates_fail_non_interactively() {
        let f = fixture();
        f.wallet.save_apple_auth(&asc("TEAMAAAAAA")).unwrap();
        f.wallet.save_apple_auth(&asc("TEAMBBBBBB")).unwrap();
        let err = f.resolve(None, AppleNeed::Auth).unwrap_err();
        assert_eq!(code_of(err), codes::CREDENTIAL_SELECTION_REQUIRED);
        assert!(f.binding_identity().is_none());
    }

    #[test]
    fn stale_binding_reresolves_to_same_org_silently() {
        let f = fixture();
        f.wallet.save_apple_auth(&apple_id("TEAMAAAAAA")).unwrap();
        // Binding references the team, but the operation needs an ASC key and
        // the wallet only has an Apple ID session -> capability error, not a
        // silent switch.
        f.bindings
            .save(f.project.path(), "ios", "apple", "TEAMAAAAAA", None)
            .unwrap();
        let err = f.resolve(None, AppleNeed::Asc).unwrap_err();
        assert_eq!(code_of(err), codes::CREDENTIAL_CAPABILITY_MISSING);
    }

    #[test]
    fn stale_binding_with_different_org_requires_confirmation() {
        let f = fixture();
        f.wallet.save_apple_auth(&asc("TEAMBBBBBB")).unwrap();
        // Bound team was deleted from the wallet; the sole remaining team is a
        // different organization -> non-interactive must not switch silently.
        f.bindings
            .save(f.project.path(), "ios", "apple", "TEAMAAAAAA", None)
            .unwrap();
        let err = f.resolve(None, AppleNeed::Auth).unwrap_err();
        assert_eq!(code_of(err), codes::CREDENTIAL_SELECTION_REQUIRED);
        // The old binding stays until the user confirms or forgets.
        assert_eq!(f.binding_identity().as_deref(), Some("TEAMAAAAAA"));
    }

    #[test]
    fn constraint_change_rebinds_without_confirmation() {
        let f = fixture();
        f.wallet.save_apple_auth(&asc("TEAMAAAAAA")).unwrap();
        f.wallet.save_apple_auth(&asc("TEAMBBBBBB")).unwrap();
        f.bindings
            .save(f.project.path(), "ios", "apple", "TEAMAAAAAA", None)
            .unwrap();
        // The project owner committed a teamId; that intent needs no prompt.
        assert_eq!(
            f.resolve(Some("TEAMBBBBBB"), AppleNeed::Auth).unwrap(),
            "TEAMBBBBBB"
        );
        assert_eq!(f.binding_identity().as_deref(), Some("TEAMBBBBBB"));
    }

    #[test]
    fn harmony_sole_identity_binds() {
        let f = fixture();
        f.wallet
            .save_harmony_agc(&crate::platform::harmony::AgcApiCredentials {
                client_id: "123456789".into(),
                client_secret: "s".into(),
                token: None,
            })
            .unwrap();
        let id = resolve_harmony_identity(
            &f.wallet,
            &f.bindings,
            Some((f.project.path(), "harmony")),
            false,
            false,
        )
        .unwrap();
        assert_eq!(id, "123456789");
        assert_eq!(
            f.bindings
                .load(f.project.path(), "harmony")
                .map(|b| b.identity)
                .as_deref(),
            Some("123456789")
        );
    }

    #[test]
    fn harmony_stale_binding_needs_confirmation() {
        let f = fixture();
        f.wallet
            .save_harmony_agc(&crate::platform::harmony::AgcApiCredentials {
                client_id: "222222222".into(),
                client_secret: "s".into(),
                token: None,
            })
            .unwrap();
        f.bindings
            .save(f.project.path(), "harmony", "harmony", "111111111", None)
            .unwrap();
        let err = resolve_harmony_identity(
            &f.wallet,
            &f.bindings,
            Some((f.project.path(), "harmony")),
            false,
            false,
        )
        .unwrap_err();
        assert_eq!(code_of(err), codes::CREDENTIAL_SELECTION_REQUIRED);
    }

    #[test]
    fn missing_credentials_have_stable_codes() {
        let f = fixture();
        let err = f.resolve(None, AppleNeed::Auth).unwrap_err();
        assert_eq!(code_of(err), codes::CREDENTIALS_MISSING);

        let err = f.resolve(Some("TEAMAAAAAA"), AppleNeed::Auth).unwrap_err();
        assert_eq!(code_of(err), codes::CREDENTIALS_MISSING);

        f.wallet.save_apple_auth(&apple_id("TEAMAAAAAA")).unwrap();
        let err = f.resolve(Some("TEAMAAAAAA"), AppleNeed::Asc).unwrap_err();
        assert_eq!(code_of(err), codes::CREDENTIAL_CAPABILITY_MISSING);
    }
}
