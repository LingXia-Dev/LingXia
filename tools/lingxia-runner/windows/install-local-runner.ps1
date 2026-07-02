param(
    # Optional private provider crate(s) to inject for this build (e.g. `cloud`),
    # mirroring the CLI's `--with-provider`. Falls back to LINGXIA_WITH_PROVIDERS.
    [string]$WithProvider = "",
    # Local checkout of the provider crate. Falls back to
    # LINGXIA_PROVIDER_<NAME>_PATH (uppercased provider name).
    [string]$ProviderPath = ""
)

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

# --- Optional provider injection -------------------------------------------
# A self-contained port of the CLI's `provider::inject` for the Windows runner
# (the macOS runner gets this for free via `lingxia build`). It activates the
# host crate's inert `<name>` feature by appending `dep:<provider-crate>`, adds
# the provider as an optional path dependency, and extends the workspace root's
# [patch.crates-io] with the provider's workspace-shared deps so types unify.
# All manifest edits are reverted in the `finally` after the build.
if (-not $WithProvider) {
    $WithProvider = if ($env:LINGXIA_WITH_PROVIDERS) { $env:LINGXIA_WITH_PROVIDERS } else { "" }
}

$ManifestBackups = @()        # @( @{ Path; Bytes } ) for exact restore
$ProviderFeatures = @()       # host features to enable in the cargo build
$RootCargoToml = Join-Path $RootDir "Cargo.toml"

function Restore-Manifests {
    foreach ($b in $script:ManifestBackups) {
        try { [IO.File]::WriteAllBytes($b.Path, $b.Bytes) } catch { Write-Warning "failed to restore $($b.Path): $_" }
    }
    $script:ManifestBackups = @()
}

function Cargo-Metadata([string]$manifestDir) {
    $out = & $CargoBin metadata --no-deps --format-version 1 --manifest-path (Join-Path $manifestDir "Cargo.toml")
    if ($LASTEXITCODE -ne 0) { throw "cargo metadata failed for $manifestDir" }
    return ($out | ConvertFrom-Json)
}

