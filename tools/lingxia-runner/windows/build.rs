fn main() {
    // Embed the dev runner's identity icon (the committed runner.ico, generated
    // from the macOS Runner's 1024px AppIcon via `lingxia icon … --output`)
    // as an .exe resource so the loose release exe shows its icon in Explorer /
    // taskbar / Alt-Tab before the process runs — the runner zip ships only the
    // exe, with no assets.
    lingxia_windows_build::configure_windows_app_with_icon("app.manifest", "runner.ico");
}
