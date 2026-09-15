use std::process::Command;

#[test]
fn cli_only_upgrade_does_not_inspect_or_modify_the_host_project() {
    let temp = tempfile::TempDir::new().unwrap();
    // A project upgrade would try to parse this; CLI-only must not enter it.
    let manifest = temp.path().join("lingxia.yaml");
    std::fs::write(&manifest, "invalid: [").unwrap();
    for check in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lingxia"));
        command.args([
            "upgrade",
            "--cli-only",
            "--version",
            env!("CARGO_PKG_VERSION"),
        ]);
        if check {
            command.arg("--check");
        }
        let output = command
            .env("LINGXIA_HOME", temp.path().join("state"))
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(std::fs::read_to_string(&manifest).unwrap(), "invalid: [");
        assert!(!temp.path().join("state/runner").exists());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("agent skill"));
    }
}
