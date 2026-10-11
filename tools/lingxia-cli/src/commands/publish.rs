use anyhow::{Context, Result, bail};
use colored::Colorize;
use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::cli_config::CliConfig;
use crate::config::{AppEnv, has_host_config};
use crate::http_client;
use crate::lxapp;

pub struct PublishOptions {
    pub token: Option<String>,
    pub lingxia_server: Option<String>,
    pub package: Option<String>,
    pub env: Option<String>,
    pub channel: Option<String>,
    pub framework: Option<String>,
    pub progress: Option<String>,
    pub update_signing_key: Option<String>,
    pub dry_run: bool,
}

#[derive(Debug)]
struct PackageMeta {
    target: String,
    target_id: String,
    version: String,
    env: AppEnv,
    channel: Option<String>,
    min_runtime: String,
}

struct ResolvedPackage {
    path: PathBuf,
    platform: Option<String>,
    cleanup_after_publish: bool,
}

impl Drop for ResolvedPackage {
    fn drop(&mut self) {
        if self.cleanup_after_publish {
            let _ = fs::remove_file(&self.path);
        }
    }
}

pub fn execute(opts: PublishOptions) -> Result<()> {
    let cwd = env::current_dir()?;

    let (meta, package) = resolve_publish_target(&cwd, &opts)?;
    let package_path = &package.path;
    if opts.dry_run {
        // The resolved, verified package is the dry run's whole output, so CI
        // can hand it to its own checks.
        println!("{}", package_path.display());
        return Ok(());
    }
    // Resolve server and token after env is known. The token is keyed by
    // (canonical server URL, env) in the wallet.
    let lingxia_server = resolve_lingxia_server(meta.env, opts.lingxia_server)?;
    let lingxia_server = lingxia_server.trim_end_matches('/').to_string();
    let token = resolve_token(meta.env, &lingxia_server, opts.token)?;
    let file_name = package_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "package".to_string());

    let channel_label = meta
        .channel
        .as_deref()
        .map(|c| format!(" ({c})"))
        .unwrap_or_default();
    println!(
        "{}  Publishing {} {} v{}{} …",
        "→".cyan(),
        meta.target,
        meta.target_id.bold(),
        meta.version.bold(),
        channel_label,
    );
    println!("   Package: {}", package_path.display());

    let file_data = fs::read(package_path)
        .with_context(|| format!("Failed to read package: {}", package_path.display()))?;
    let sha256 = lingxia_update::archive_sha256_hex(&file_data);
    println!("   SHA256:  {sha256}");
    if !meta.min_runtime.is_empty() {
        println!("   minRuntime: {}", meta.min_runtime);
    }

    let channel = meta.channel.as_deref().unwrap_or("");
    let platform = match meta.target.as_str() {
        "app" => package
            .platform
            .clone()
            .context("host package platform is unknown")?,
        _ => "any".to_string(),
    };
    let update_signing_key = clean_arg(opts.update_signing_key, "--update-signing-key")?;
    let extra = signed_multipart_fields(
        meta.env,
        update_signing_key.as_deref(),
        &lingxia_update::SignRequest {
            kind: &meta.target,
            target_id: &meta.target_id,
            channel,
            platform: &platform,
            version: &meta.version,
            sha256: &sha256,
        },
    )?;
    let (upload_url, fields) = publish_upload(
        &lingxia_server,
        &meta,
        package.platform.as_deref(),
        &sha256,
        &extra,
    )?;
    println!("   Upload → {upload_url}");
    let field_refs: Vec<(&str, &str)> = fields
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    let boundary = format!("----LingXiaBoundary{}", rand_hex());
    let body = build_multipart(&boundary, &field_refs, &file_name, &file_data);
    let content_type = format!("multipart/form-data; boundary={boundary}");

    let agent = http_client::create_agent(120);
    let mut resp = agent
        .post(&upload_url)
        .header("Authorization", &format!("Bearer {token}"))
        .header("Content-Type", &content_type)
        .send(body.as_slice())
        .map_err(|err| upload_transport_error(&upload_url, file_data.len(), err))?;

    let status = resp.status().as_u16();
    let body_str = resp
        .body_mut()
        .read_to_string()
        .unwrap_or_else(|_| "<unreadable>".to_string());

    if status == 200 {
        println!("{} Published successfully.", "✓".green().bold());
        if meta.target == "lxapp" && meta.channel.as_deref() == Some("draft") {
            match draft_open_url(&lingxia_server, &meta.target_id) {
                Ok(url) => {
                    println!("   Open draft: {url}");
                    if let Err(err) = print_draft_qr(&url) {
                        eprintln!("Could not display QR code: {err}");
                    }
                }
                Err(err) => eprintln!("Could not generate draft link: {err}"),
            }
        }
        Ok(())
    } else {
        bail!("Upload failed (HTTP {status}): {body_str}");
    }
}

fn draft_open_url(server: &str, appid: &str) -> Result<String> {
    let mut url = url::Url::parse(server).context("invalid publish server URL")?;
    if url.scheme() != "https" || url.host_str().is_none() {
        bail!("draft scan links require an HTTPS publish server");
    }
    if !url.username().is_empty() || url.password().is_some() {
        bail!("draft scan links cannot include server credentials");
    }
    url.set_path("/lxapp/open");
    url.set_query(None);
    url.set_fragment(None);
    url.set_query(Some(&format!(
        "appId={}&channel=draft",
        urlencoding::encode(appid)
    )));
    Ok(url.into())
}

fn draft_qr_png(code: &qrcode::QrCode) -> Result<Vec<u8>> {
    let width = ((code.width() + 8) * 6) as u32;
    let image = image::GrayImage::from_fn(width, width, |x, y| {
        let x = (x / 6) as usize;
        let y = (y / 6) as usize;
        let dark = x >= 4
            && y >= 4
            && x < code.width() + 4
            && y < code.width() + 4
            && code[(x - 4, y - 4)] == qrcode::Color::Dark;
        image::Luma([if dark { 0 } else { 255 }])
    });
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageLuma8(image).write_to(&mut png, image::ImageFormat::Png)?;
    Ok(png.into_inner())
}

