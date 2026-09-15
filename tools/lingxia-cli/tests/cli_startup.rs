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
