param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RootDir = Resolve-Path (Join-Path $ScriptDir "..\..\..")
$RunnerCargoToml = Join-Path $ScriptDir "Cargo.toml"

function Read-RunnerVersion {
    foreach ($line in Get-Content $RunnerCargoToml) {
        if ($line -match '^version\s*=\s*"([^"]+)"') {
            return $Matches[1]
        }
    }
    throw "failed to read runner version from $RunnerCargoToml"
}

$RunnerVersion = if ($env:RUNNER_VERSION) { $env:RUNNER_VERSION } else { Read-RunnerVersion }
if ([string]::IsNullOrWhiteSpace($RunnerVersion)) {
    throw "runner version is empty"
}

$HomeDir = [Environment]::GetFolderPath("UserProfile")
if ([string]::IsNullOrWhiteSpace($HomeDir)) {
    throw "failed to resolve user profile directory"
}

$TargetDir = if ($env:RUNNER_TARGET_DIR) {
    $env:RUNNER_TARGET_DIR
} else {
    Join-Path $HomeDir ".lingxia\runner\$RunnerVersion"
}
$TargetParent = Split-Path -Parent $TargetDir
$TmpTargetDir = Join-Path $TargetParent ".tmp-runner-$RunnerVersion-$PID"
$BackupTargetDir = Join-Path $TargetParent ".prev-runner-$RunnerVersion-$PID"
$CargoBin = if ($env:CARGO_BIN) { $env:CARGO_BIN } else { "cargo" }
$BuildProfile = if ($env:RUNNER_BUILD_PROFILE) { $env:RUNNER_BUILD_PROFILE } else { "release" }
if ($BuildProfile -ne "debug" -and $BuildProfile -ne "release") {
    throw "unsupported RUNNER_BUILD_PROFILE: $BuildProfile (expected debug or release)"
}

$RunnerRoot = [IO.Path]::GetFullPath((Join-Path $HomeDir ".lingxia\runner")).TrimEnd([char[]]@('\', '/'))
$RunnerRootPrefix = $RunnerRoot + [IO.Path]::DirectorySeparatorChar
$TargetFull = [IO.Path]::GetFullPath($TargetDir).TrimEnd([char[]]@('\', '/'))
if (-not $TargetFull.StartsWith($RunnerRootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
    throw "refusing to clear non-runner directory: $TargetDir"
}

if (-not (Get-Command $CargoBin -ErrorAction SilentlyContinue)) {
    throw "missing cargo: $CargoBin"
}

Write-Host "==> Building Windows Runner ($BuildProfile)"
Push-Location $RootDir
$OriginalRustFlags = $env:RUSTFLAGS
try {
    $StaticCrtFlag = "-C target-feature=+crt-static"
    if ([string]::IsNullOrWhiteSpace($env:RUSTFLAGS)) {
        $env:RUSTFLAGS = $StaticCrtFlag
    } elseif ($env:RUSTFLAGS -notlike "*target-feature=+crt-static*") {
        $env:RUSTFLAGS = "$env:RUSTFLAGS $StaticCrtFlag"
    }
    $cargoArgs = @("build", "--package", "lingxia-runner-windows", "--bin", "lingxia-runner")
    if ($BuildProfile -eq "release") {
        $cargoArgs += "--release"
    }
    & $CargoBin @cargoArgs
    if ($LASTEXITCODE -ne 0) {
        throw "Windows Runner cargo build failed"
    }
} finally {
    $env:RUSTFLAGS = $OriginalRustFlags
    Pop-Location
}

$CargoTargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $RootDir "target" }
$RunnerExe = Join-Path $CargoTargetDir "$BuildProfile\lingxia-runner.exe"
if (-not (Test-Path $RunnerExe -PathType Leaf)) {
    throw "Windows Runner executable not found after build: $RunnerExe"
}

Write-Host "==> Installing Windows Runner to $TargetDir"
Remove-Item -LiteralPath $TmpTargetDir, $BackupTargetDir -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $TmpTargetDir | Out-Null
Copy-Item -LiteralPath $RunnerExe -Destination (Join-Path $TmpTargetDir "lingxia-runner.exe") -Force
Set-Content -LiteralPath (Join-Path $TmpTargetDir "VERSION") -Value $RunnerVersion -NoNewline
New-Item -ItemType Directory -Force -Path $TargetParent | Out-Null
if (Test-Path $TargetDir) {
    Move-Item -LiteralPath $TargetDir -Destination $BackupTargetDir
}
Move-Item -LiteralPath $TmpTargetDir -Destination $TargetDir
Remove-Item -LiteralPath $BackupTargetDir -Recurse -Force -ErrorAction SilentlyContinue

Write-Host "Done: $(Join-Path $TargetDir 'lingxia-runner.exe')"
