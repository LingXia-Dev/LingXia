# LingXia Runner — Windows

The Windows Runner executable. `lingxia dev` in an lxapp project launches the
installed runner; it does not rebuild the executable per run.

## Local install

Install a locally built runner where `lingxia dev` looks for it
(`%USERPROFILE%\.lingxia\runner\<version>\`):

```powershell
powershell -ExecutionPolicy Bypass -File tools\lingxia-runner\windows\install-local-runner.ps1
```

Re-run it after changing runner code. The installed runner statically links the
MSVC CRT, so it runs on machines without a Visual C++ redistributable.

The script installs the release-profile build by default; while iterating on
the runner itself, install a debug build:

```powershell
$env:RUNNER_BUILD_PROFILE = "debug"
powershell -ExecutionPolicy Bypass -File tools\lingxia-runner\windows\install-local-runner.ps1
```
