#[cfg(target_os = "windows")]
mod device;
#[cfg(target_os = "windows")]
mod runner;

#[cfg(target_os = "windows")]
fn main() -> lingxia_windows::Result<()> {
    runner::run()
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!(
        "lingxia-runner is the Windows dev runner; on macOS use the LingXia Runner app instead."
    );
    std::process::exit(1);
}