if ($WithProvider) {
    $ProviderName = ($WithProvider -split ',')[0].Trim()
    if (-not $ProviderPath) {
        $envKey = "LINGXIA_PROVIDER_$($ProviderName.ToUpper())_PATH"
        $ProviderPath = [Environment]::GetEnvironmentVariable($envKey)
    }
    if ([string]::IsNullOrWhiteSpace($ProviderPath)) {
        $gitKey = "LINGXIA_PROVIDER_$($ProviderName.ToUpper())_GIT"
        $ProviderGit = [Environment]::GetEnvironmentVariable($gitKey)
        if (-not [string]::IsNullOrWhiteSpace($ProviderGit)) {
            if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
                throw "provider '$ProviderName' requested from $gitKey but git is missing"
            }
            $ProviderPath = Join-Path ([IO.Path]::GetTempPath()) "lingxia-provider-$ProviderName"
            Remove-Item -LiteralPath $ProviderPath -Recurse -Force -ErrorAction SilentlyContinue
            $cloneArgs = @("clone")
            $refKey = "LINGXIA_PROVIDER_$($ProviderName.ToUpper())_REF"
            $ProviderRef = [Environment]::GetEnvironmentVariable($refKey)
            if (-not [string]::IsNullOrWhiteSpace($ProviderRef)) {
                $cloneArgs += @("--branch", $ProviderRef)
            }
            $cloneArgs += @($ProviderGit, $ProviderPath)
            & git @cloneArgs
            if ($LASTEXITCODE -ne 0) { throw "git clone provider '$ProviderName' failed" }
            $revKey = "LINGXIA_PROVIDER_$($ProviderName.ToUpper())_REV"
            $ProviderRev = [Environment]::GetEnvironmentVariable($revKey)
            if (-not [string]::IsNullOrWhiteSpace($ProviderRev)) {
                & git -C $ProviderPath checkout $ProviderRev
                if ($LASTEXITCODE -ne 0) { throw "git checkout provider '$ProviderName' rev failed" }
            }
        }
    }
    if ([string]::IsNullOrWhiteSpace($ProviderPath)) {
        throw "provider '$ProviderName' requested but no --ProviderPath / LINGXIA_PROVIDER_$($ProviderName.ToUpper())_PATH / LINGXIA_PROVIDER_$($ProviderName.ToUpper())_GIT"
    }
    $ProviderPath = (Resolve-Path $ProviderPath).Path
    Write-Host "==> Injecting provider '$ProviderName' from $ProviderPath"

    # Provider crate name + its dependency package names.
    $pmeta = Cargo-Metadata $ProviderPath
    $pManifest = (Join-Path $ProviderPath "Cargo.toml") -replace '/', '\'
    $ppkg = $pmeta.packages | Where-Object { ($_.manifest_path -replace '/', '\') -ieq $pManifest } | Select-Object -First 1
    if (-not $ppkg) { throw "provider crate not found at $ProviderPath" }
    $CrateName = $ppkg.name
    $providerDeps = @($ppkg.dependencies | ForEach-Object { $_.name } | Sort-Object -Unique)

    # Workspace members (name -> crate dir) of the runner workspace.
    $wmeta = Cargo-Metadata $RootDir
    $members = @{}
    foreach ($p in $wmeta.packages) { $members[$p.name] = (Split-Path -Parent ($p.manifest_path -replace '/', '\')) }

    # Host-requested provider features from the runner manifest metadata.
    $runnerText = [IO.File]::ReadAllText($RunnerCargoToml)
    $featList = @()
    $mFeat = [regex]::Match($runnerText, "(?ms)^\[package\.metadata\.lingxia\.providers\.$([regex]::Escape($ProviderName))\].*?^\s*features\s*=\s*\[(.*?)\]")
    if ($mFeat.Success) {
        $featList = @([regex]::Matches($mFeat.Groups[1].Value, '"([^"]+)"') | ForEach-Object { $_.Groups[1].Value })
    }
    $featsToml = (($featList | ForEach-Object { "`"$_`"" }) -join ", ")

    # Patch the runner manifest: activate the feature + add the provider dep.
    $ManifestBackups += @{ Path = $RunnerCargoToml; Bytes = [IO.File]::ReadAllBytes($RunnerCargoToml) }
    $lines = $runnerText -split "`n"
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match "^$([regex]::Escape($ProviderName))\s*=\s*\[(.*)\]\s*$") {
            $inner = $Matches[1].Trim()
            if ($inner -notmatch "dep:$([regex]::Escape($CrateName))") {
                $newInner = if ($inner) { "$inner, `"dep:$CrateName`"" } else { "`"dep:$CrateName`"" }
                $lines[$i] = "$ProviderName = [$newInner]"
            }
            break
        }
    }
    $patched = ($lines -join "`n").TrimEnd() + "`n"
    if ($patched -notmatch "(?m)^\[dependencies\.$([regex]::Escape($CrateName))\]") {
        $depPath = $ProviderPath -replace '\\', '/'
        $patched += "`n[dependencies.$CrateName]`npath = `"$depPath`"`noptional = true`nfeatures = [$featsToml]`n"
    }
    [IO.File]::WriteAllText($RunnerCargoToml, $patched)

    # Patch the workspace root [patch.crates-io] with provider deps that are
    # workspace members, so the provider and host share one crate instance.
    $rootText = [IO.File]::ReadAllText($RootCargoToml)
    $entries = @()
    foreach ($d in $providerDeps) {
        if ($members.ContainsKey($d)) {
            $entries += "$d = { path = `"$($members[$d] -replace '\\','/')`" }"
        }
    }
    if ($entries.Count -gt 0) {
        $ManifestBackups += @{ Path = $RootCargoToml; Bytes = [IO.File]::ReadAllBytes($RootCargoToml) }
        $rootLines = $rootText -split "`n"
        $outLines = New-Object System.Collections.Generic.List[string]
        $inserted = $false
        foreach ($rl in $rootLines) {
            $outLines.Add($rl)
            if (-not $inserted -and $rl.Trim() -eq "[patch.crates-io]") {
                foreach ($e in $entries) { $outLines.Add($e) }
                $inserted = $true
            }
        }
        if (-not $inserted) { throw "workspace root $RootCargoToml has no [patch.crates-io] table to extend" }
        [IO.File]::WriteAllText($RootCargoToml, ($outLines -join "`n"))
        Write-Host ("    + [patch.crates-io]: {0}" -f (($entries | ForEach-Object { ($_ -split ' ')[0] }) -join ', '))
    }

    # Back up the lockfile cargo will rewrite, so injected entries never leak.
    $rootLock = Join-Path $RootDir "Cargo.lock"
    if (Test-Path $rootLock) {
        $ManifestBackups += @{ Path = $rootLock; Bytes = [IO.File]::ReadAllBytes($rootLock) }
    }

    $ProviderFeatures += $ProviderName
    Write-Host "    + feature '$ProviderName' -> dep:$CrateName"
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
    if ($ProviderFeatures.Count -gt 0) {
        $cargoArgs += @("--features", ($ProviderFeatures -join ","))
    }
    # cargo writes normal progress ("Updating crates.io index", "Compiling …")
    # to stderr; under captured output + EAP=Stop, PS 5.1 wraps each line as a
    # terminating NativeCommandError. Relax EAP across the build and gate on the
    # exit code instead.
    $prevEap = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    & $CargoBin @cargoArgs
    $buildExit = $LASTEXITCODE
    $ErrorActionPreference = $prevEap
    if ($buildExit -ne 0) {
        throw "Windows Runner cargo build failed"
    }
} finally {
    $env:RUSTFLAGS = $OriginalRustFlags
    Pop-Location
    Restore-Manifests
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
