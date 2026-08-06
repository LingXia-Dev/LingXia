//! Terminal configuration for Apple hosts: the command line, and the
//! environment a spawned session needs to find it.
//!
//! Loading, watching and applying live in the shared configuration crate, so
//! Windows gets the same behaviour from the same code.

use std::path::PathBuf;

pub use lingxia_terminal_config::runtime::{
    apply_theme, current_json, generation, load, set_installed_fonts, watched_directory,
};

/// Run the `term` CLI if this process was invoked as one.
///
/// Hosts call this from `main` **before** touching any UI framework: the
/// product's executable doubles as its command line, and a configuration
/// command must not open a window. Returns the exit code when it handled the
/// invocation, `None` when the process should carry on and become the app.
pub fn run_cli_if_invoked(app_data_dir: PathBuf, system_is_dark: bool) -> Option<i32> {
    use std::io::IsTerminal;

    let mut args = std::env::args();
    let executable = args.next().unwrap_or_default();
    let command = std::path::Path::new(&executable)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("app")
        .to_string();

    let first = args.next();
    // Invoked from a terminal, this process is a command line — never an app.
    // Anything else means the OS launched the bundle, which is the app
    // starting normally. Without this rule an unrecognized argument falls
    // through to startup and dies against a running instance's databases.
    let from_terminal = std::io::stdout().is_terminal() || std::io::stdin().is_terminal();
    if !from_terminal && first.as_deref() != Some("term") {
        return None;
    }
    if first.as_deref() != Some("term") {
        let arguments: Vec<String> = std::env::args().skip(1).collect();
        let output = lingxia_terminal_config::cli::unknown(&command, &arguments);
        eprintln!("{}", output.text);
        return Some(output.code);
    }

    let rest: Vec<String> = std::env::args().skip(2).collect();
    let output = lingxia_terminal_config::cli::run(&app_data_dir, &command, &rest, system_is_dark);
    if output.code == 0 {
        println!("{}", output.text);
    } else {
        eprintln!("{}", output.text);
    }
    Some(output.code)
}

/// Make the product's command typable: write the launcher and return the
/// environment a spawned session needs to find it.
///
/// The executable lives inside an application bundle, which is neither on
/// `PATH` nor pleasant to type, so sessions we spawn get a launcher directory
/// prepended and the executable's own path, which is what tells a program
/// running inside the terminal that it has one to talk to.
pub fn session_environment(app_data_dir: &std::path::Path) -> Vec<(String, String)> {
    let mut environment = Vec::new();
    match lingxia_terminal_config::cli::install_launcher(app_data_dir) {
        Ok(launcher) => {
            log::info!("terminal command: {}", launcher.display());
            environment.push((
                "LINGXIA_TERMINAL_CLI".to_string(),
                launcher.to_string_lossy().into_owned(),
            ));
            let directory = lingxia_terminal_config::cli::bin_dir(app_data_dir);
            let path = std::env::var("PATH").unwrap_or_default();
            environment.push((
                "PATH".to_string(),
                format!("{}:{path}", directory.to_string_lossy()),
            ));
        }
        Err(error) => log::warn!("terminal command launcher not installed: {error}"),
    }
    environment
}
