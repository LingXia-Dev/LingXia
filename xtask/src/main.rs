//! `cargo xtask sdk`: build platform SDK artifacts from this checkout.
//!
//! The one build path for the release workflow (which uploads what this
//! writes), CI, and `lingxia pin`, which builds a pinned commit's SDKs with
//! that commit's own steps. It only orchestrates: cargo,
//! Gradle and hvigor decide what is stale, and nothing here touches the
//! network beyond what those tools do or publishes anything. Tool output goes
//! to stderr; stdout carries only the artifact paths.

use anyhow::{Context, Result, bail};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const USAGE: &str = "usage: cargo xtask sdk --platform <apple|android|harmony|all> --out <dir> \
[--cli <lingxia>] [--checksums] [--android-maven-dir <dir>] [--no-zip]";

#[derive(Clone, Copy, PartialEq)]
enum Platform {
    Apple,
    Android,
    Harmony,
}

impl Platform {
    fn name(self) -> &'static str {
        match self {
            Platform::Apple => "apple",
            Platform::Android => "android",
            Platform::Harmony => "harmony",
        }
    }
}

struct Args {
    platforms: Vec<Platform>,
    /// `SHASUMS256-<version>-<platforms>.txt` beside the artifacts.
    checksums: bool,
    out: PathBuf,
    /// The `lingxia` that generates i18n and icons; compiled from this
    /// checkout when absent.
    cli: Option<PathBuf>,
    android_maven_dir: Option<PathBuf>,
    zip: bool,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut argv = std::env::args().skip(1);
    match argv.next().as_deref() {
        Some("sdk") => {}
        _ => bail!("{USAGE}"),
    }
    let args = parse(argv)?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("xtask lives in the workspace root")?
        .to_path_buf();
    fs::create_dir_all(&args.out)?;
    let out = args.out.canonicalize()?;
    let version = workspace_version(&root)?;

    let mut artifacts = Vec::new();
    for &platform in &args.platforms {
        eprintln!("==> Building the {} SDK", platform.name());
        generate_resources(&root, platform, args.cli.as_deref())?;
        let artifact = match platform {
            Platform::Apple => build_apple(&root, &out, &version, args.zip)?,
            Platform::Android => build_android(&root, &out, &version, &args)?,
            Platform::Harmony => build_harmony(&root, &out, &version)?,
        };
        println!("{}", artifact.display());
        artifacts.push(artifact);
    }
    if args.checksums {
        let names: Vec<_> = args.platforms.iter().map(|p| p.name()).collect();
        let sums = out.join(format!("SHASUMS256-{version}-{}.txt", names.join("-")));
        write_checksums(&sums, &artifacts)?;
        println!("{}", sums.display());
    }
    Ok(())
}

/// `<sha256>  <file name>` per artifact, as `sha256sum` writes it.
fn write_checksums(path: &Path, artifacts: &[PathBuf]) -> Result<()> {
    use sha2::Digest;
    let mut text = String::new();
    for artifact in artifacts {
        if !artifact.is_file() {
            bail!(
                "--checksums needs files, but {} is not one (--no-zip?)",
                artifact.display()
            );
        }
        let digest = sha2::Sha256::digest(fs::read(artifact)?);
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        let name = artifact.file_name().unwrap_or_default().to_string_lossy();
        text.push_str(&format!("{hex}  {name}\n"));
    }
    fs::write(path, text)?;
    Ok(())
}

fn parse(mut argv: impl Iterator<Item = String>) -> Result<Args> {
    let mut platforms = None;
    let mut checksums = false;
    let mut out = None;
    let mut cli = None;
    let mut android_maven_dir = None;
    let mut zip = true;
    while let Some(arg) = argv.next() {
        let mut value = || argv.next().with_context(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--platform" => {
                platforms = Some(match value()?.to_ascii_lowercase().as_str() {
                    "apple" | "ios" => vec![Platform::Apple],
                    "android" => vec![Platform::Android],
                    "harmony" => vec![Platform::Harmony],
                    "all" => vec![Platform::Apple, Platform::Android, Platform::Harmony],
                    other => bail!("unknown platform {other}\n{USAGE}"),
                })
            }
            "--checksums" => checksums = true,
            "--out" => out = Some(PathBuf::from(value()?)),
            "--cli" => cli = Some(PathBuf::from(value()?)),
            "--android-maven-dir" => android_maven_dir = Some(PathBuf::from(value()?)),
            "--no-zip" => zip = false,
            other => bail!("unknown argument {other}\n{USAGE}"),
        }
    }
    Ok(Args {
        platforms: platforms.context(USAGE)?,
        checksums,
        out: out.context(USAGE)?,
        cli,
        android_maven_dir,
        zip,
    })
}

/// `[workspace.package] version` of the root manifest.
fn workspace_version(root: &Path) -> Result<String> {
    let manifest = fs::read_to_string(root.join("Cargo.toml"))?;
    let mut in_section = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_section = line == "[workspace.package]";
        } else if in_section
            && let Some(value) = line.strip_prefix("version")
            && let Some(value) = value.trim_start().strip_prefix('=')
        {
            return Ok(value.trim().trim_matches('"').to_string());
        }
    }
    bail!("no [workspace.package] version in Cargo.toml")
}

