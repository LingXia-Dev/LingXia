use serde_json::Value;
use std::process::Command;

fn run(args: &[&str]) -> std::process::Output {
    let temp = tempfile::TempDir::new().unwrap();
    Command::new(env!("CARGO_BIN_EXE_lingxia"))
        .arg("--skip-skill")
        .args(args)
        .current_dir(temp.path())
        .env("LINGXIA_HOME", temp.path().join("state"))
        .output()
        .unwrap()
}

#[test]
fn store_cli_json_errors_are_one_document_with_nonzero_exit() {
    for args in [
        vec!["store", "submit", "-p", "harmony", "--json"],
        vec!["store", "status", "-p", "ios", "--wait", "--json"],
        vec!["store", "status", "-p", "windows", "--json"],
        vec!["store", "submit", "-p", "invalid", "--json"],
    ] {
        let output = run(&args);
        assert!(!output.status.success());
        let body: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
            panic!(
                "{err}: stdout={}, stderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        });
        assert_eq!(body["schema_version"], 1);
        assert_eq!(body["ok"], false);
        assert_eq!(body["uploaded"], false);
        assert!(body["error"]["code"].is_string());
        assert!(body["results"].is_array());
    }
}

#[test]
fn store_cli_wait_arguments_validate_without_network() {
    for args in [
        vec!["store", "submit", "-p", "ios", "--wait", "--timeout", "0"],
        vec![
            "store",
            "submit",
            "-p",
            "ios",
            "--wait",
            "--poll-interval",
            "0",
        ],
        vec!["store", "status", "-p", "ios", "--version", "1.0"],
        vec![
            "store",
            "status",
            "-p",
            "ios",
            "--submission-id",
            "123",
            "--version",
            "1.0",
            "--build-number",
            "1",
        ],
        vec!["store", "submit", "-p", "ios", "--timeout", "20"],
    ] {
        assert_eq!(run(&args).status.code(), Some(2), "{args:?}");
    }
    for action in ["submit", "status"] {
        let output = run(&["store", action, "--help"]);
        assert!(output.status.success());
        let help = String::from_utf8(output.stdout).unwrap();
        assert!(help.contains("--wait"));
        assert!(help.contains("--json"));
    }
}
