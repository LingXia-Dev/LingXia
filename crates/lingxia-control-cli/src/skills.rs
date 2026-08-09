//! Agent skills, written by the product that answers them.
//!
//! A skill shipped in a package can only describe the commands its author
//! imagined. This one takes its entry points from the `clap` definition the
//! product dispatches and filters them through the capabilities the running
//! product declared. Exact leaf syntax stays in `--help`, where it cannot drift.
//!
//! Writing lands in another tool's configuration directory, which is why this
//! is a command the user runs rather than something a toggle does quietly.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::{Args, CommandFactory, Subcommand};

/// Where a skill goes, per agent. Each is a directory of markdown, so a new
/// agent is a row here rather than a new renderer.
#[derive(Clone, Copy, clap::ValueEnum, PartialEq, Eq)]
pub enum Agent {
    /// Claude Code — `~/.claude/skills/<name>/SKILL.md`
    Claude,
    /// Codex — `~/.codex/skills/<name>/SKILL.md`
    Codex,
}

impl Agent {
    fn root(self) -> Option<PathBuf> {
        let home = home_dir()?;
        Some(match self {
            Agent::Claude => home.join(".claude").join("skills"),
            Agent::Codex => home.join(".codex").join("skills"),
        })
    }

    fn label(self) -> &'static str {
        match self {
            Agent::Claude => "Claude Code",
            Agent::Codex => "Codex",
        }
    }
}

fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    let windows_home = (
        std::env::var_os("USERPROFILE"),
        std::env::var_os("HOMEDRIVE"),
        std::env::var_os("HOMEPATH"),
    );
    #[cfg(not(windows))]
    let windows_home = (None, None, None);

    home_dir_from_env(
        std::env::var_os("HOME"),
        windows_home.0,
        windows_home.1,
        windows_home.2,
    )
}

fn home_dir_from_env(
    home: Option<OsString>,
    user_profile: Option<OsString>,
    home_drive: Option<OsString>,
    home_path: Option<OsString>,
) -> Option<PathBuf> {
    let non_empty = |value: Option<OsString>| value.filter(|value| !value.is_empty());
    if let Some(profile) = non_empty(user_profile) {
        return Some(profile.into());
    }
    if let Some(home) = non_empty(home) {
        return Some(home.into());
    }
    let mut combined = non_empty(home_drive)?;
    combined.push(non_empty(home_path)?);
    Some(combined.into())
}

#[derive(Args, Clone)]
pub struct SkillsOptions {
    #[command(subcommand)]
    pub command: SkillsCommand,
}

#[derive(Subcommand, Clone)]
pub enum SkillsCommand {
    /// Write a skill describing this product's commands
    Install {
        /// Which agent to write for (repeatable)
        #[arg(long, value_enum, required = true)]
        agent: Vec<Agent>,
    },
    /// Remove a previously written skill
    Remove {
        #[arg(long, value_enum, required = true)]
        agent: Vec<Agent>,
    },
    /// Print the skill without writing it
    Show,
}

/// What the product is willing to say about itself.
pub struct Manifest {
    /// The command as it is typed.
    pub command: String,
    /// The product's user-facing name.
    pub product: String,
    /// Namespaces this build declared, when the app was reachable to ask.
    /// `None` means unknown, which prevents showing or installing the skill.
    pub declared: Option<Vec<String>>,
}

pub fn execute<C: CommandFactory>(manifest: &Manifest, options: SkillsOptions) -> i32 {
    match options.command {
        SkillsCommand::Show => match render::<C>(manifest) {
            Ok(body) => {
                println!("{body}");
                0
            }
            Err(error) => report_render_error(manifest, error),
        },
        SkillsCommand::Install { agent } => {
            let body = match render::<C>(manifest) {
                Ok(body) => body,
                Err(error) => return report_render_error(manifest, error),
            };
            report(agent.iter().map(|agent| install(*agent, manifest, &body)))
        }
        SkillsCommand::Remove { agent } => {
            report(agent.iter().map(|agent| remove(*agent, manifest)))
        }
    }
}

fn report_render_error(manifest: &Manifest, error: anyhow::Error) -> i32 {
    eprintln!("Error: {error}");
    if manifest.declared.is_none() { 8 } else { 7 }
}

fn report(results: impl Iterator<Item = Result<String>>) -> i32 {
    let mut code = 0;
    for result in results {
        match result {
            Ok(note) => println!("{note}"),
            Err(error) => {
                eprintln!("Error: {error}");
                code = 1;
            }
        }
    }
    code
}

