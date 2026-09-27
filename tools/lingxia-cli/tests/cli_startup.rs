use std::process::Command;

#[test]
fn skip_skill_is_global_and_supports_the_ci_environment() {
    let temp = tempfile::TempDir::new().unwrap();
    for args in [
        vec!["--skip-skill", "version"],
        vec!["version", "--skip-skill"],
        vec!["version"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_lingxia"))
            .args(args)
            .env("LINGXIA_SKIP_SKILL", "1")
            .env("LINGXIA_HOME", temp.path().join("state"))
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!String::from_utf8_lossy(&output.stderr).contains("agent skill"));
    }
}

// `dirs::home_dir` follows `HOME` only on Unix; elsewhere this would write to
// the real home directory.
#[cfg(unix)]
#[test]
fn skill_install_writes_the_skill_and_refreshes_a_stale_pointer() {
    const MARKER: &str = "<!-- lingxia skill: AGENTS.md pointer -->";
    let temp = tempfile::TempDir::new().unwrap();
    let home = temp.path().join("home");
    let project = temp.path().join("project");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    // The pointer an older CLI wrote named a command that never existed.
    std::fs::write(
        project.join("AGENTS.md"),
        format!("# AGENTS\n\n{MARKER}\nlingxia skill install --user\n{MARKER}\n"),
    )
    .unwrap();

    let install = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_lingxia"))
            .args(args)
            .env("HOME", &home)
            .env("LINGXIA_HOME", temp.path().join("state"))
            .env_remove("LINGXIA_SKIP_SKILL")
            .current_dir(&project)
            .output()
            .unwrap()
    };

    let output = install(&["skill", "install"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(home.join(".claude/skills/lingxia/SKILL.md").is_file());
    let agents = std::fs::read_to_string(project.join("AGENTS.md")).unwrap();
    assert!(agents.contains("run `lingxia skill install`."), "{agents}");
    assert!(!agents.contains("--user"), "{agents}");

    let again = install(&["skill", "install"]);
    assert!(again.status.success());
    assert!(String::from_utf8_lossy(&again.stdout).contains("is current"));

    // Clean break: the flag the stale pointer named is not accepted.
    assert!(!install(&["skill", "install", "--user"]).status.success());
}
