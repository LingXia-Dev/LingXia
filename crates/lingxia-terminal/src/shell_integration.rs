//! Shell integration: spawn-time wiring that makes interactive shells
//! emit OSC 133 command marks and OSC 7 cwd reports.
//!
//! The scripts below are crate resources — auditable, versioned with
//! the crate, and never written into user rc files. They are staged
//! into a per-process temp directory and injected through standard
//! shell mechanisms (bash `--rcfile`, zsh `ZDOTDIR`, fish `-C`,
//! PowerShell prompt wrapping), each sourcing the user's own startup
//! files first. Shells without a plan keep working as plain terminals.

use std::path::{Path, PathBuf};

/// How to adjust a shell invocation so it emits integration sequences.
pub struct ShellIntegrationPlan {
    /// Replacement arguments; `None` keeps the resolved default args.
    pub args: Option<Vec<String>>,
    /// Extra environment variables for the spawned shell.
    pub env: Vec<(String, String)>,
}

const BASH_INTEGRATION: &str = r#"# LingXia shell integration for bash.
# Sourced via --rcfile; the user's own bashrc runs first.
[ -f "$HOME/.bashrc" ] && . "$HOME/.bashrc"
__lingxia_prompt_command() {
    local __exit_code=$?
    printf '\e]133;D;%s\e\\' "$__exit_code"
    printf '\e]7;file://%s%s\e\\' "${HOSTNAME:-localhost}" "$PWD"
    printf '\e]133;A\e\\'
}
if [[ "$PROMPT_COMMAND" != *"__lingxia_prompt_command"* ]]; then
    PROMPT_COMMAND="__lingxia_prompt_command${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
fi
# PS0 (bash >= 4.4) marks command output start; harmless on older bash.
PS0='\e]133;C\e\\'
"#;

const ZSH_INTEGRATION: &str = r#"# LingXia shell integration for zsh.
# Lives in a staging ZDOTDIR; the user's own zshrc runs first.
__lingxia_orig_zdotdir="${LINGXIA_ORIGINAL_ZDOTDIR:-$HOME}"
[ -f "$__lingxia_orig_zdotdir/.zshrc" ] && . "$__lingxia_orig_zdotdir/.zshrc"
__lingxia_precmd() {
    local __exit_code=$?
    printf '\e]133;D;%s\e\\' "$__exit_code"
    printf '\e]7;file://%s%s\e\\' "${HOST:-localhost}" "$PWD"
    printf '\e]133;A\e\\'
}
__lingxia_preexec() {
    printf '\e]133;C\e\\'
}
autoload -Uz add-zsh-hook
add-zsh-hook precmd __lingxia_precmd
add-zsh-hook preexec __lingxia_preexec
"#;

const FISH_INTEGRATION: &str = r#"# LingXia shell integration for fish.
function __lingxia_prompt_mark --on-event fish_prompt
    set -l __exit_code $status
    printf '\e]133;D;%s\e\\' "$__exit_code"
    printf '\e]7;file://%s%s\e\\' (hostname) "$PWD"
    printf '\e]133;A\e\\'
end
function __lingxia_output_mark --on-event fish_postexec
    printf '\e]133;C\e\\'
end
"#;

/// PowerShell prompt wrapper emitting OSC 133/7. Builds on the cwd
/// integration already injected for PowerShell shells.
#[cfg(windows)]
const POWERSHELL_INTEGRATION: &str = r#"
$global:__LingXiaOriginalPrompt = $function:prompt
function global:prompt {
    $exitCode = 0
    if ($global:LASTEXITCODE -is [int]) { $exitCode = $global:LASTEXITCODE }
    $cwd = $executionContext.SessionState.Path.CurrentFileSystemLocation.Path
    try {
        [Environment]::CurrentDirectory = $cwd
    } catch {}
    $esc = [char]27
    $encodedCwd = [uri]::EscapeDataString($cwd)
    $marks = "$esc]133;D;$exitCode$esc\$esc]7;file://localhost/$encodedCwd$esc\$esc]133;A$esc\"
    if ($global:__LingXiaOriginalPrompt) {
        $marks + (& $global:__LingXiaOriginalPrompt)
    } else {
        $marks + "PS $cwd> "
    }
}
"#;