fn skill_dir(agent: Agent, manifest: &Manifest) -> Result<PathBuf> {
    let root = agent
        .root()
        .context("cannot locate the home directory to write a skill into")?;
    Ok(root.join(&manifest.command))
}

/// Left beside the skill so removal can tell what it wrote from what it found.
const OWNED: &str = ".lingxia-product-skill";

fn install(agent: Agent, manifest: &Manifest, body: &str) -> Result<String> {
    let directory = skill_dir(agent, manifest)?;
    // A product name that happens to match a skill someone wrote themselves
    // must not silently replace it. This directory belongs to another tool and
    // the files in it are not ours to assume.
    if directory.exists() && !owned(&directory, manifest) {
        anyhow::bail!(
            "{} already exists and was not written by this product; \
             rename the product or remove that directory first",
            directory.display()
        );
    }
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("failed to create {}", directory.display()))?;
    std::fs::write(directory.join(OWNED), manifest.product.as_bytes())
        .with_context(|| format!("failed to mark {}", directory.display()))?;
    let path = directory.join("SKILL.md");
    std::fs::write(&path, body).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(format!("{}: {}", agent.label(), path.display()))
}

fn owned(directory: &Path, manifest: &Manifest) -> bool {
    std::fs::read_to_string(directory.join(OWNED)).is_ok_and(|owner| owner == manifest.product)
}

fn remove(agent: Agent, manifest: &Manifest) -> Result<String> {
    let directory = skill_dir(agent, manifest)?;
    if !directory.exists() {
        return Ok(format!("{}: nothing to remove", agent.label()));
    }
    // Only what this product wrote. A directory it did not create may hold
    // someone's own work, and a name collision is not a licence to delete it.
    if !owned(&directory, manifest) {
        anyhow::bail!(
            "{} was not written by this product; leaving it alone",
            directory.display()
        );
    }
    for file in ["SKILL.md", OWNED] {
        match std::fs::remove_file(directory.join(file)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to remove {}", directory.display()));
            }
        }
    }
    // Only if nothing else moved in.
    let _ = std::fs::remove_dir(&directory);
    Ok(format!(
        "{}: removed {}",
        agent.label(),
        directory.display()
    ))
}

/// Capability behind an agent-facing entry point. Human administration
/// (`control`, `skills`) deliberately has no row and never enters the skill.
fn required_capability(namespace: &str) -> Option<&'static str> {
    let needs = match namespace {
        "computer" | "desktop" => "computerUse",
        "browser" => "browserUse",
        "app" | "screenshot" | "windows" | "mouse" | "key" | "doctor" => "appUse",
        _ => return None,
    };
    Some(needs)
}

/// Whether this running build will answer an agent-facing entry point.
fn allowed(declared: &[String], namespace: &str) -> bool {
    let Some(needs) = required_capability(namespace) else {
        return false;
    };
    // `computerUse` contains `appUse`: an agent that may drive any window can
    // reach this product's own through the wider door. The product reports both
    // when it declares the wider one, and applying the rule here too means a
    // hand-written list cannot make the skill disagree with the endpoint.
    declared
        .iter()
        .any(|name| name == needs || (needs == "appUse" && name == "computerUse"))
}