fn print_draft_qr(url: &str) -> Result<()> {
    use base64::Engine;
    use std::io::{IsTerminal, Write};

    let mut stdout = std::io::stdout().lock();
    if !stdout.is_terminal() {
        return Ok(());
    }
    let code = qrcode::QrCode::new(url.as_bytes())?;
    let term_program = env::var("TERM_PROGRAM").unwrap_or_default();
    let kitty = env::var("TERM").is_ok_and(|term| term == "xterm-kitty")
        || env::var_os("KITTY_WINDOW_ID").is_some()
        || term_program == "ghostty";
    // WezTerm ships with Kitty graphics disabled but speaks the iTerm2 protocol.
    let iterm = matches!(term_program.as_str(), "iTerm.app" | "WezTerm");
    // Multiplexers may suppress graphics escapes; text remains scannable.
    if (kitty || iterm) && env::var_os("TMUX").is_none() && env::var_os("STY").is_none() {
        let data = base64::engine::general_purpose::STANDARD.encode(draft_qr_png(&code)?);
        if iterm {
            writeln!(
                stdout,
                "\x1b]1337;File=inline=1;preserveAspectRatio=1:{data}\x07"
            )?;
        } else {
            let chunks: Vec<_> = data.as_bytes().chunks(4096).collect();
            for (index, chunk) in chunks.iter().enumerate() {
                let more = usize::from(index + 1 < chunks.len());
                let header = if index == 0 { "a=T,f=100,t=d,q=2," } else { "" };
                write!(
                    stdout,
                    "\x1b_G{header}m={more};{}\x1b\\",
                    std::str::from_utf8(chunk)?
                )?;
            }
            writeln!(stdout)?;
        }
    } else {
        let width = code.width() + 8;
        if width >= usize::from(console::Term::stdout().size().1) {
            return Ok(());
        }
        let dark = |x: usize, y: usize| {
            x >= 4
                && y >= 4
                && x < code.width() + 4
                && y < code.width() + 4
                && code[(x - 4, y - 4)] == qrcode::Color::Dark
        };
        for y in (0..width).step_by(2) {
            write!(stdout, "\x1b[30;47m")?;
            for x in 0..width {
                let cell = match (dark(x, y), dark(x, y + 1)) {
                    (false, false) => ' ',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (true, true) => '█',
                };
                write!(stdout, "{cell}")?;
            }
            writeln!(stdout, "\x1b[0m")?;
        }
    }
    stdout.flush()?;
    Ok(())
}

/// `lingxia auth login lingxia`: store the token in the wallet, keyed by the
/// canonical server URL + env, so publish finds it from the server and env
/// alone. `--server` also saves the machine-wide server default.
pub fn publish_login(
    server: Option<String>,
    token: Option<String>,
    env: Option<String>,
) -> Result<()> {
    let server = clean_arg(server, "--server")?;
    if let Some(server) = server.as_deref() {
        validate_publish_server(server)?;
    }
    let env = env
        .as_deref()
        .map(AppEnv::parse_cli)
        .transpose()?
        .unwrap_or(AppEnv::Dev);

    // The token is keyed by the server: an explicit --server wins, otherwise
    // the machine default for this env names it.
    let server_url = resolve_lingxia_server(env, server.clone())
        .context("cannot determine which server this token is for; pass --server")?;
    let canonical = crate::wallet::canonical_publish_server(&server_url)?;

    let token = match clean_arg(token, "--token")? {
        Some(token) => token,
        None => dialoguer::Password::new()
            .with_prompt(format!("Publish token for {canonical} ({})", env.as_str()))
            .interact()?,
    };

    let wallet = crate::wallet::Wallet::open()?;
    if let Some(old) = wallet.load_publish_token(&canonical, env.as_str())?
        && old != token
    {
        println!(
            "{} Rotating the stored token: {} -> {}",
            "ℹ".blue(),
            mask_token(&old),
            mask_token(&token)
        );
    }
    let path = wallet.save_publish_token(&canonical, env.as_str(), &token)?;

    // Persist an explicitly given server as the machine-wide default too.
    if let Some(server) = server {
        let mut config = CliConfig::load()?;
        config.set_publish_server(Some(env), server);
        config.save()?;
    }

    println!("{} Saved publish token.", "✓".green().bold());
    println!("   Server: {canonical}");
    println!("   Env:    {}", env.as_str());
    println!("   Token:  {}", mask_token(&token));
    println!("   Slot:   {}", path.display());
    Ok(())
}

/// `lingxia auth logout lingxia`: remove the stored token for (server, env).
pub fn publish_logout(server: Option<String>, env: Option<String>) -> Result<()> {
    let env = env
        .as_deref()
        .map(AppEnv::parse_cli)
        .transpose()?
        .unwrap_or(AppEnv::Dev);
    let server_url = resolve_lingxia_server(env, server)
        .context("cannot determine which server to log out from; pass --server")?;
    let canonical = crate::wallet::canonical_publish_server(&server_url)?;
    let wallet = crate::wallet::Wallet::open()?;
    if wallet.delete_publish_token(&canonical, env.as_str())? {
        println!(
            "{} Removed the publish token for {canonical} ({}).",
            "✓".green(),
            env.as_str()
        );
    } else {
        println!(
            "{} No publish token stored for {canonical} ({}).",
            "ℹ".blue(),
            env.as_str()
        );
    }
    Ok(())
}

/// Trim an optional arg, rejecting a present-but-empty value.
fn clean_arg(value: Option<String>, flag: &str) -> Result<Option<String>> {
    match value {
        None => Ok(None),
        Some(v) => {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                bail!("{flag} cannot be empty");
            }
            Ok(Some(trimmed.to_string()))
        }
    }
}

/// A publish server must be an absolute http(s) URL. The upload path is
/// `/api/v2/lingxia/{lingxiaId}/package` or `/api/v2/lxapp/{appId}/package`.
fn validate_publish_server(server: &str) -> Result<()> {
    if !(server.starts_with("http://") || server.starts_with("https://")) {
        bail!("--server must be an http(s) URL (got '{server}')");
    }
    Ok(())
}

/// Show only the first/last few characters of a token in logs.
fn mask_token(token: &str) -> String {
    let chars: Vec<char> = token.chars().collect();
    if chars.len() <= 8 {
        return "*".repeat(chars.len());
    }
    let head: String = chars[..4].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}…{tail}")
}

/// A given package is a host package and carries its own identity; without
/// one, the lxapp or lxplugin in `cwd` is built and its archive published.
fn resolve_publish_target(
    cwd: &Path,
    opts: &PublishOptions,
) -> Result<(PackageMeta, ResolvedPackage)> {
    if let Some(package) = opts.package.as_deref() {
        if opts.channel.is_some() {
            bail!(
                "--channel is not supported for a host package; host updates are env-scoped and carry no channel"
            );
        }
        let path = host_package_path(&cwd.join(package))?;
        let (platform, metadata) = read_host_package(&path)?;
        reject_env_mismatch(opts.env.as_deref(), &path, metadata.env)?;
        let meta = PackageMeta {
            target: "app".to_string(),
            target_id: metadata.lingxia_id,
            version: metadata.version,
            env: metadata.env,
            channel: None,
            min_runtime: String::new(),
        };
        return Ok((
            meta,
            ResolvedPackage {
                path,
                platform: Some(platform.to_string()),
                cleanup_after_publish: false,
            },
        ));
    }
    if opts.dry_run {
        bail!("--dry-run takes a host package: lingxia publish <PACKAGE|DIR> --dry-run");
    }
    let mut meta = resolve_meta(cwd, opts.env.as_deref(), opts.channel.as_deref())?;
    let package = package_current_project(cwd, opts.framework.clone(), opts.progress.clone())?;
    apply_packaged_manifest(&mut meta, &package.path)?;
    Ok((meta, package))
}

