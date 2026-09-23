# NSIS/portable/MSIX packaging plus real installation, update, and data retention.
# Run on a Windows build host with NSIS 3, the Windows SDK and WebView2 installed.
param([string]$Root = (Join-Path $env:TEMP ('lingxia-package-smoke-' + [Guid]::NewGuid().ToString('N'))))
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$Root = [IO.Path]::GetFullPath($Root)
New-Item -ItemType Directory -Force $Root | Out-Null
$identity = 'com.lingxia.smoke.' + [Guid]::NewGuid().ToString('N')
$install = Join-Path $Root "O'Brien 安装 App"
$portable = Join-Path $Root "O'Brien 便携 App.exe"
$state = Join-Path $Root 'user-state'
New-Item -ItemType Directory -Force $state | Out-Null
Set-Content -LiteralPath (Join-Path $state 'keep.txt') -Value 'keep me'
$savedEnvironment = @{}
foreach ($key in @('LINGXIA_STATE_ROOT', 'LINGXIA_SMOKE_REPORT', 'LINGXIA_SMOKE_WAIT_MS', 'LINGXIA_MAKENSIS', 'LINGXIA_PACKAGING_TEST_ROOT')) {
    $savedEnvironment[$key] = [Environment]::GetEnvironmentVariable($key, 'Process')
}
$env:LINGXIA_STATE_ROOT = $state
$env:LINGXIA_SMOKE_REPORT = Join-Path $Root 'report.txt'
$makensisCandidates = @(
    (Join-Path ${env:ProgramFiles(x86)} 'NSIS\makensis.exe'),
    (Join-Path $env:ProgramFiles 'NSIS\makensis.exe')
)
$makensis = $makensisCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
if (-not $makensis) {
    $fromPath = Get-Command makensis -ErrorAction SilentlyContinue
    if ($fromPath) { $makensis = $fromPath.Source }
}
if (-not $makensis) { throw 'makensis.exe not found. Install NSIS 3 before the packaging smoke test.' }
$env:LINGXIA_MAKENSIS = $makensis
function Assert($Condition, [string]$Message) { if (-not $Condition) { throw $Message } }
function Run-Exe([string]$File, [string]$Arguments = '', [int]$ExpectedExit = 0) {
    $params = @{ FilePath = $File; PassThru = $true }
    if ($Arguments) { $params.ArgumentList = $Arguments }
    Write-Host "Running: $File $Arguments"
    $p = Start-Process @params
    try {
        if (-not $p.WaitForExit(120000)) { $p.Kill(); throw "Timed out: $File" }
        Assert ($p.ExitCode -eq $ExpectedExit) "Unexpected exit $($p.ExitCode) (expected $ExpectedExit): $File"
    } finally { $p.Dispose() }
}
function Read-Report { Get-Content -LiteralPath $env:LINGXIA_SMOKE_REPORT -Encoding UTF8 }
$fixtureSource = @'
#![windows_subsystem = "windows"]
fn main() {
    let env = |key| std::env::var(key).unwrap_or_default();
    let report = format!("VERSION=__VERSION__\nCWD={}\nARGS={}\nSTATE={}\nLAUNCHER={}\n",
        std::env::current_dir().unwrap().display(),
        std::env::args().skip(1).collect::<Vec<_>>().join("|"),
        env("LINGXIA_STATE_ROOT"), env("LINGXIA_PORTABLE_EXECUTABLE"));
    std::fs::write(env("LINGXIA_SMOKE_REPORT"), report).unwrap();
    if let Ok(wait) = env("LINGXIA_SMOKE_WAIT_MS").parse::<u64>() {
        std::thread::sleep(std::time::Duration::from_millis(wait));
    }
}
'@
Push-Location $repo
try {
    rustc --edition 2024 scripts/ci/windows-package-helper.rs -o (Join-Path $Root 'helper.exe')
    Assert ($LASTEXITCODE -eq 0) 'Failed to compile helper renderer'
    foreach ($version in @('1.0.0', '1.1.0')) {
        $project = Join-Path $Root $version
        $payload = Join-Path $project 'payload'
        New-Item -ItemType Directory -Force (Join-Path $payload 'assets') | Out-Null
        $source = Join-Path $project 'demo.rs'
        $fixtureSource.Replace('__VERSION__', $version) | Set-Content -LiteralPath $source -Encoding UTF8
        rustc --edition 2024 --crate-name packaging_demo $source -o "$payload\demo.exe"
        Assert ($LASTEXITCODE -eq 0) 'Failed to compile payload'
        # Avoid the UTF-8 BOM rejected by serde_json.
        [IO.File]::WriteAllText((Join-Path $payload 'assets/app.json'), (@{ windowsAppId = $identity; productName = 'LingXia Packaging Smoke'; productVersion = $version; env = 'prod' } | ConvertTo-Json))
        $yaml = @"
app:
  projectName: smoke
  productName: LingXia Packaging Smoke
  productVersion: $version
  packageId: $identity
  platforms: [windows]
windows:
  portableData: false
"@
        [IO.File]::WriteAllText((Join-Path $project 'lingxia.yaml'), $yaml)
        $env:LINGXIA_PACKAGING_TEST_ROOT = $project
        cargo test -p lingxia-cli --bin lingxia windows_distribution_smoke_fixture -- --ignored --nocapture
        Assert ($LASTEXITCODE -eq 0) "Packaging failed for $version"
    }
    $v1 = Join-Path $Root '1.0.0/dist/windows'
    $v2 = Join-Path $Root '1.1.0/dist/windows'
    Run-Exe (Join-Path $v1 'smoke-1.0.0-x64-Setup.exe') "/S /D=$install"
    $app = Join-Path $install 'app/demo.exe'
    Assert (Test-Path $app) 'Installed executable missing'
    Assert ((Get-Content (Join-Path $install 'app/.lingxia-distribution') -Raw) -eq 'nsis') 'Missing NSIS marker'
    Run-Exe $app 'hello "two words"'
    Assert ((Read-Report) -contains 'VERSION=1.0.0') 'Installed app did not run'
    Assert ((Read-Report) -contains 'ARGS=hello|two words') 'App arguments changed'
    $registry = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$identity"
    Assert ((Get-ItemProperty $registry).DisplayVersion -eq '1.0.0') 'Uninstall registration missing'
    $shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) "LingXia Packaging Smoke ($identity).lnk"
    Assert (Test-Path $shortcut) 'Start Menu shortcut missing'

    # A live image must block replacement before staging starts.
    $env:LINGXIA_SMOKE_WAIT_MS = '30000'
    $running = Start-Process -FilePath $app -PassThru
    Remove-Item Env:LINGXIA_SMOKE_WAIT_MS
    try {
        Run-Exe (Join-Path $v2 'smoke-1.1.0-x64-Setup.exe') "/S /D=$install" 4
        Assert ((Get-ItemProperty $registry).DisplayVersion -eq '1.0.0') 'Running app was replaced'
    } finally {
        if (-not $running.HasExited) { $running.Kill() }
        $running.WaitForExit()
        $running.Dispose()
    }

    # An open file that denies deletion must leave the previous payload intact.
    $reader = [IO.File]::Open($app, 'Open', 'Read', 'ReadWrite')
    try {
        Run-Exe (Join-Path $v2 'smoke-1.1.0-x64-Setup.exe') "/S /D=$install" 5
        Assert (Test-Path $app) 'Failed upgrade removed the previous payload'
        Assert ((Get-ItemProperty $registry).DisplayVersion -eq '1.0.0') 'Failed upgrade changed installed version'
    } finally { $reader.Dispose() }

    # Exercise the runtime-generated setup update script, including spaces/quotes.
    $stage = Join-Path $Root 'staged-nsis'
    New-Item -ItemType Directory $stage | Out-Null
    $setup = Join-Path $stage 'setup.exe'
    Copy-Item (Join-Path $v2 'smoke-1.1.0-x64-Setup.exe') $setup
    $helper = Join-Path $Root 'apply-nsis.ps1'
    & (Join-Path $Root 'helper.exe') nsis 2147483647 $setup $install $app $stage $helper
    Assert ($LASTEXITCODE -eq 0) 'NSIS update helper failed'
    Start-Sleep -Milliseconds 500
    Assert ((Read-Report) -contains 'VERSION=1.1.0') 'NSIS update did not relaunch new app'
    Assert ((Get-ItemProperty $registry).DisplayVersion -eq '1.1.0') 'Upgrade did not update uninstall version'
    Assert (-not (Test-Path $stage)) 'Update staging was not cleaned'

    Copy-Item (Join-Path $v1 'smoke-1.0.0-x64-Portable.exe') $portable
    Run-Exe $portable 'hello "two words"'
    Assert ((Read-Report) -contains 'VERSION=1.0.0') 'Portable app did not run'
    Assert ((Read-Report) -contains 'ARGS=hello|two words') 'Portable did not forward arguments'
    Assert ((Read-Report) -contains "LAUNCHER=$portable") 'Portable origin missing'
    Assert ((Read-Report) -contains "STATE=$state") 'Portable changed stable user data root'
    $cwd = ((Read-Report) | Where-Object { $_.StartsWith('CWD=') }).Substring(4)
    Assert (-not (Test-Path $cwd)) 'Portable extraction was not cleaned'
    $stage = Join-Path $Root 'staged-portable'
    New-Item -ItemType Directory $stage | Out-Null
    $source = Join-Path $stage 'portable.exe'
    Copy-Item (Join-Path $v2 'smoke-1.1.0-x64-Portable.exe') $source
    $helper = Join-Path $Root 'apply-portable.ps1'
    & (Join-Path $Root 'helper.exe') portable 2147483647 $source $portable 2147483646 $stage $helper
    Assert ($LASTEXITCODE -eq 0) 'Portable update helper failed'
    Start-Sleep -Seconds 2
    Assert ((Read-Report) -contains 'VERSION=1.1.0') 'Portable update did not run the new launcher'
    Assert ((Get-FileHash $portable).Hash -eq (Get-FileHash (Join-Path $v2 'smoke-1.1.0-x64-Portable.exe')).Hash) 'Outer portable EXE was not replaced'

    # Copy uninstaller out first so the parent process can wait for completion.
    $uninstaller = Join-Path $Root 'uninstall-test.exe'
    Copy-Item (Join-Path $install 'Uninstall.exe') $uninstaller
    Run-Exe $uninstaller "/S _?=$install"
    Assert (-not (Test-Path $app)) 'Uninstall left the application installed'
    Assert (-not (Test-Path $registry)) 'Uninstall registration was not removed'
    Assert (-not (Test-Path $shortcut)) 'Shortcut was not removed'
    Assert ((Get-Content (Join-Path $state 'keep.txt')) -eq 'keep me') 'User data was removed'
    Write-Host "PASS: package, install, launch, NSIS update, portable update, cleanup, uninstall and data retention ($Root)"
} finally {
    Pop-Location
    # A failed assertion must not leave a registered test installation behind.
    $installedUninstaller = Join-Path $install 'Uninstall.exe'
    $owner = Join-Path $install '.lingxia-install-id'
    if ((Test-Path $installedUninstaller) -and (Test-Path $owner) -and
        ((Get-Content -LiteralPath $owner -Raw).Trim() -eq $identity)) {
        try {
            $cleanupExe = Join-Path $Root 'uninstall-cleanup.exe'
            Copy-Item -LiteralPath $installedUninstaller -Destination $cleanupExe -Force
            Run-Exe $cleanupExe "/S _?=$install"
        } catch { Write-Warning "Test installation cleanup failed: $_" }
    }
    foreach ($key in $savedEnvironment.Keys) {
        [Environment]::SetEnvironmentVariable($key, $savedEnvironment[$key], 'Process')
    }
}