/// Render a short operating contract. Exact command trees stay behind
/// `<entry-point> --help`, so loading the skill does not load a CLI manual.
fn render<C: CommandFactory>(manifest: &Manifest) -> Result<String> {
    let declared = manifest.declared.as_deref().with_context(|| {
        format!(
            "{} is not reachable; open the app, enable its automation interface, and retry",
            manifest.product
        )
    })?;
    if declared.is_empty() {
        anyhow::bail!("{} declares no automation capability", manifest.product);
    }

    let command = C::command();
    let has_entry_point = command
        .get_subcommands()
        .any(|sub| allowed(declared, sub.get_name()));
    if !has_entry_point {
        anyhow::bail!(
            "{} declares capabilities this command does not implement",
            manifest.product
        );
    }

    let mut out = String::new();

    out.push_str("---\n");
    out.push_str(&format!("name: {}\n", manifest.command));
    let description = format!(
        "Inspect or operate the running {} through its local `{}` CLI. Use only for tasks targeting this product; the app must be open.",
        manifest.product, manifest.command
    );
    out.push_str(&format!(
        "description: {}\n",
        serde_json::to_string(&description).expect("a skill description is valid JSON")
    ));
    out.push_str("---\n\n");

    out.push_str(&format!("# {}\n\n", manifest.product));
    out.push_str(&format!(
        "`{}` reaches the running {} on this machine. It never starts the app; \
         if it cannot connect, ask the user to open the app and stop.\n\n",
        manifest.command, manifest.product
    ));

    out.push_str("## Available entry points\n\n");
    out.push_str("This list is filtered by the capabilities reported by the running build.\n\n");
    for sub in command.get_subcommands() {
        if !allowed(declared, sub.get_name()) {
            continue;
        }
        let entry = format!("{} {}", manifest.command, sub.get_name());
        let about = sub
            .get_about()
            .map(|about| format!(" — {about}"))
            .unwrap_or_default();
        out.push_str(&format!(
            "- `{entry}`{about}; inspect with `{entry} --help`.\n"
        ));
    }
    out.push_str("\nUse the narrowest entry point that can complete the task. Run the leaf command's `--help` before first use; prefer `--json` when offered.\n\n");

    out.push_str("## Operating rules\n\n");
    out.push_str(
        "- Inspect before acting, then verify the result with a read command. \
         Prefer structured app or browser commands over screen coordinates.\n",
    );
    if declared.iter().any(|name| name == "computerUse") {
        out.push_str(&format!(
            "- Before machine-wide work, run `{} computer permissions --json`. \
             If an OS grant is missing, tell the user; do not retry permission prompts.\n",
            manifest.command
        ));
        out.push_str(
            "- On macOS and Windows, mutating computer commands may open an activity \
             indicator or viewer for the person. Never target, hide, or dismiss it.\n",
        );
        out.push_str(
            "- On Windows, pointer and key input uses the active desktop. Keep the target \
             visible; a named window or pid is activated before input.\n",
        );
    }
    if declared.iter().any(|name| name == "browserUse") {
        out.push_str(
            "- `browser` controls only this product's in-app browser. Chrome, Edge, and \
             Safari are external apps; never treat them as `browser` targets.\n",
        );
    }
    out.push_str(
        "- `--allow-control` only acknowledges a state change already authorized \
         by the user's request; it does not grant permission. Add \
         `--allow-destructive` only when the user explicitly authorized that \
         destructive effect.\n",
    );
    out.push_str(
        "- Read both the exit code and message. For exit 6: add a missing \
         acknowledgement only when authorized; stop on an undeclared capability; \
         ask the user for a missing OS grant. Never route around a refusal.\n",
    );
    out.push_str(
        "- Other exits: 2 usage (read `--help`), 3 not found, 4 ambiguous, \
         5 timeout, 7 unsupported, 8 unavailable, 9 stale handle (refresh it), \
         10 operation failed after target resolution.\n",
    );
    Ok(out)
}

/// A skill that names the running executable is wrong the moment the product
/// moves, so callers pass what they are; nothing here reads a build constant.
pub fn manifest_for(command: impl Into<String>, product: impl Into<String>) -> Manifest {
    Manifest {
        command: command.into(),
        product: product.into(),
        declared: None,
    }
}

/// Build a manifest from what the running product reports. Keep this query in
/// one place so examples and product entry points fail closed the same way.
pub fn manifest_for_running(
    command: impl Into<String>,
    product: impl Into<String>,
    transport: &dyn crate::transport::Transport,
) -> Manifest {
    let mut manifest = manifest_for(command, product);
    manifest.declared = transport
        .request(lingxia_control_protocol::methods::control::STATUS, None)
        .ok()
        .and_then(|result| result?.get("declared")?.as_array().cloned())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect()
        });
    manifest
}

pub fn skill_path(agent: Agent, command: &str) -> Option<PathBuf> {
    Some(agent.root()?.join(command).join("SKILL.md"))
}

