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
fn skill_install_links_claude_to_the_shared_copy_and_leaves_the_project_alone() {
    const MARKER: &str = "<!-- lingxia skill: AGENTS.md pointer -->";
    let temp = tempfile::TempDir::new().unwrap();
    let home = temp.path().join("home");
    let project = temp.path().join("project");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    // The pointer an older CLI wrote named a command that never existed.
    let stale = format!("# AGENTS\n\n{MARKER}\nlingxia skill install --user\n{MARKER}\n");
    std::fs::write(project.join("AGENTS.md"), &stale).unwrap();

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
    let canonical = home.join(".agents/skills/lingxia");
    let claude = home.join(".claude/skills/lingxia");
    let linked = || {
        std::fs::symlink_metadata(&claude).is_ok_and(|metadata| metadata.file_type().is_symlink())
            && std::fs::canonicalize(&claude).unwrap() == std::fs::canonicalize(&canonical).unwrap()
    };

    std::fs::create_dir_all(&claude).unwrap();
    std::fs::write(claude.join("SKILL.md"), "old").unwrap();
    let output = install(&["skill", "install"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(canonical.join("SKILL.md").is_file());
    assert!(linked(), "Claude Code keeps finding the skill");
    // A user-level install reports a stale pointer and never edits it.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("outdated LingXia pointer"), "{stdout}");
    assert!(stdout.contains("run `lingxia skill install`."), "{stdout}");
    assert_eq!(
        std::fs::read_to_string(project.join("AGENTS.md")).unwrap(),
        stale
    );

    let again = install(&["skill", "install"]);
    assert!(again.status.success());
    assert!(String::from_utf8_lossy(&again.stdout).contains("is current"));
    assert!(linked());

    // A normal command also migrates an old copy, without touching the project.
    std::fs::remove_dir_all(&canonical).unwrap();
    std::fs::remove_file(&claude).unwrap();
    std::fs::create_dir_all(&claude).unwrap();
    std::fs::write(claude.join("SKILL.md"), "old").unwrap();
    let synced = install(&["auth", "status", "--json"]);
    assert!(
        synced.status.success(),
        "{}",
        String::from_utf8_lossy(&synced.stderr)
    );
    serde_json::from_slice::<serde_json::Value>(&synced.stdout)
        .expect("sync must not pollute JSON output");
    assert!(canonical.join("SKILL.md").is_file());
    assert!(linked());
    assert_eq!(
        std::fs::read_to_string(project.join("AGENTS.md")).unwrap(),
        stale
    );

    // Clean break: the flag the stale pointer named is not accepted.
    assert!(!install(&["skill", "install", "--user"]).status.success());
}
