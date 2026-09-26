//! `lingxia dev stop` as a script's cleanup step: safe to run when nothing
//! is running, and again after that.

#[cfg(unix)]
#[test]
fn dev_stop_with_nothing_running_succeeds_every_time() {
    use std::process::Command;

    let home = tempfile::TempDir::new().unwrap();
    let project = tempfile::TempDir::new().unwrap();
    std::fs::write(project.path().join("lxapp.json"), "{}").unwrap();
    let stop = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_lingxia"))
            .args(["dev", "stop"])
            .args(args)
            // A broker of its own, not the user's.
            .env("HOME", home.path())
            .env("LINGXIA_HOME", home.path().join("state"))
            .env("LINGXIA_SKIP_SKILL", "1")
            .current_dir(project.path())
            .output()
            .unwrap()
    };
    for args in [&[][..], &[][..], &["ci"][..]] {
        let output = stop(args);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "{args:?}: {stdout}{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(stdout.contains("No dev session"), "{stdout}");
    }
    shut_down_broker(&home.path().join(".lingxia/broker.sock"));
}

/// The broker `dev stop` started exits once asked while idle.
#[cfg(unix)]
fn shut_down_broker(socket: &std::path::Path) {
    use std::io::{BufRead, BufReader, Write};
    let Ok(mut stream) = std::os::unix::net::UnixStream::connect(socket) else {
        return;
    };
    let _ = stream.write_all(b"{\"op\":\"shutdown\",\"v\":1}\n");
    let mut reply = String::new();
    let _ = BufReader::new(stream).read_line(&mut reply);
}
