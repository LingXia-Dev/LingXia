//! Agent skills, written by the product that answers them.
//!
//! A skill shipped in a package can only describe the commands its author
//! imagined. This one is rendered from the very `clap` definition the product
//! dispatches, and from the capabilities it actually declared — so it cannot
//! describe a namespace this build refuses, and it cannot fall behind a
//! version bump. It is the same rule the repo applies to its own docs: mirror
//! nothing by hand.
//!
//! Writing lands in another tool's configuration directory, which is why this
//! is a command the user runs rather than something a toggle does quietly.

use std::path::{Path, PathBuf};

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
        let home = std::env::var_os("HOME").map(PathBuf::from)?;
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
    /// `None` means unknown — which the skill says plainly rather than
    /// guessing, since a wrong list is worse than no list.
    pub declared: Option<Vec<String>>,
}

pub fn execute<C: CommandFactory>(manifest: &Manifest, options: SkillsOptions) -> i32 {
    match options.command {
        SkillsCommand::Show => {
            println!("{}", render::<C>(manifest));
            0
        }
        SkillsCommand::Install { agent } => {
            let body = render::<C>(manifest);
            report(agent.iter().map(|agent| install(*agent, manifest, &body)))
        }
        SkillsCommand::Remove { agent } => {
            report(agent.iter().map(|agent| remove(*agent, manifest)))
        }
    }
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
    if directory.exists() && !owned(&directory) {
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

fn owned(directory: &Path) -> bool {
    directory.join(OWNED).exists()
}

fn remove(agent: Agent, manifest: &Manifest) -> Result<String> {
    let directory = skill_dir(agent, manifest)?;
    if !directory.exists() {
        return Ok(format!("{}: nothing to remove", agent.label()));
    }
    // Only what this product wrote. A directory it did not create may hold
    // someone's own work, and a name collision is not a licence to delete it.
    if !owned(&directory) {
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

/// Walk to the leaves. Stopping one level down left an agent with
/// `computer pointer` — a form that does nothing — instead of
/// `computer pointer click`, and it will invent the rest.
fn walk(command: &clap::Command, prefix: &str, out: &mut Vec<String>) {
    let mut children = command.get_subcommands().peekable();
    if children.peek().is_none() {
        return;
    }
    for child in command.get_subcommands() {
        let path = format!("{prefix} {}", child.get_name());
        if child.get_subcommands().next().is_some() {
            walk(child, &path, out);
        } else {
            // The required options too. A leaf on its own is a form that
            // cannot be run, and an agent handed one will invent the rest.
            let required: Vec<String> = child
                .get_arguments()
                .filter(|arg| arg.is_required_set())
                .map(|arg| match arg.get_long() {
                    Some(long) => format!(" --{long} <{}>", arg.get_id()),
                    None => format!(" <{}>", arg.get_id()),
                })
                .collect();
            out.push(format!(
                "- `{path}{}`{}",
                required.join(""),
                child
                    .get_about()
                    .map(|about| format!(" — {about}"))
                    .unwrap_or_default()
            ));
        }
    }
}

/// Whether this build will answer a namespace at all.
///
/// `None` declared means the app could not be reached, and a list nobody can
/// check is better shown whole than silently trimmed.
fn allowed(manifest: &Manifest, namespace: &str) -> bool {
    let Some(declared) = manifest.declared.as_deref() else {
        return true;
    };
    // The namespaces that ride a capability, by the name the product declares.
    // The product's own window commands sit at the top level rather than under
    // a prefix, so they have to be named individually — a browser-only product
    // must not be told it can screenshot windows it will be refused.
    let needs = match namespace {
        "computer" => "computerUse",
        "browser" => "browserUse",
        "screenshot" | "windows" | "mouse" | "key" | "doctor" => "appUse",
        // What is left is about the tool rather than about the machine.
        _ => return true,
    };
    // `computerUse` contains `appUse`: an agent that may drive any window can
    // reach this product's own through the wider door. The product reports both
    // when it declares the wider one, and applying the rule here too means a
    // hand-written list cannot make the skill disagree with the endpoint.
    declared
        .iter()
        .any(|name| name == needs || (needs == "appUse" && name == "computerUse"))
}

/// Render the skill from the command definition itself.
fn render<C: CommandFactory>(manifest: &Manifest) -> String {
    let command = C::command();
    let mut out = String::new();

    out.push_str("---\n");
    out.push_str(&format!("name: {}\n", manifest.command));
    out.push_str(&format!(
        "description: Drive {} — the running app, from this machine.\n",
        manifest.product
    ));
    out.push_str("---\n\n");

    out.push_str(&format!("# {}\n\n", manifest.product));
    out.push_str(&format!(
        "`{}` drives the running {} over a local socket. It reaches only this \
         machine and only while the app is running; if the app is closed, every \
         command fails to connect rather than starting it.\n\n",
        manifest.command, manifest.product
    ));

    out.push_str("## What this build allows\n\n");
    match manifest.declared.as_deref() {
        Some([]) => out.push_str("Nothing — this build declared no automation capability.\n\n"),
        Some(declared) => {
            for name in declared {
                out.push_str(&format!("- `{name}`\n"));
            }
            out.push_str("\nAnything outside these is refused by the app, not by this file.\n\n");
        }
        // The app is the only thing that knows, so an unreachable app leaves
        // this unrecorded rather than guessed.
        None => out.push_str(
            "Not recorded — the app could not be reached when this was written. \
             It refuses any namespace it did not declare; treat a refusal as the \
             answer, not as something to work around.\n\n",
        ),
    }

    out.push_str("## Commands\n\n");
    for sub in command.get_subcommands() {
        // A namespace this build refuses is worse than absent: an agent reads
        // it as available, tries it, and gets a refusal it cannot act on.
        if !allowed(manifest, sub.get_name()) {
            continue;
        }
        out.push_str(&format!(
            "### `{} {}`\n\n",
            manifest.command,
            sub.get_name()
        ));
        if let Some(about) = sub.get_about() {
            out.push_str(&format!("{about}\n\n"));
        }
        let mut lines = Vec::new();
        walk(
            sub,
            &format!("{} {}", manifest.command, sub.get_name()),
            &mut lines,
        );
        if !lines.is_empty() {
            out.push_str(&lines.join("\n"));
            out.push_str("\n\n");
        }
    }

    out.push_str("## Reading results\n\n");
    out.push_str(
        "Most commands take `--json` for machine-readable output; `--help` on \
         any of them is exact about which. Failures use the exit code, not just \
         the message: 2 usage, 3 not found, 4 ambiguous, 5 timeout, \
         6 permission, 7 unsupported, 8 unavailable, 9 stale handle, 10 failed \
         after the target was resolved.\n\n",
    );
    out.push_str(
        "Commands that change something need `--allow-control`; the destructive \
         ones also need `--allow-destructive`. A permission error means the user \
         has not granted the app what the OS requires — say so rather than \
         retrying.\n",
    );
    out
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
        /// Automate the machine
        Computer {
            #[command(subcommand)]
            _command: FakeLeaf,
        },
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
        let body = render::<Fake>(&manifest);

        assert!(body.contains("name: myapp"));
        assert!(body.contains("`myapp screenshot`"));
        assert!(body.contains("Capture a PNG of the app window"));
        // Nested commands matter most: an agent that only knows the top level
        // will guess at the rest.
        assert!(body.contains("`myapp computer windows`"));
        assert!(body.contains("List every window"));
        assert!(body.contains("computerUse"));
        // Down to the leaf: `computer pointer` on its own does nothing, and an
        // agent given only that will invent the rest.
        assert!(body.contains("`myapp computer pointer click`"));
        assert!(
            !body.contains("`myapp computer pointer`\n"),
            "a form that cannot be run must not be offered"
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
        let body = render::<Fake>(&manifest);
        assert!(body.contains("`myapp screenshot`"), "its own surface stays");
        assert!(
            !body.contains("`myapp computer pointer click`"),
            "a namespace nobody declared must not be described"
        );

        // The product's own window commands sit at the top level, so they are
        // the easy ones to forget to filter.
        let browser_only = Manifest {
            command: "myapp".into(),
            product: "My App".into(),
            declared: Some(vec!["browserUse".into()]),
        };
        assert!(
            !render::<Fake>(&browser_only).contains("`myapp screenshot`"),
            "a browser-only product cannot screenshot its own windows"
        );
    }

    #[test]
    fn an_unknown_list_says_so_rather_than_guessing() {
        let body = render::<Fake>(&manifest_for("myapp", "My App"));
        assert!(body.contains("Not recorded"));
        assert!(
            !body.contains("refused by the app, not by this file"),
            "an unknown list must not be presented as a complete one"
        );
    }
}