/// Whether a directory already holds a skill this product wrote.
pub fn is_installed(agent: Agent, command: &str) -> bool {
    skill_path(agent, command).is_some_and(|path: PathBuf| Path::new(&path).exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_profile_is_used_when_home_is_missing() {
        assert_eq!(
            home_dir_from_env(None, Some(r"C:\Users\demo".into()), None, None),
            Some(PathBuf::from(r"C:\Users\demo"))
        );
    }

    #[test]
    fn windows_profile_wins_over_a_shell_specific_home() {
        assert_eq!(
            home_dir_from_env(
                Some(r"/c/Users/demo".into()),
                Some(r"C:\Users\demo".into()),
                None,
                None,
            ),
            Some(PathBuf::from(r"C:\Users\demo"))
        );
    }

    #[test]
    fn windows_drive_and_path_are_the_last_home_fallback() {
        assert_eq!(
            home_dir_from_env(
                Some(OsString::new()),
                Some(OsString::new()),
                Some("C:".into()),
                Some(r"\Users\demo".into()),
            ),
            Some(PathBuf::from(r"C:\Users\demo"))
        );
    }

    struct StatusTransport;

    impl crate::transport::Transport for StatusTransport {
        fn request(
            &self,
            method: &str,
            _params: Option<serde_json::Value>,
        ) -> Result<Option<serde_json::Value>> {
            assert_eq!(method, lingxia_control_protocol::methods::control::STATUS);
            Ok(Some(serde_json::json!({
                "declared": ["appUse", "browserUse"]
            })))
        }
    }

    #[derive(clap::Parser)]
    #[command(about = "test")]
    struct Fake {
        #[command(subcommand)]
        _command: FakeCommand,
    }

    #[derive(Subcommand)]
    enum FakeCommand {
        /// Capture a PNG of the app window
        Screenshot,
        /// Drive the in-app browser
        Browser {
            #[command(subcommand)]
            _command: FakeLeaf,
        },
        /// Automate the machine
        Computer {
            #[command(subcommand)]
            _command: FakeLeaf,
        },
        /// User administration, not agent work
        Control,
        /// Manage installed skills
        Skills,
    }

    #[derive(Subcommand)]
    enum FakeLeaf {
        /// List every window
        Windows,
        /// Mouse input
        Pointer {
            #[command(subcommand)]
            _command: FakeVerb,
        },
    }

    #[derive(Subcommand)]
    enum FakeVerb {
        /// Click at a point
        Click,
    }

    #[test]
    fn the_skill_is_rendered_from_the_commands_that_exist() {
        let manifest = Manifest {
            command: "myapp".into(),
            product: "My App".into(),
            declared: Some(vec!["computerUse".into()]),
        };
        let body = render::<Fake>(&manifest).unwrap();

        assert!(body.contains("name: myapp"));
        assert!(body.contains("`myapp screenshot`"));
        assert!(body.contains("Capture a PNG of the app window"));
        assert!(body.contains("`myapp computer --help`"));
        assert!(!body.contains("myapp computer pointer click"));
        assert!(!body.contains("`myapp control`"));
        assert!(!body.contains("`myapp skills`"));
        assert!(body.contains("does not grant permission"));
        assert!(body.contains("uses the active desktop"));
        assert!(
            body.lines().count() < 50,
            "the entry skill should stay a short router, not become a CLI manual"
        );
    }

    /// A namespace this build refuses is worse than one that is absent: an
    /// agent reads it as available, tries it, and gets a refusal it cannot act
    /// on.
    #[test]
    fn a_namespace_the_product_did_not_declare_is_not_described() {
        let manifest = Manifest {
            command: "myapp".into(),
            product: "My App".into(),
            declared: Some(vec!["appUse".into()]),
        };
        let body = render::<Fake>(&manifest).unwrap();
        assert!(body.contains("`myapp screenshot`"), "its own surface stays");
        assert!(
            !body.contains("`myapp computer`"),
            "a namespace nobody declared must not be described"
        );

        // The product's own window commands sit at the top level, so they are
        // the easy ones to forget to filter.
        let browser_only = Manifest {
            command: "myapp".into(),
            product: "My App".into(),
            declared: Some(vec!["browserUse".into()]),
        };
        let body = render::<Fake>(&browser_only).unwrap();
        assert!(body.contains("`myapp browser`"));
        assert!(body.contains("only this product's in-app browser"));
        assert!(!body.contains("`myapp screenshot`"));
        assert!(!body.contains("`myapp computer`"));
    }

    #[test]
    fn an_unknown_list_refuses_to_write_a_guess() {
        let error = render::<Fake>(&manifest_for("myapp", "My App")).unwrap_err();
        assert!(error.to_string().contains("not reachable"));
    }

    #[test]
    fn a_running_manifest_uses_the_products_declared_capabilities() {
        let manifest = manifest_for_running("myapp", "My App", &StatusTransport);
        assert_eq!(
            manifest.declared,
            Some(vec!["appUse".to_string(), "browserUse".to_string()])
        );
    }
}