fn package_current_project(
    cwd: &Path,
    framework: Option<String>,
    progress: Option<String>,
) -> Result<ResolvedPackage> {
    let args = publish_build_args(framework.as_deref(), progress.as_deref());
    lxapp::run_in_dir(&args, cwd)?;
    Ok(ResolvedPackage {
        path: lxapp::package_in_dir(cwd, framework.as_deref())?,
        platform: None,
        cleanup_after_publish: true,
    })
}

fn publish_build_args(framework: Option<&str>, progress: Option<&str>) -> Vec<String> {
    let mut args = vec!["build".to_string(), "--release".to_string()];
    if let Some(framework) = framework {
        args.push("--framework".to_string());
        args.push(framework.to_string());
    }
    if let Some(progress) = progress {
        args.push("--progress".to_string());
        args.push(progress.to_string());
    }
    args
}

fn resolve_meta(
    cwd: &Path,
    env_arg: Option<&str>,
    channel_arg: Option<&str>,
) -> Result<PackageMeta> {
    let env = env_arg
        .map(AppEnv::parse_cli)
        .transpose()?
        .unwrap_or(AppEnv::Dev);
    let channel = Some(match channel_arg {
        Some(value) => normalize_channel(value)?,
        None => lingxia_update::default_channel().as_str().to_string(),
    });
    if cwd.join("lxapp.json").exists() {
        let (id, version, min_runtime) = read_lxapp_json(cwd)?;
        return Ok(PackageMeta {
            target: "lxapp".to_string(),
            target_id: id,
            version,
            env,
            channel,
            min_runtime,
        });
    }
    if cwd.join("lxplugin.json").exists() {
        let (id, version) = read_lxplugin_json(cwd)?;
        return Ok(PackageMeta {
            target: "lxplugin".to_string(),
            target_id: id,
            version,
            env,
            channel,
            min_runtime: String::new(),
        });
    }
    if has_host_config(cwd) {
        bail!(
            "A host app publishes the package `lingxia package` wrote: lingxia publish <PACKAGE>"
        );
    }
    bail!(
        "No lxapp.json or lxplugin.json in {}. Run publish from an lxapp or lxplugin project, or pass a host package: lingxia publish <PACKAGE>",
        cwd.display()
    )
}

fn normalize_channel(s: &str) -> Result<String> {
    match s {
        "release" | "draft" => Ok(s.to_string()),
        "dev" => bail!("'dev' is a host env, not an lxapp channel; use --channel draft"),
        "prod" => bail!("'prod' is a host env, not an lxapp channel; use --channel release"),
        "preview" => {
            bail!("'preview' is not an lxapp channel; use --channel draft or --channel release")
        }
        "developer" | "develop" => bail!("invalid channel '{s}'; use draft"),
        other => bail!("invalid channel '{other}'; must be one of: release, draft"),
    }
}

#[derive(Debug)]
struct AppPackageMetadata {
    env: AppEnv,
    /// The runtime resolves updates against this exact packaged id.
    lingxia_id: String,
    /// `productVersion` baked into the package. Clients compare this string.
    version: String,
}

/// A host package is built for one env and publishes only there; an `--env`
/// that disagrees would send it to the wrong server.
fn reject_env_mismatch(given: Option<&str>, package: &Path, packaged: AppEnv) -> Result<()> {
    let Some(given) = given.map(AppEnv::parse_cli).transpose()? else {
        return Ok(());
    };
    if given != packaged {
        bail!(
            "{} was packaged for {packaged}, so it publishes to {packaged}. Drop --env, or run `lingxia package --env {given}` first.",
            package.display()
        );
    }
    Ok(())
}

/// A file is the package itself; a directory is a `lingxia package` output,
/// whose manifest names (and checksums) its update payload.
fn host_package_path(path: &Path) -> Result<PathBuf> {
    if path.is_file() {
        return Ok(path.to_path_buf());
    }
    if !path.is_dir() {
        bail!("Package not found: {}", path.display());
    }
    let manifest = crate::dist_manifest::read(path)?;
    let formats: &[&str] = match manifest.platform.as_str() {
        "android" => &["apk"],
        "macos" | "windows" => &["update"],
        other => bail!("{other} host apps update through their store; use `lingxia store submit`"),
    };
    Ok(crate::dist_manifest::resolve(path, formats)?.1)
}

/// Identify a host package by its layout, never its file name: each layout
/// `lingxia package` writes is distinct, and anything else is refused.
fn read_host_package(path: &Path) -> Result<(&'static str, AppPackageMetadata)> {
    let not_host = || {
        anyhow::anyhow!(
            "{} is not a host package from `lingxia package` (an Android APK, or a macOS/Windows update zip)",
            path.display()
        )
    };
    let file =
        fs::File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| not_host())?;
    // PowerShell `Compress-Archive` stores backslash-separated names.
    let names: Vec<(String, String)> = zip
        .file_names()
        .map(|name| (name.replace('\\', "/"), name.to_string()))
        .collect();
    let normalized: Vec<&str> = names.iter().map(|(name, _)| name.as_str()).collect();
    let (platform, app_json) = classify_host_package(&normalized).ok_or_else(not_host)?;
    let original = names
        .iter()
        .find(|(name, _)| *name == app_json)
        .map(|(_, original)| original.clone())
        .ok_or_else(not_host)?;
    let mut data = Vec::new();
    zip.by_name(&original)
        .with_context(|| format!("Failed to read {app_json} from {}", path.display()))?
        .read_to_end(&mut data)
        .with_context(|| format!("Failed to read {app_json} from {}", path.display()))?;
    Ok((platform, parse_app_json(&data)?))
}

