// Exercise the exact runtime helper without constructing a WebView host.
#[path = "../../crates/lingxia-platform/src/windows/update/installer.rs"]
mod update_installer;
use std::path::Path;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(args.len(), 8);
    let pid = args[2].parse().unwrap();
    let mode = match args[1].as_str() {
        "nsis" => update_installer::Installer::Nsis {
            setup: Path::new(&args[3]),
            install_root: Path::new(&args[4]),
            executable: Path::new(&args[5]),
        },
        "portable" => update_installer::Installer::Portable {
            source: Path::new(&args[3]),
            launcher: Path::new(&args[4]),
            launcher_pid: args[5].parse().unwrap(),
        },
        _ => panic!("unknown installer"),
    };
    let script = update_installer::render(pid, mode, Path::new(&args[6]), Path::new(&args[7]));
    std::fs::write(&args[7], script).unwrap();
    // Reproduce a host launched with its payload as the working directory.
    if args[1] == "nsis" {
        std::env::set_current_dir(Path::new(&args[5]).parent().unwrap()).unwrap();
    }
    let mut helper = update_installer::command(Path::new(&args[7]))
        .spawn()
        .unwrap();
    std::env::set_current_dir(std::env::temp_dir()).unwrap();
    std::process::exit(helper.wait().unwrap().code().unwrap_or(1));
}