/// Run with output on stderr, so stdout stays the artifact path.
fn run_in(dir: &Path, program: &str, args: &[&str], envs: &[(&str, &str)]) -> Result<()> {
    eprintln!("+ ({}) {program} {}", dir.display(), args.join(" "));
    let status = Command::new(program)
        .args(args)
        .envs(envs.iter().copied())
        .current_dir(dir)
        .stdout(Stdio::from(std::io::stderr()))
        .status()
        .with_context(|| format!("run {program}"))?;
    if !status.success() {
        bail!("{program} {} failed", args.join(" "));
    }
    Ok(())
}

/// A tool on PATH; Windows installs the DevEco and Gradle ones as scripts.
fn tool(name: &str) -> String {
    if cfg!(windows) {
        for ext in ["cmd", "bat", "exe"] {
            let candidate = format!("{name}.{ext}");
            let found = std::env::var_os("PATH").is_some_and(|path| {
                std::env::split_paths(&path).any(|dir| dir.join(&candidate).is_file())
            });
            if found {
                return candidate;
            }
        }
    }
    name.to_string()
}

fn generate_resources(root: &Path, platform: Platform, cli: Option<&Path>) -> Result<()> {
    eprintln!("==> Generating SDK resources");
    let i18n = root.join("i18n");
    let icons = root.join("design/icons/svg");
    let (i18n_out, icons_out) = match platform {
        Platform::Apple => (
            vec!["--no-android", "--no-harmony", "--ios-out"],
            "--ios-out",
        ),
        Platform::Android => (
            vec!["--no-ios", "--no-harmony", "--android-out"],
            "--android-out",
        ),
        Platform::Harmony => (
            vec!["--no-android", "--no-ios", "--harmony-out"],
            "--harmony-out",
        ),
    };
    let (res_dir, icons_dir) = match platform {
        Platform::Apple => (
            root.join("lingxia-sdk/apple/Sources/Resources"),
            root.join("lingxia-sdk/apple/Sources/Resources/icons"),
        ),
        Platform::Android => (
            root.join("lingxia-sdk/android/lingxia/src/main/res"),
            root.join("lingxia-sdk/android/lingxia/src/main/res/drawable"),
        ),
        Platform::Harmony => (
            root.join("lingxia-sdk/harmony/lingxia/src/main/resources"),
            root.join("lingxia-sdk/harmony/lingxia/src/main/resources/rawfile/icons"),
        ),
    };
    let i18n = i18n.to_string_lossy();
    let icons = icons.to_string_lossy();
    let res_dir = res_dir.to_string_lossy();
    let icons_dir = icons_dir.to_string_lossy();
    let mut i18n_args = vec!["gen", "i18n", "--input", &i18n, "--no-rust", "--no-ts"];
    i18n_args.extend(i18n_out);
    i18n_args.push(&res_dir);
    let icons_args = ["gen", "icons", "--input", &icons, icons_out, &icons_dir];
    match cli {
        Some(cli) => {
            let cli = cli.to_string_lossy();
            run_in(root, &cli, &i18n_args, &[])?;
            run_in(root, &cli, &icons_args, &[])
        }
        None => {
            let cargo = |gen_args: &[&str]| {
                let mut all = vec!["run", "-q", "-p", "lingxia-cli", "--"];
                all.extend_from_slice(gen_args);
                run_in(root, "cargo", &all, &[])
            };
            cargo(&i18n_args)?;
            cargo(&icons_args)
        }
    }
}

fn build_apple(root: &Path, out: &Path, version: &str, zip: bool) -> Result<PathBuf> {
    let sdk = root.join("lingxia-sdk/apple");
    // Cargo decides what is stale; the bridge build scripts write into
    // Sources/generated and rerun when their output is missing.
    run_in(
        root,
        "cargo",
        &[
            "build",
            "-p",
            "lingxia",
            "--target",
            "aarch64-apple-ios",
            "--release",
        ],
        &[("LINGXIA_GENERATE_BRIDGE", "1")],
    )?;
    let generated = sdk.join("Sources/generated");
    for item in ["LingXiaRustAPI", "LingXiaSwiftAPI", "SwiftBridgeCore.h"] {
        if !generated.join(item).exists() {
            bail!("the bridge build left no Sources/generated/{item}");
        }
    }

    // The staged copy local dev and the CLI read inside this workspace.
    let staged = root.join("target/spm/lingxia");
    let _ = fs::remove_dir_all(&staged);
    copy_tree(&sdk, &staged, &[".build", ".swiftpm"])?;

    if !zip {
        return Ok(sdk);
    }
    let artifact = out.join(format!("lingxia-sdk-apple-source-{version}.zip"));
    let mut archive = Zipper::create(&artifact)?;
    archive.add_file(
        &sdk.join("Package.swift"),
        "lingxia-apple-sdk/Package.swift",
    )?;
    archive.add_tree(&sdk.join("Sources"), "lingxia-apple-sdk/Sources")?;
    archive.finish()?;
    Ok(artifact)
}