/// The platform and the `app.json` entry of a host package layout; `None`
/// unless exactly one layout matches.
fn classify_host_package(names: &[&str]) -> Option<(&'static str, String)> {
    let has = |name: &str| names.contains(&name);
    let mut found = Vec::new();
    if has("AndroidManifest.xml") && has("assets/app.json") {
        found.push(("android", "assets/app.json".to_string()));
    }
    for name in names {
        if let Some(bundle) = name.strip_suffix("/Contents/Info.plist")
            && bundle.ends_with(".app")
            && !bundle.contains('/')
        {
            let app_json = format!("{bundle}/Contents/Resources/app.json");
            if has(&app_json) {
                found.push(("macos", app_json));
            }
        }
    }
    // The install directory, at the root or under one top folder.
    for name in names {
        if let Some(stem) = name.strip_suffix(".exe") {
            let dir = stem
                .rsplit_once('/')
                .map(|(dir, _)| format!("{dir}/"))
                .unwrap_or_default();
            let app_json = format!("{dir}assets/app.json");
            if dir.matches('/').count() <= 1
                && has(&app_json)
                && !found
                    .iter()
                    .any(|(platform, json)| *platform == "windows" && *json == app_json)
            {
                found.push(("windows", app_json));
            }
        }
    }
    match found.as_slice() {
        [(platform, app_json)] => Some((platform, app_json.clone())),
        _ => None,
    }
}

fn parse_app_json(data: &[u8]) -> Result<AppPackageMetadata> {
    let value: serde_json::Value =
        serde_json::from_slice(data).context("Failed to parse app.json in package")?;
    let env = value
        .get("env")
        .and_then(|value| value.as_str())
        .map(AppEnv::parse_cli)
        .transpose()?
        .unwrap_or(AppEnv::Prod);
    let lingxia_id = value
        .get("lingxiaId")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .context("lingxiaId is missing from packaged app.json")?
        .to_string();
    let version = value
        .get("productVersion")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .context("productVersion is missing from packaged app.json")?;
    lingxia_update::Version::parse(version).map_err(|_| {
        anyhow::anyhow!(
            "productVersion in packaged app.json must be a semantic version (major.minor.patch)"
        )
    })?;
    Ok(AppPackageMetadata {
        env,
        lingxia_id,
        version: version.to_string(),
    })
}

fn read_lxapp_json(cwd: &Path) -> Result<(String, String, String)> {
    parse_lxapp_manifest(&read_project_manifest(cwd, "lxapp.json")?)
}

fn read_lxplugin_json(cwd: &Path) -> Result<(String, String)> {
    parse_lxplugin_manifest(&read_project_manifest(cwd, "lxplugin.json")?)
}

fn read_project_manifest(cwd: &Path, name: &str) -> Result<serde_json::Value> {
    let path = cwd.join(name);
    if !path.exists() {
        bail!("{name} not found in {}", cwd.display());
    }
    serde_json::from_str(&fs::read_to_string(&path)?)
        .with_context(|| format!("Failed to parse {name}"))
}

fn parse_lxapp_manifest(val: &serde_json::Value) -> Result<(String, String, String)> {
    let id = non_empty_str(&val["appId"], "appId in lxapp.json")?;
    let version = non_empty_str(&val["version"], "version in lxapp.json")?;
    let min_runtime = crate::versions::lxapp_min_runtime(val)?;
    Ok((id, version, min_runtime))
}

fn parse_lxplugin_manifest(val: &serde_json::Value) -> Result<(String, String)> {
    let id = non_empty_str(&val["lxPluginId"], "lxPluginId in lxplugin.json")?;
    let version = non_empty_str(&val["version"], "version in lxplugin.json")?;
    Ok((id, version))
}

/// The uploader vouches for what it uploads: id, version, and minRuntime come
/// from the manifest inside the archive, and a project that disagrees with its
/// own build stops the publish.
fn apply_packaged_manifest(meta: &mut PackageMeta, package: &Path) -> Result<()> {
    let (id, version, min_runtime) = match meta.target.as_str() {
        "lxapp" => parse_lxapp_manifest(&read_archive_manifest(package, "lxapp.json")?)?,
        "lxplugin" => {
            let (id, version) =
                parse_lxplugin_manifest(&read_archive_manifest(package, "lxplugin.json")?)?;
            (id, version, String::new())
        }
        _ => return Ok(()),
    };
    if (&id, &version, &min_runtime) != (&meta.target_id, &meta.version, &meta.min_runtime) {
        bail!(
            "{} holds {} v{} (minRuntime {}), but the project declares {} v{} (minRuntime {}); rebuild before publishing",
            package.display(),
            id,
            version,
            display_or_none(&min_runtime),
            meta.target_id,
            meta.version,
            display_or_none(&meta.min_runtime)
        );
    }
    Ok(())
}

fn display_or_none(value: &str) -> &str {
    if value.is_empty() { "none" } else { value }
}

fn read_archive_manifest(package: &Path, name: &str) -> Result<serde_json::Value> {
    let file = fs::File::open(package)
        .with_context(|| format!("Failed to open package {}", package.display()))?;
    let decoder = zstd::stream::read::Decoder::new(file)
        .with_context(|| format!("Failed to read package {}", package.display()))?;
    let mut archive = tar::Archive::new(decoder);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        let normalized: PathBuf = path
            .components()
            .filter(|component| !matches!(component, std::path::Component::CurDir))
            .collect();
        if normalized == Path::new(name) {
            let mut text = String::new();
            entry
                .read_to_string(&mut text)
                .with_context(|| format!("Failed to read {name} from {}", package.display()))?;
            return serde_json::from_str(&text)
                .with_context(|| format!("Failed to parse {name} in {}", package.display()));
        }
    }
    bail!("{name} is missing from package {}", package.display())
}

fn non_empty_str(val: &serde_json::Value, label: &str) -> Result<String> {
    let s = val.as_str().unwrap_or("").trim().to_string();
    if s.is_empty() {
        bail!("{label} is missing or empty");
    }
    Ok(s)
}

/// Resolve the bearer token: `--token` flag → `LINGXIA_PUBLISH_TOKEN` →
/// wallet slot keyed by (canonical server URL, env) → error.
fn resolve_token(env: AppEnv, server: &str, token_arg: Option<String>) -> Result<String> {
    if let Some(t) = token_arg {
        let trimmed = t.trim();
        if trimmed.is_empty() {
            bail!("--token cannot be empty");
        }
        return Ok(trimmed.to_string());
    }
    if let Ok(token) = env::var("LINGXIA_PUBLISH_TOKEN")
        && !token.trim().is_empty()
    {
        return Ok(token.trim().to_string());
    }
    let canonical = crate::wallet::canonical_publish_server(server)?;
    if let Some(token) =
        crate::wallet::Wallet::open()?.load_publish_token(&canonical, env.as_str())?
    {
        return Ok(token);
    }
    bail!(
        "No LingXia publish token for {canonical} ({}). Fix: lingxia auth login lingxia --env {} --token <token>",
        env.as_str(),
        env.as_str()
    );
}

