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

fn install(agent: Agent, manifest: &Manifest, body: &str) -> Result<String> {
    let directory = skill_dir(agent, manifest)?;
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("failed to create {}", directory.display()))?;
    let path = directory.join("SKILL.md");
    std::fs::write(&path, body).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(format!("{}: {}", agent.label(), path.display()))
}

fn remove(agent: Agent, manifest: &Manifest) -> Result<String> {
    let directory = skill_dir(agent, manifest)?;
    match std::fs::remove_dir_all(&directory) {
        Ok(()) => Ok(format!(
            "{}: removed {}",
            agent.label(),
            directory.display()
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(format!("{}: nothing to remove", agent.label()))
        }
        Err(error) => {
            Err(error).with_context(|| format!("failed to remove {}", directory.display()))
        }
    }
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
        let about = sub.get_about().map(|about| about.to_string());
        out.push_str(&format!(
            "### `{} {}`\n\n",
            manifest.command,
            sub.get_name()
        ));
        if let Some(about) = about {
            out.push_str(&format!("{about}\n\n"));
        }
        let leaves: Vec<_> = sub.get_subcommands().collect();
        if !leaves.is_empty() {
            for leaf in leaves {
                out.push_str(&format!(
                    "- `{} {} {}`{}\n",
                    manifest.command,
                    sub.get_name(),
                    leaf.get_name(),
                    leaf.get_about()
                        .map(|about| format!(" — {about}"))
                        .unwrap_or_default()
                ));
            }
            out.push('\n');
        }
    }

    out.push_str("## Reading results\n\n");
    out.push_str(
        "Every command takes `--json` for machine-readable output. Failures use \
         the exit code, not just the message: 2 usage, 3 not found, 4 ambiguous, \
         5 timeout, 6 permission, 7 unsupported, 8 unavailable, 9 stale handle, \
         10 failed after the target was resolved.\n\n",
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