fn build_android(root: &Path, out: &Path, version: &str, args: &Args) -> Result<PathBuf> {
    let sdk = root.join("lingxia-sdk/android");
    let maven = args
        .android_maven_dir
        .clone()
        .unwrap_or_else(|| out.join("android/maven"));
    fs::create_dir_all(&maven)?;
    let maven = maven.canonicalize()?;
    let gradlew = if cfg!(windows) {
        "gradlew.bat"
    } else {
        "./gradlew"
    };
    // Only the local directory repository; Maven Central publishing runs in
    // CI with credentials and a signing key.
    run_in(
        &sdk,
        gradlew,
        &[
            "publishAllPublicationsToLocalExampleRepository",
            &format!("-PLOCAL_MAVEN_REPO_DIR={}", maven.display()),
            &format!("-Pversion={version}"),
        ],
        &[],
    )?;
    let group = maven.join("io/github/lingxia-dev");
    let aar = group.join(format!("lingxia/{version}/lingxia-{version}.aar"));
    if !aar.is_file() {
        bail!("no AAR after publishing: {}", aar.display());
    }
    if !args.zip {
        return Ok(maven);
    }
    let artifact = out.join(format!("lingxia-sdk-android-maven-{version}.zip"));
    let mut archive = Zipper::create(&artifact)?;
    // Only this release's modules, not whatever else the local repo holds.
    for module in [
        "lingxia",
        "lingxia-core",
        "lingxia-camera",
        "lingxia-scanner",
    ] {
        let dir = group.join(module).join(version);
        if !dir.is_dir() {
            bail!("missing module {module}/{version}");
        }
        archive.add_tree(
            &dir,
            &format!("maven/io/github/lingxia-dev/{module}/{version}"),
        )?;
    }
    archive.finish()?;
    Ok(artifact)
}

fn build_harmony(root: &Path, out: &Path, version: &str) -> Result<PathBuf> {
    let sdk = root.join("lingxia-sdk/harmony");
    let module = sdk.join("lingxia");
    let _ = fs::remove_dir_all(module.join("oh_modules"));
    let _ = fs::remove_file(module.join("oh-package-lock.json5"));
    run_in(&module, &tool("ohpm"), &["install"], &[])?;
    let workspace_har = root.join("target/ohpm/lingxia.har");
    let _ = fs::remove_file(&workspace_har);
    let _ = fs::remove_dir_all(module.join("build"));
    run_in(&sdk, &tool("hvigorw"), &["assembleHar"], &[])?;
    let har = find_file(&module.join("build"), "har")
        .context("no .har under lingxia-sdk/harmony/lingxia/build")?;
    // The workspace copy example apps depend on through `file:`.
    fs::create_dir_all(workspace_har.parent().unwrap_or(root))?;
    fs::copy(&har, &workspace_har)?;
    let artifact = out.join(format!("lingxia-sdk-harmony-{version}.har"));
    fs::copy(&har, &artifact)?;
    Ok(artifact)
}

fn find_file(dir: &Path, extension: &str) -> Option<PathBuf> {
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file(&path, extension) {
                return Some(found);
            }
        } else if path.extension().is_some_and(|ext| ext == extension) {
            return Some(path);
        }
    }
    None
}

fn copy_tree(from: &Path, to: &Path, skip: &[&str]) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str == ".DS_Store" || skip.contains(&name_str.as_ref()) {
            continue;
        }
        let target = to.join(&name);
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target, &[])?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// A zip with forward-slash entry names and no `.DS_Store`.
struct Zipper {
    writer: zip::ZipWriter<fs::File>,
}

impl Zipper {
    fn create(path: &Path) -> Result<Self> {
        let _ = fs::remove_file(path);
        Ok(Self {
            writer: zip::ZipWriter::new(fs::File::create(path)?),
        })
    }

    fn options() -> zip::write::SimpleFileOptions {
        zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
    }

    fn add_file(&mut self, path: &Path, name: &str) -> Result<()> {
        self.writer.start_file(name, Self::options())?;
        self.writer.write_all(&fs::read(path)?)?;
        Ok(())
    }

    fn add_tree(&mut self, dir: &Path, prefix: &str) -> Result<()> {
        let mut entries: Vec<_> = fs::read_dir(dir)?.collect::<Result<_, _>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == ".DS_Store" {
                continue;
            }
            let entry_name = format!("{prefix}/{name}");
            if entry.file_type()?.is_dir() {
                self.writer
                    .add_directory(format!("{entry_name}/"), Self::options())?;
                self.add_tree(&entry.path(), &entry_name)?;
            } else {
                self.add_file(&entry.path(), &entry_name)?;
            }
        }
        Ok(())
    }

    fn finish(self) -> Result<()> {
        self.writer.finish()?;
        Ok(())
    }
}
