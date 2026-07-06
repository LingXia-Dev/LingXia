<#
.SYNOPSIS
  LingXia CLI installer for Windows PowerShell (rustup / deno / bun style).

.DESCRIPTION
  Downloads a prebuilt `lingxia.exe` binary from GitHub Releases, verifies its
  sha256 against the release SHASUMS file, and installs it to %USERPROFILE%\.local\bin
  (adding that directory to the user PATH).

  This is the PowerShell counterpart to install.sh. POSIX-shell users (Git Bash,
  MSYS, Cygwin) should use install.sh instead.

.EXAMPLE
  irm https://raw.githubusercontent.com/LingXia-Dev/LingXia/main/install.ps1 | iex

.EXAMPLE
  # Pin a version (set the env var first, then pipe):
  $env:LINGXIA_VERSION = "0.8.0"
  irm https://raw.githubusercontent.com/LingXia-Dev/LingXia/main/install.ps1 | iex

.NOTES
  Environment:
    LINGXIA_VERSION      Version to install, e.g. 0.8.0 (default: latest CLI release)
#>

$ErrorActionPreference = 'Stop'
# Invoke-WebRequest's progress bar slows downloads to a crawl on PS 5.1.
$ProgressPreference = 'SilentlyContinue'
# Windows PowerShell 5.1 defaults to TLS 1.0/1.1; GitHub requires TLS 1.2+.
try {
    [Net.ServicePointManager]::SecurityProtocol = `
        [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
} catch {}

function Write-Info { param([string]$Message) Write-Host $Message }
function Stop-WithError {
    param([string]$Message)
    Write-Host "error: $Message" -ForegroundColor Red
    exit 1
}

function Get-WindowsVersion {
    try {
        $currentVersion = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
        if ($null -ne $currentVersion.CurrentMajorVersionNumber) {
            $major = [int]$currentVersion.CurrentMajorVersionNumber
            $minor = if ($null -ne $currentVersion.CurrentMinorVersionNumber) { [int]$currentVersion.CurrentMinorVersionNumber } else { 0 }
            $build = if ($currentVersion.CurrentBuildNumber) { [int]$currentVersion.CurrentBuildNumber } else { 0 }
            return [version]::new($major, $minor, $build)
        }
    } catch {}

    return [Environment]::OSVersion.Version
}

function Assert-SupportedWindows {
    $version = Get-WindowsVersion
    if ($version.Major -lt 10) {
        Stop-WithError "LingXia for Windows requires Windows 10 or later (detected Windows $version)"
    }
}

Assert-SupportedWindows

$Repo       = 'LingXia-Dev/LingXia'
$InstallDir = Join-Path $HOME '.local\bin'
$TagPrefix  = 'lingxia-cli-v'

# --- Detect architecture -----------------------------------------------------
# PROCESSOR_ARCHITEW6432 is set when a 32-bit process runs on 64-bit Windows;
# it reports the *true* OS arch, so prefer it over PROCESSOR_ARCHITECTURE.
$rawArch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
$Arch = switch ($rawArch) {
    'AMD64' { 'x86_64' }
    'ARM64' { 'aarch64' }
    default { Stop-WithError "unsupported architecture '$rawArch'" }
}

# Binaries shipped together in the lingxia-cli release, installed as peers:
# the CLI (`lingxia`) and the devtools client (`lxdev`). Asset name scheme
# matches .github/workflows/release-cli.yml exactly.
$Binaries = @('lingxia', 'lxdev')

# --- Resolve version ---------------------------------------------------------
# The repo ships several components, each with its own tag prefix (e.g.
# lingxia-cli-v*, sdk-v*), so /releases/latest is NOT reliable -- it returns the
# newest release of ANY component. Instead we list releases (newest-first) and
# take the first whose tag starts with "lingxia-cli-v".
function Resolve-Version {
    if ($env:LINGXIA_VERSION) { return $env:LINGXIA_VERSION }

    try {
        $releases = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases" `
            -Headers @{ 'User-Agent' = 'lingxia-installer'; 'Accept' = 'application/vnd.github+json' }
    } catch {
        Stop-WithError "failed to query releases for $Repo : $($_.Exception.Message)"
    }

    $release = $releases | Where-Object { $_.tag_name -like "$TagPrefix*" } | Select-Object -First 1
    if (-not $release) { Stop-WithError "could not find a $TagPrefix release in $Repo" }
    return $release.tag_name.Substring($TagPrefix.Length)
}