fn staging_dir() -> Option<PathBuf> {
    let dir = std::env::temp_dir().join(format!(
        "lingxia-shell-integration-{}-{}",
        env!("CARGO_PKG_VERSION"),
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn stage_script(dir: &Path, name: &str, contents: &str) -> Option<PathBuf> {
    let path = dir.join(name);
    let current = std::fs::read_to_string(&path).ok();
    if current.as_deref() != Some(contents) {
        std::fs::write(&path, contents).ok()?;
    }
    Some(path)
}

/// Compute the integration plan for a resolved shell path, or `None`
/// when the shell is unknown or scripts cannot be staged.
pub fn plan_for(shell_path: &str) -> Option<ShellIntegrationPlan> {
    let name = Path::new(shell_path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase())?;
    let name = name.strip_suffix(".exe").unwrap_or(&name);

    match name {
        "bash" => {
            let script = stage_script(
                &staging_dir()?,
                "lingxia-integration.bash",
                BASH_INTEGRATION,
            )?;
            Some(ShellIntegrationPlan {
                args: Some(vec![
                    "--rcfile".to_string(),
                    script.to_string_lossy().into_owned(),
                    "-i".to_string(),
                ]),
                env: Vec::new(),
            })
        }
        "zsh" => {
            let dir = staging_dir()?.join("zsh");
            std::fs::create_dir_all(&dir).ok()?;
            stage_script(&dir, ".zshrc", ZSH_INTEGRATION)?;
            let original = std::env::var("ZDOTDIR").unwrap_or_default();
            Some(ShellIntegrationPlan {
                args: None,
                env: vec![
                    ("ZDOTDIR".to_string(), dir.to_string_lossy().into_owned()),
                    ("LINGXIA_ORIGINAL_ZDOTDIR".to_string(), original),
                ],
            })
        }
        "fish" => {
            let script = stage_script(
                &staging_dir()?,
                "lingxia-integration.fish",
                FISH_INTEGRATION,
            )?;
            Some(ShellIntegrationPlan {
                args: Some(vec![
                    "-i".to_string(),
                    "-C".to_string(),
                    format!("source {}", script.to_string_lossy()),
                ]),
                env: Vec::new(),
            })
        }
        #[cfg(windows)]
        "pwsh" | "powershell" => Some(ShellIntegrationPlan {
            args: Some(vec![
                "-NoLogo".to_string(),
                "-NoExit".to_string(),
                "-Command".to_string(),
                POWERSHELL_INTEGRATION.to_string(),
            ]),
            env: Vec::new(),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_for_bash_stage_script_and_rcfile_args() {
        let plan = plan_for("/bin/bash").expect("bash plan");
        let args = plan.args.expect("replacement args");
        assert_eq!(args.first().map(String::as_str), Some("--rcfile"));
        assert_eq!(args.last().map(String::as_str), Some("-i"));
        let script = std::fs::read_to_string(&args[1]).unwrap();
        assert!(script.contains("133;A"));
        assert!(script.contains(".bashrc"), "user rc must still run");
    }

    #[test]
    fn plans_for_zsh_stage_zdotdir_without_touching_args() {
        let plan = plan_for("/usr/bin/zsh").expect("zsh plan");
        assert!(plan.args.is_none(), "zsh keeps its default args");
        let zdotdir = plan
            .env
            .iter()
            .find(|(key, _)| key == "ZDOTDIR")
            .map(|(_, value)| value.clone())
            .expect("ZDOTDIR override");
        let script = std::fs::read_to_string(Path::new(&zdotdir).join(".zshrc")).unwrap();
        assert!(script.contains("add-zsh-hook"));
        assert!(script.contains("LINGXIA_ORIGINAL_ZDOTDIR"));
    }

    #[test]
    fn plans_for_fish_source_staged_script() {
        let plan = plan_for("/opt/homebrew/bin/fish").expect("fish plan");
        let args = plan.args.expect("fish args");
        assert_eq!(args[0], "-i");
        assert_eq!(args[1], "-C");
        assert!(args[2].starts_with("source "));
        let script = std::fs::read_to_string(args[2].trim_start_matches("source ")).unwrap();
        assert!(script.contains("fish_postexec"));
    }

    #[test]
    fn unknown_shells_have_no_plan() {
        assert!(plan_for("/bin/sh").is_none());
        assert!(plan_for("/usr/bin/python3").is_none());
        assert!(plan_for("").is_none());
    }
}