fn publish_upload(
    server: &str,
    meta: &PackageMeta,
    platform: Option<&str>,
    sha256: &str,
    extra: &[(String, String)],
) -> Result<(String, Vec<(String, String)>)> {
    let server = server.trim_end_matches('/');
    let id = urlencoding::encode(meta.target_id.trim());
    let url = match meta.target.as_str() {
        "app" => {
            let platform = platform.context("host package platform is unknown")?;
            format!(
                "{server}/api/v2/lingxia/{id}/package?platform={}",
                urlencoding::encode(platform)
            )
        }
        "lxapp" | "lxplugin" => {
            let channel = meta
                .channel
                .as_deref()
                .context("lxapp publish requires a channel")?;
            format!(
                "{server}/api/v2/lxapp/{id}/package?channel={}",
                urlencoding::encode(channel)
            )
        }
        other => bail!("unknown publish target: {other}"),
    };
    let mut fields = vec![
        ("version".to_string(), meta.version.clone()),
        ("sha256".to_string(), sha256.to_string()),
    ];
    if !meta.min_runtime.is_empty() {
        fields.push(("minRuntime".to_string(), meta.min_runtime.clone()));
    }
    fields.extend(extra.iter().cloned());
    Ok((url, fields))
}

/// The upload server is the publisher's choice: `--lingxia-server`, else the
/// default `lingxia auth login lingxia --server` saved. Packages and project
/// files never pick it.
fn resolve_lingxia_server(env: AppEnv, lingxia_server_arg: Option<String>) -> Result<String> {
    if let Some(s) = lingxia_server_arg {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            bail!("--lingxia-server cannot be empty");
        }
        return Ok(trimmed.to_string());
    }
    if let Some(url) = global_lingxia_server(env) {
        return Ok(url);
    }
    bail!("Use --lingxia-server to specify the package upload server URL.");
}

/// `[publish]` server from `~/.lingxia/cli/config.toml`, routed by env
/// (defaults to `dev` when absent); `--lingxia-server` wins.
fn global_lingxia_server(env: AppEnv) -> Option<String> {
    let publish = CliConfig::load().ok()?.publish?;
    publish.lingxia_server_for(env).map(str::to_string)
}

fn upload_transport_error(url: &str, package_bytes: usize, err: ureq::Error) -> anyhow::Error {
    let message = err.to_string();
    let lower = message.to_ascii_lowercase();
    let size_mib = package_bytes as f64 / 1024.0 / 1024.0;

    if lower.contains("broken pipe")
        || lower.contains("connection reset")
        || lower.contains("connection reset by peer")
    {
        return anyhow::anyhow!(
            "HTTP request failed: {url}\n\
             Transport error: {message}\n\
             The server closed the connection while receiving a {size_mib:.1} MiB package.\n\
             Check the upload server or gateway request-body limit, then retry. For cloud-mockd, restart the updated mock server."
        );
    }

    anyhow::anyhow!("HTTP request failed: {url}\nTransport error: {message}")
}

fn signed_multipart_fields(
    env: AppEnv,
    key: Option<&str>,
    req: &lingxia_update::SignRequest<'_>,
) -> Result<Vec<(String, String)>> {
    let env = match env {
        AppEnv::Dev => lingxia_app_context::AppEnv::Dev,
        AppEnv::Prod => lingxia_app_context::AppEnv::Prod,
    };
    match lingxia_update::sign_package_from_key(env, key, req)
        .map_err(|e| anyhow::anyhow!("{e}"))?
    {
        None => Ok(Vec::new()),
        Some(auth) => Ok(vec![
            ("signed".to_string(), auth.signed),
            (
                "signatures".to_string(),
                serde_json::to_string(&auth.signatures).context("encode update signatures")?,
            ),
        ]),
    }
}