$Version = Resolve-Version
$Tag     = "$TagPrefix$Version"
$BaseUrl = "https://github.com/$Repo/releases/download/$Tag"
$Shasums = "SHASUMS256-$Version.txt"

Write-Info "Installing lingxia $Version (windows/$Arch) from $Repo"

# --- Download into a temp dir ------------------------------------------------
$TmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ("lingxia-" + [System.IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $TmpDir -Force | Out-Null

try {
    # The SHASUMS file covers every release asset; download it once and verify
    # each binary against it.
    $ShasumsPath = Join-Path $TmpDir $Shasums
    Write-Info "Downloading checksums ..."
    try {
        Invoke-WebRequest -Uri "$BaseUrl/$Shasums" -OutFile $ShasumsPath `
            -Headers @{ 'User-Agent' = 'lingxia-installer' }
    } catch {
        Stop-WithError "failed to download $Shasums from $BaseUrl : $($_.Exception.Message)"
    }

    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    # Download, verify, and install each binary (the CLI and the devtools client).
    foreach ($bin in $Binaries) {
        $asset     = "$bin-windows-$Arch.exe"
        $binName   = "$bin.exe"
        $assetPath = Join-Path $TmpDir $asset

        Write-Info "Downloading $asset ..."
        try {
            Invoke-WebRequest -Uri "$BaseUrl/$asset" -OutFile $assetPath `
                -Headers @{ 'User-Agent' = 'lingxia-installer' }
        } catch {
            Stop-WithError "failed to download $asset from $BaseUrl : $($_.Exception.Message)"
        }

        # Isolate this asset's line (separator may be two spaces or " *").
        $line = Get-Content $ShasumsPath |
            Where-Object { $_ -match "[\s\*]$([regex]::Escape($asset))\s*$" } |
            Select-Object -First 1
        if (-not $line) { Stop-WithError "no checksum entry for $asset in $Shasums" }
        $expectedHash = ($line -split '\s+')[0]
        $actualHash   = (Get-FileHash -Path $assetPath -Algorithm SHA256).Hash
        if ($actualHash -ne $expectedHash) {
            Stop-WithError "checksum verification failed for $asset (expected $expectedHash, got $actualHash)"
        }

        $dest = Join-Path $InstallDir $binName
        Move-Item -Path $assetPath -Destination $dest -Force
        Write-Info "Installed $binName -> $dest"
    }

    $MetaPath = Join-Path $InstallDir 'lingxia-cli-install.json'
    $InstallPath = Join-Path $InstallDir 'lingxia.exe'
    $Metadata = [ordered]@{
        channel      = 'github-release'
        repo         = $Repo
        version      = $Version
        install_path = [System.IO.Path]::GetFullPath($InstallPath)
    }
    $MetadataJson = $Metadata | ConvertTo-Json -Depth 4
    $Utf8NoBom = [System.Text.UTF8Encoding]::new($false)
    [System.IO.File]::WriteAllText($MetaPath, $MetadataJson, $Utf8NoBom)
    Write-Info "Installed update metadata -> $MetaPath"
}
finally {
    Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue
}

Write-Info ""
Write-Info "Installed lingxia + lxdev $Version to $InstallDir; run ``lingxia.exe version``"

# --- Ensure the install dir is on PATH ---------------------------------------
# Persist to the *user* PATH (idempotent) and update the current session so the
# new binary is usable right away.
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$pathEntries = @($userPath -split ';' | Where-Object { $_ -ne '' })
if ($pathEntries -notcontains $InstallDir) {
    [Environment]::SetEnvironmentVariable('Path', (($pathEntries + $InstallDir) -join ';'), 'User')
    Write-Info ""
    Write-Info "Added $InstallDir to your user PATH."
    Write-Info "Open a new terminal (or restart your shell) for it to take effect."
}
# Reflect it in the current session regardless, so `lingxia` works immediately.
if (($env:Path -split ';') -notcontains $InstallDir) {
    $env:Path = "$env:Path;$InstallDir"
}
