# LingXia Windows Runner

Windows dev runner launched by `lingxia dev` for standalone lxapp projects.

This crate is intentionally separate from `tools/lingxia-runner/macos`.
The macOS runner links `macos/runner-lib` as a Swift static library; this crate
builds the Windows executable and depends on the `lingxia-windows-sdk` host entry
crate.

## Local install

`lingxia dev` launches a versioned local runner instead of rebuilding the
executable on every run. Install it from the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File tools\lingxia-runner\windows\install-local-runner.ps1
```

The script installs `lingxia-runner.exe` to
`%USERPROFILE%\.lingxia\runner\<version>\`.

The installed runner statically links the MSVC CRT so it can run on machines
without a separately installed Visual C++ redistributable.

The script installs the release-profile runner by default. To use a debug
build while iterating on the runner itself:

```powershell
$env:RUNNER_BUILD_PROFILE = "debug"
powershell -ExecutionPolicy Bypass -File tools\lingxia-runner\windows\install-local-runner.ps1
```
