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

# Asset name scheme matches .github/workflows/release-cli.yml exactly.
$Asset   = "lingxia-windows-$Arch.exe"
$BinName = "lingxia.exe"

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
    $AssetPath   = Join-Path $TmpDir $Asset
    $ShasumsPath = Join-Path $TmpDir $Shasums

    Write-Info "Downloading $Asset ..."
    try {
        Invoke-WebRequest -Uri "$BaseUrl/$Asset" -OutFile $AssetPath `
            -Headers @{ 'User-Agent' = 'lingxia-installer' }
    } catch {
        Stop-WithError "failed to download $Asset from $BaseUrl : $($_.Exception.Message)"
    }

    Write-Info "Downloading checksums ..."
    try {
        Invoke-WebRequest -Uri "$BaseUrl/$Shasums" -OutFile $ShasumsPath `
            -Headers @{ 'User-Agent' = 'lingxia-installer' }
    } catch {
        Stop-WithError "failed to download $Shasums from $BaseUrl : $($_.Exception.Message)"
    }

    # --- Verify checksum -----------------------------------------------------
    # The SHASUMS file covers every release asset; isolate the line for our
    # binary so we do not fail on files we did not download. Lines look like
    # "<hex>  <filename>" (the separator may be two spaces or " *").
    Write-Info "Verifying checksum ..."
    $line = Get-Content $ShasumsPath |
        Where-Object { $_ -match "[\s\*]$([regex]::Escape($Asset))\s*$" } |
        Select-Object -First 1
    if (-not $line) { Stop-WithError "no checksum entry for $Asset in $Shasums" }

    $ExpectedHash = ($line -split '\s+')[0]
    $ActualHash   = (Get-FileHash -Path $AssetPath -Algorithm SHA256).Hash
    # PowerShell string comparison is case-insensitive by default, so this works
    # whether the SHASUMS file uses lower- or upper-case hex.
    if ($ActualHash -ne $ExpectedHash) {
        Stop-WithError "checksum verification failed for $Asset (expected $ExpectedHash, got $ActualHash)"
    }

    # --- Install -------------------------------------------------------------
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }
    $Dest = Join-Path $InstallDir $BinName
    Move-Item -Path $AssetPath -Destination $Dest -Force
}
finally {
    Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue
}

Write-Info ""
Write-Info "Installed lingxia $Version to $Dest; run ``$BinName --version``"

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