fn rand_hex() -> String {
    use rand::RngExt;
    let bytes: [u8; 8] = rand::rng().random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn build_multipart(
    boundary: &str,
    fields: &[(&str, &str)],
    file_name: &str,
    file_data: &[u8],
) -> Vec<u8> {
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"package\"; filename=\"{file_name}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(file_data);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

#[cfg(test)]
mod tests {
    use super::{
        AppPackageMetadata, PackageMeta, PublishOptions, apply_packaged_manifest, build_multipart,
        classify_host_package, host_package_path, normalize_channel, publish_build_args,
        publish_upload, read_host_package, reject_env_mismatch, resolve_meta,
        resolve_publish_target, signed_multipart_fields,
    };
    use super::{draft_open_url, draft_qr_png};
    use crate::config::AppEnv;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;
    use zip::write::SimpleFileOptions;

    use super::{clean_arg, mask_token, validate_publish_server};

    #[test]
    fn validate_publish_server_requires_http_scheme() {
        assert!(validate_publish_server("https://api.example.com").is_ok());
        assert!(validate_publish_server("http://localhost:8080").is_ok());
        assert!(validate_publish_server("api.example.com").is_err());
        assert!(validate_publish_server("ftp://x").is_err());
    }

    #[test]
    fn clean_arg_rejects_present_but_empty() {
        assert!(clean_arg(None, "--server").unwrap().is_none());
        assert_eq!(
            clean_arg(Some("  x ".to_string()), "--server").unwrap(),
            Some("x".to_string())
        );
        assert!(clean_arg(Some("   ".to_string()), "--token").is_err());
    }

    #[test]
    fn mask_token_hides_the_middle() {
        assert_eq!(mask_token("lx_abcdefgh"), "lx_a…efgh");
        assert_eq!(mask_token("short"), "*****");
    }

    #[test]
    fn build_multipart_includes_text_fields() {
        let body = build_multipart(
            "boundary",
            &[("version", "1.0.0"), ("sha256", "abc")],
            "app-release.apk",
            b"apk",
        );
        let body = String::from_utf8(body).unwrap();
        assert!(body.contains("name=\"version\"\r\n\r\n1.0.0"));
        assert!(body.contains("name=\"sha256\"\r\n\r\nabc"));
    }

    #[test]
    fn publish_upload_puts_identity_in_the_path() {
        let host = PackageMeta {
            target: "app".into(),
            target_id: "host app".into(),
            version: "1.2.0".into(),
            env: AppEnv::Prod,
            channel: None,
            min_runtime: String::new(),
        };
        let (url, fields) =
            publish_upload("https://api.example.com/", &host, Some("macos"), "abc", &[]).unwrap();
        assert_eq!(
            url,
            "https://api.example.com/api/v2/lingxia/host%20app/package?platform=macos"
        );
        assert_eq!(
            fields,
            vec![
                ("version".to_string(), "1.2.0".to_string()),
                ("sha256".to_string(), "abc".to_string()),
            ]
        );

        let lxapp = PackageMeta {
            target: "lxapp".into(),
            target_id: "shop/home".into(),
            version: "1.0.1".into(),
            env: AppEnv::Dev,
            channel: Some("draft".into()),
            min_runtime: "0.17.0".into(),
        };
        let (url, fields) =
            publish_upload("https://api.example.com", &lxapp, None, "def", &[]).unwrap();
        assert_eq!(
            url,
            "https://api.example.com/api/v2/lxapp/shop%2Fhome/package?channel=draft"
        );
        assert_eq!(
            field_names(&fields),
            vec!["version", "sha256", "minRuntime"]
        );
        assert!(publish_upload("https://api.example.com", &host, None, "abc", &[]).is_err());
    }

    fn field_names(fields: &[(String, String)]) -> Vec<&str> {
        fields.iter().map(|(name, _)| name.as_str()).collect()
    }

    fn sign_request(sha256: &str) -> lingxia_update::SignRequest<'_> {
        lingxia_update::SignRequest {
            kind: "lxapp",
            target_id: "shop",
            channel: "release",
            platform: "any",
            version: "1.0.1",
            sha256,
        }
    }

    #[test]
    fn signed_multipart_fields_omits_envelope_for_dev_without_key() {
        let sha256 = lingxia_update::archive_sha256_hex(b"pkg");
        let extra = signed_multipart_fields(AppEnv::Dev, None, &sign_request(&sha256)).unwrap();
        assert!(extra.is_empty());
    }

    #[test]
    fn signed_multipart_fields_requires_key_for_prod() {
        let sha256 = lingxia_update::archive_sha256_hex(b"pkg");
        let err = signed_multipart_fields(AppEnv::Prod, None, &sign_request(&sha256)).unwrap_err();
        assert!(err.to_string().contains("--update-signing-key"), "{err}");
    }

    #[test]
    fn signed_multipart_policy_follows_env_for_every_channel() {
        let sha256 = lingxia_update::archive_sha256_hex(b"pkg");
        for channel in ["release", "draft", ""] {
            let mut req = sign_request(&sha256);
            req.channel = channel;
            if channel.is_empty() {
                req.kind = "app";
                req.platform = "windows";
            }
            assert!(
                signed_multipart_fields(AppEnv::Dev, None, &req)
                    .unwrap()
                    .is_empty()
            );
            let err = signed_multipart_fields(AppEnv::Prod, None, &req).unwrap_err();
            assert!(err.to_string().contains("prod publish"), "{channel}: {err}");
        }
    }

    #[test]
    fn signed_multipart_fields_are_uploaded_verbatim() {
        let package = b"pkg";
        let sha256 = lingxia_update::archive_sha256_hex(package);
        let extra = signed_multipart_fields(
            AppEnv::Prod,
            Some("BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc"),
            &sign_request(&sha256),
        )
        .unwrap();
        // No scheme field: the envelope does not name its own algorithm.
        assert_eq!(extra[0].0, "signed");
        assert_eq!(extra[1].0, "signatures");
        assert_eq!(extra.len(), 2);
        let refs: Vec<(&str, &str)> = extra
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        let body =
            String::from_utf8(build_multipart("boundary", &refs, "shop.tar.zst", package)).unwrap();
        assert!(!body.contains("name=\"scheme\""));
        assert!(body.contains("name=\"signed\""));
        assert!(body.contains("name=\"signatures\""));
        assert!(body.contains(&extra[0].1));
    }

    #[test]
    fn lxapp_publish_channel_can_be_selected() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("lxapp.json"),
            br#"{"appId":"demo","version":"1.0.0","minRuntime":"0.17.0","pages":["index.html"]}"#,
        )
        .unwrap();

        let meta = resolve_meta(temp.path(), None, Some("draft")).unwrap();

        assert_eq!(meta.target, "lxapp");
        assert_eq!(meta.env, AppEnv::Dev);
        assert_eq!(meta.channel.as_deref(), Some("draft"));
        assert_eq!(meta.min_runtime, "0.17.0");
    }

    #[test]
    fn lxapp_publish_defaults_to_release_in_every_env() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("lxapp.json"),
            br#"{"appId":"demo","version":"1.0.0","minRuntime":"0.17.0","pages":["index.html"]}"#,
        )
        .unwrap();

        let meta = resolve_meta(temp.path(), None, None).unwrap();

        assert_eq!(meta.target, "lxapp");
        assert_eq!(meta.env, AppEnv::Dev);
        assert_eq!(meta.channel.as_deref(), Some("release"));

        let prod = resolve_meta(temp.path(), Some("prod"), None).unwrap();
        assert_eq!(prod.env, AppEnv::Prod);
        assert_eq!(prod.channel.as_deref(), Some("release"));
    }

    #[test]
    fn draft_link_uses_publish_origin_and_encodes_appid() {
        let url = draft_open_url(
            "https://api.example.com:8443/service?old=1#fragment",
            "shop/home &x=1",
        )
        .unwrap();
        let url = url::Url::parse(&url).unwrap();
        assert_eq!(
            url.origin().ascii_serialization(),
            "https://api.example.com:8443"
        );
        assert_eq!(url.path(), "/lxapp/open");
        assert_eq!(url.fragment(), None);
        assert_eq!(
            url.query_pairs().collect::<Vec<_>>(),
            vec![
                ("appId".into(), "shop/home &x=1".into()),
                ("channel".into(), "draft".into()),
            ]
        );
        assert!(draft_open_url("http://localhost:8080", "shop").is_err());
        assert!(draft_open_url("https://user:secret@api.example.com", "shop").is_err());
    }

    #[test]
    fn draft_qr_decodes_to_the_publish_link() {
        let url = draft_open_url("https://api.example.com:8443", "shop/home &x=1").unwrap();
        let code = qrcode::QrCode::new(url.as_bytes()).unwrap();
        let png = draft_qr_png(&code).unwrap();
        let image = image::load_from_memory(&png).unwrap().to_luma8();
        let (width, height) = image.dimensions();
        let decoded =
            rxing::helpers::detect_in_luma(image.into_raw(), width, height, None).unwrap();
        assert_eq!(decoded.getText(), url);
    }

    #[test]
    fn publish_channel_rejects_env_names() {
        assert!(normalize_channel("dev").is_err());
        assert!(normalize_channel("developer").is_err());
        assert!(normalize_channel("develop").is_err());
        assert!(normalize_channel("preview").is_err());
    }

    #[test]
    fn publish_build_does_not_pass_env_or_channel() {
        let args = publish_build_args(Some("react"), Some("plain"));

        assert_eq!(
            args,
            vec![
                "build",
                "--release",
                "--framework",
                "react",
                "--progress",
                "plain"
            ]
        );
    }

    #[test]
    fn a_host_project_publishes_its_package_not_itself() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("lingxia.yaml"), "app: {}\n").unwrap();
        let err = resolve_meta(temp.path(), None, None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("lingxia publish <PACKAGE>"), "{err}");
    }

    fn options(package: Option<String>) -> PublishOptions {
        PublishOptions {
            token: None,
            lingxia_server: None,
            package,
            env: None,
            channel: None,
            framework: None,
            progress: None,
            update_signing_key: None,
            dry_run: false,
        }
    }

    #[test]
    fn a_host_package_publishes_from_anywhere_with_its_own_identity() {
        let temp = TempDir::new().unwrap();
        let package = write_macos_package(temp.path(), "0.2.6");
        let (meta, resolved) =
            resolve_publish_target(temp.path(), &options(Some(package.display().to_string())))
                .unwrap();
        assert_eq!(meta.target, "app");
        assert_eq!(meta.target_id, "demo");
        assert_eq!(meta.version, "0.2.6");
        assert_eq!(meta.env, AppEnv::Prod);
        assert_eq!(resolved.platform.as_deref(), Some("macos"));

        let mut with_channel = options(Some(package.display().to_string()));
        with_channel.channel = Some("draft".into());
        let err = resolve_publish_target(temp.path(), &with_channel)
            .err()
            .unwrap()
            .to_string();
        assert!(
            err.contains("--channel is not supported for a host package"),
            "{err}"
        );
    }

    #[test]
    fn a_package_directory_publishes_its_recorded_update_payload() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("dist/macos");
        fs::create_dir_all(&dir).unwrap();
        let update = write_macos_package(&dir, "0.2.6");
        let dmg = dir.join("Flourish-0.2.6.dmg");
        fs::write(&dmg, b"dmg").unwrap();
        // An older build left beside it is not what was packaged.
        write_macos_package(&dir, "0.2.5");
        crate::dist_manifest::write(
            &dir,
            "macos",
            "0.2.6",
            "prod",
            Some("demo"),
            &[
                crate::dist_manifest::Produced {
                    format: "update",
                    path: update.clone(),
                },
                crate::dist_manifest::Produced {
                    format: "dmg",
                    path: dmg,
                },
            ],
        )
        .unwrap();
        assert_eq!(host_package_path(&dir).unwrap(), update);

        let mut dry_run = options(Some(dir.display().to_string()));
        dry_run.dry_run = true;
        let (meta, resolved) = resolve_publish_target(temp.path(), &dry_run).unwrap();
        assert_eq!(meta.version, "0.2.6");
        assert_eq!(resolved.path, update);

        let ios = temp.path().join("dist/ios");
        fs::create_dir_all(&ios).unwrap();
        let ipa = ios.join("Demo.ipa");
        fs::write(&ipa, b"ipa").unwrap();
        crate::dist_manifest::write(
            &ios,
            "ios",
            "0.2.6",
            "prod",
            None,
            &[crate::dist_manifest::Produced {
                format: "ipa",
                path: ipa,
            }],
        )
        .unwrap();
        let err = host_package_path(&ios).unwrap_err().to_string();
        assert!(err.contains("lingxia store submit"), "{err}");
    }

    #[test]
    fn dry_run_needs_a_host_package() {
        let temp = TempDir::new().unwrap();
        let mut dry_run = options(None);
        dry_run.dry_run = true;
        let err = resolve_publish_target(temp.path(), &dry_run)
            .err()
            .unwrap()
            .to_string();
        assert!(err.contains("--dry-run takes a host package"), "{err}");
    }

    #[test]
    fn host_packages_are_identified_by_layout_not_name() {
        assert_eq!(
            classify_host_package(&["AndroidManifest.xml", "classes.dex", "assets/app.json"]),
            Some(("android", "assets/app.json".to_string()))
        );
        assert_eq!(
            classify_host_package(&[
                "Demo.app/Contents/Info.plist",
                "Demo.app/Contents/MacOS/demo",
                "Demo.app/Contents/Resources/app.json",
            ]),
            Some(("macos", "Demo.app/Contents/Resources/app.json".to_string()))
        );
        assert_eq!(
            classify_host_package(&["demo.exe", "assets/app.json"]),
            Some(("windows", "assets/app.json".to_string()))
        );
        assert_eq!(
            classify_host_package(&["Demo/demo.exe", "Demo/assets/app.json"]),
            Some(("windows", "Demo/assets/app.json".to_string()))
        );
        // Neither a stray app.json nor an exe without its assets is a package.
        assert_eq!(classify_host_package(&["assets/app.json"]), None);
        assert_eq!(classify_host_package(&["demo.exe", "README.txt"]), None);
        // Two layouts at once is not a package `lingxia package` writes.
        assert_eq!(
            classify_host_package(&["AndroidManifest.xml", "assets/app.json", "demo.exe"]),
            None
        );
    }

    #[test]
    fn a_misnamed_package_is_read_by_what_it_contains() {
        let temp = TempDir::new().unwrap();
        let mac = write_macos_package(temp.path(), "0.2.6");
        let renamed = temp.path().join("Demo-0.2.6-windows.zip");
        fs::rename(&mac, &renamed).unwrap();
        assert_eq!(read_host_package(&renamed).unwrap().0, "macos");

        // PowerShell `Compress-Archive` writes backslash-separated names.
        let windows = temp.path().join("Demo.zip");
        write_zip(
            &windows,
            &[
                ("Demo\\demo.exe", b""),
                (
                    "Demo\\assets\\app.json",
                    br#"{"lingxiaId":"demo","productVersion":"1.0.0","env":"dev"}"#,
                ),
            ],
        );
        let (platform, metadata) = read_host_package(&windows).unwrap();
        assert_eq!(platform, "windows");
        assert_eq!(metadata.env, AppEnv::Dev);

        let other = temp.path().join("notes.zip");
        write_zip(&other, &[("notes.txt", b"hi")]);
        let err = read_host_package(&other).unwrap_err().to_string();
        assert!(err.contains("is not a host package"), "{err}");
    }

    fn read_app_package_metadata(path: &std::path::Path) -> anyhow::Result<AppPackageMetadata> {
        read_host_package(path).map(|(_, metadata)| metadata)
    }

    #[test]
    fn publish_reads_env_from_android_app_json() {
        let temp = TempDir::new().unwrap();
        let apk = temp.path().join("app-dev.apk");
        write_zip(
            &apk,
            &[("AndroidManifest.xml", b""), (
                "assets/app.json",
                br#"{"productName":"Demo","productVersion":"1.0.0","homeAppId":"demo","homeAppVersion":"1.0.0","env":"dev","lingxiaId":"demo.dev"}"#,
            )],
        );

        let metadata = read_app_package_metadata(&apk).unwrap();

        assert_eq!(metadata.env, AppEnv::Dev);
        assert_eq!(metadata.lingxia_id, "demo.dev");
        assert_eq!(metadata.version, "1.0.0");
    }

    #[test]
    fn publish_reads_env_from_macos_app_json() {
        let temp = TempDir::new().unwrap();
        let zip = temp.path().join("Demo-1.0.0-macos.zip");
        write_zip(
            &zip,
            &[("Demo.app/Contents/Info.plist", b""), (
                "Demo.app/Contents/Resources/app.json",
                br#"{"productName":"Demo","productVersion":"1.0.0","homeAppId":"demo","homeAppVersion":"1.0.0","env":"prod","lingxiaId":"demo"}"#,
            )],
        );

        let metadata = read_app_package_metadata(&zip).unwrap();

        assert_eq!(metadata.env, AppEnv::Prod);
        assert_eq!(metadata.version, "1.0.0");
    }

    #[test]
    fn missing_app_json_env_defaults_to_prod() {
        let temp = TempDir::new().unwrap();
        let apk = temp.path().join("app.apk");
        write_zip(
            &apk,
            &[("AndroidManifest.xml", b""), (
                "assets/app.json",
                br#"{"productName":"Demo","productVersion":"1.0.0","homeAppId":"demo","homeAppVersion":"1.0.0","lingxiaId":"demo"}"#,
            )],
        );

        let metadata = read_app_package_metadata(&apk).unwrap();
        assert_eq!(metadata.env, AppEnv::Prod);
    }

    #[test]
    fn publish_picks_up_suffixed_lingxia_id_from_app_package() {
        let temp = TempDir::new().unwrap();
        let apk = temp.path().join("app-dev.apk");
        write_zip(
            &apk,
            &[("AndroidManifest.xml", b""), (
                "assets/app.json",
                br#"{"productName":"Demo","productVersion":"1.0.0","homeAppId":"demo","homeAppVersion":"1.0.0","env":"dev","lingxiaId":"demo.dev"}"#,
            )],
        );

        let metadata = read_app_package_metadata(&apk).unwrap();

        assert_eq!(metadata.env, AppEnv::Dev);
        assert_eq!(metadata.lingxia_id, "demo.dev");
    }

    #[test]
    fn publish_rejects_app_package_without_lingxia_id() {
        let temp = TempDir::new().unwrap();
        let apk = temp.path().join("app.apk");
        write_zip(
            &apk,
            &[
                ("AndroidManifest.xml", b""),
                ("assets/app.json", br#"{"env":"prod"}"#),
            ],
        );

        let error = read_app_package_metadata(&apk).unwrap_err().to_string();
        assert!(error.contains("lingxiaId is missing"), "{error}");
    }

    #[test]
    fn publish_rejects_app_package_without_product_version() {
        let temp = TempDir::new().unwrap();
        let apk = temp.path().join("app.apk");
        write_zip(
            &apk,
            &[
                ("AndroidManifest.xml", b""),
                ("assets/app.json", br#"{"lingxiaId":"demo","env":"prod"}"#),
            ],
        );

        let error = read_app_package_metadata(&apk).unwrap_err().to_string();
        assert!(error.contains("productVersion is missing"), "{error}");
    }

    #[test]
    fn publish_rejects_non_semver_product_version() {
        let temp = TempDir::new().unwrap();
        let apk = temp.path().join("app.apk");
        write_zip(
            &apk,
            &[
                ("AndroidManifest.xml", b""),
                (
                    "assets/app.json",
                    br#"{"lingxiaId":"demo","productVersion":"1.0","env":"prod"}"#,
                ),
            ],
        );

        let error = read_app_package_metadata(&apk).unwrap_err().to_string();
        assert!(error.contains("semantic version"), "{error}");
    }

    fn write_lxapp_archive(path: &std::path::Path, manifest: &str) {
        let file = fs::File::create(path).unwrap();
        let encoder = zstd::stream::write::Encoder::new(file, 0).unwrap();
        let mut tar = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_size(manifest.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, "./lxapp.json", manifest.as_bytes())
            .unwrap();
        tar.into_inner().unwrap().finish().unwrap();
    }

    fn lxapp_meta(version: &str) -> PackageMeta {
        PackageMeta {
            target: "lxapp".into(),
            target_id: "demo.home".into(),
            version: version.into(),
            env: AppEnv::Dev,
            channel: Some("release".into()),
            min_runtime: "0.24.0".into(),
        }
    }

    #[test]
    fn lxapp_publish_vouches_for_the_packaged_manifest() {
        let temp = TempDir::new().unwrap();
        let archive = temp.path().join("home-1.0.1.tar.zst");
        write_lxapp_archive(
            &archive,
            r#"{"appId":"demo.home","version":"1.0.1","minRuntime":"0.24.0"}"#,
        );

        let mut meta = lxapp_meta("1.0.1");
        apply_packaged_manifest(&mut meta, &archive).unwrap();
        assert_eq!(meta.version, "1.0.1");

        let error = apply_packaged_manifest(&mut lxapp_meta("1.0.2"), &archive)
            .unwrap_err()
            .to_string();
        assert!(error.contains("holds demo.home v1.0.1"), "{error}");
        assert!(error.contains("rebuild before publishing"), "{error}");
    }

    #[test]
    fn an_archive_without_its_manifest_is_refused() {
        let temp = TempDir::new().unwrap();
        let archive = temp.path().join("plugin.tar.zst");
        write_lxapp_archive(&archive, "{}");
        let mut meta = lxapp_meta("1.0.0");
        meta.target = "lxplugin".into();
        let error = apply_packaged_manifest(&mut meta, &archive)
            .unwrap_err()
            .to_string();
        assert!(error.contains("lxplugin.json is missing"), "{error}");
    }

    fn write_macos_package(dir: &std::path::Path, version: &str) -> std::path::PathBuf {
        let path = dir.join(format!("Flourish-{version}-macos.zip"));
        let app_json =
            format!(r#"{{"lingxiaId":"demo","productVersion":"{version}","env":"prod"}}"#);
        write_zip(
            &path,
            &[
                ("Flourish.app/Contents/Info.plist", b""),
                (
                    "Flourish.app/Contents/Resources/app.json",
                    app_json.as_bytes(),
                ),
            ],
        );
        path
    }

    #[test]
    fn host_publish_refuses_an_env_the_package_was_not_built_for() {
        let package = std::path::Path::new("Flourish-0.2.6-macos.zip");
        reject_env_mismatch(None, package, AppEnv::Prod).unwrap();
        reject_env_mismatch(Some("prod"), package, AppEnv::Prod).unwrap();
        let error = reject_env_mismatch(Some("dev"), package, AppEnv::Prod)
            .unwrap_err()
            .to_string();
        assert!(error.contains("packaged for prod"), "{error}");
        assert!(error.contains("lingxia package --env dev"), "{error}");
    }

    fn write_zip(path: &std::path::Path, entries: &[(&str, &[u8])]) {
        let file = fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        for (name, data) in entries {
            zip.start_file(*name, SimpleFileOptions::default()).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }
}
