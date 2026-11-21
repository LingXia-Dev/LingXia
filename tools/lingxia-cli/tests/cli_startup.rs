// `dirs::home_dir` follows `HOME` only on Unix; elsewhere these tests would
// touch the real home directory.
#![cfg(unix)]

use std::process::Command;

#[test]
fn commands_leave_a_home_without_agent_tooling_alone() {
    let temp = tempfile::TempDir::new().unwrap();
    let home = temp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    for args in [&["version"][..], &["auth", "status", "--json"][..]] {
        let output = Command::new(env!("CARGO_BIN_EXE_lingxia"))
            .args(args)
            .env("HOME", &home)
            .env("LINGXIA_HOME", temp.path().join("state"))
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!String::from_utf8_lossy(&output.stderr).contains("agent skill"));
    }
    assert!(!home.join(".agents").exists());
    assert!(!home.join(".claude").exists());
}

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

    // An untouched copy an older CLI wrote straight into Claude's skills root.
    std::fs::create_dir_all(claude.parent().unwrap()).unwrap();
    assert!(install(&["skill", "install"]).status.success());
    std::fs::remove_file(&claude).unwrap();
    std::fs::rename(&canonical, &claude).unwrap();
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

    // A normal command from a dev build (this test binary) syncs nothing; the
    // release-build migration is covered by the unit tests.
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
        .expect("a command must not pollute JSON output");
    assert!(!canonical.exists());
    assert_eq!(
        std::fs::read_to_string(claude.join("SKILL.md")).unwrap(),
        "old"
    );
    assert_eq!(
        std::fs::read_to_string(project.join("AGENTS.md")).unwrap(),
        stale
    );

    // Clean break: the flag the stale pointer named is not accepted.
    assert!(!install(&["skill", "install", "--user"]).status.success());
}
