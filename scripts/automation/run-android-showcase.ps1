[CmdletBinding()]
param(
  [ValidateSet('react', 'vue', 'all')]
  [string]$Framework = 'all',
  [string]$Device,
  [int]$TimeoutSeconds = 600
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$showcaseRoot = Join-Path $repoRoot 'examples\lingxia-showcase'
$lxappRoot = Join-Path $showcaseRoot 'lxapp'

function Resolve-LingXiaTool {
  param([string]$Name)

  $local = Join-Path $repoRoot "target\debug\$Name.exe"
  if (Test-Path -LiteralPath $local) { return $local }
  $command = Get-Command $Name -ErrorAction SilentlyContinue
  if ($null -eq $command) {
    throw "Missing $Name. Run cargo build -p lingxia-cli -p lingxia-devtools-cli."
  }
  return $command.Source
}

function Invoke-Checked {
  param(
    [string]$FilePath,
    [string[]]$Arguments
  )

  & $FilePath @Arguments
  if ($LASTEXITCODE -ne 0) {
    throw "$FilePath exited with code $LASTEXITCODE"
  }
}

function Get-AndroidUiHierarchy {
  param([string]$Destination)

  & $adb @adbTarget shell uiautomator dump /sdcard/lingxia-automation-window.xml | Out-Null
  if ($LASTEXITCODE -ne 0) { throw 'Android UIAutomator could not dump the window hierarchy.' }
  $xmlText = (& $adb @adbTarget exec-out cat /sdcard/lingxia-automation-window.xml | Out-String).Trim()
  if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($xmlText)) {
    throw 'Android UIAutomator hierarchy was empty.'
  }
  Set-Content -LiteralPath $Destination -Value $xmlText -Encoding utf8
  return [xml]$xmlText
}

function Invoke-NativeVideoLifecycleProbe {
  param([string]$ResultDirectory)

  Invoke-Checked $lxdev @('lxapp', 'nav', 'relaunch', 'components', '--json')
  Invoke-Checked $lxdev @(
    'lxapp', 'nav', 'to', 'video',
    '--query', 'automationFixture=video-context-shape',
    '--json'
  )
  Invoke-Checked $lxdev @(
    'lxapp', 'page', 'wait', '--page', 'video',
    '--css', '#lx-video-shape-fixture', '--state', 'visible', '--timeout-ms', '10000'
  )

  $description = 'LingXia native component video.native lx-video-shape-fixture'
  $beforePath = Join-Path $ResultDirectory 'native-video-before-back.xml'
  $before = Get-AndroidUiHierarchy $beforePath
  if ($before.SelectNodes("//node[@content-desc='$description']").Count -lt 1) {
    throw "Android native video was not visible in the OS view hierarchy: $description"
  }

  & $adb @adbTarget shell input keyevent 4
  if ($LASTEXITCODE -ne 0) { throw 'Could not send a physical Android Back key event.' }
  Invoke-Checked $lxdev @(
    'lxapp', 'page', 'wait', '--page', 'components',
    '--css', '[data-testid="components-page"]', '--state', 'visible', '--timeout-ms', '5000'
  )

  $deadline = [DateTime]::UtcNow.AddSeconds(5)
  $afterPath = Join-Path $ResultDirectory 'native-video-after-back.xml'
  do {
    $after = Get-AndroidUiHierarchy $afterPath
    if ($after.SelectNodes("//node[@content-desc='$description']").Count -eq 0) {
      return
    }
    Start-Sleep -Milliseconds 100
  } while ([DateTime]::UtcNow -lt $deadline)

  throw 'Android native video remained visible in the OS view hierarchy for more than 5 seconds after Back.'
}

function Invoke-SameRouteRelaunchStress {
  for ($iteration = 1; $iteration -le 10; $iteration += 1) {
    Invoke-Checked $lxdev @('lxapp', 'nav', 'relaunch', 'home', '--json')
    Invoke-Checked $lxdev @(
      'lxapp', 'page', 'wait', '--page', 'home',
      '--css', '[data-testid="home-page"]', '--state', 'visible', '--timeout-ms', '10000'
    )
  }
}

$lingxia = Resolve-LingXiaTool 'lingxia'
$lxdev = Resolve-LingXiaTool 'lxdev'
$adb = (Get-Command adb -ErrorAction SilentlyContinue).Source
if ([string]::IsNullOrWhiteSpace($adb)) {
  throw 'adb is not on PATH. Set ANDROID_SDK_ROOT and add platform-tools to PATH.'
}

Invoke-Checked $lingxia @('doctor', '--platform', 'android')
Invoke-Checked $lingxia @('devices', '--platform', 'android')

$adbTarget = @()
if (-not [string]::IsNullOrWhiteSpace($Device)) { $adbTarget = @('-s', $Device) }
$abiList = (& $adb @adbTarget shell getprop ro.product.cpu.abilist).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Could not query the selected Android device ABI list.' }
if ([string]::IsNullOrWhiteSpace($abiList)) {
  $abiList = (& $adb @adbTarget shell getprop ro.product.cpu.abi).Trim()
  if ($LASTEXITCODE -ne 0) { throw 'Could not query the selected Android device ABI.' }
}
$abis = @($abiList.Split(',') | ForEach-Object { $_.Trim() } | Where-Object { $_ })
if (-not ($abis -contains 'arm64-v8a') -and -not ($abis -contains 'armeabi-v7a')) {
  throw "Unsupported Android device ABIs '$abiList'; use an ARM64 or ARMv7 device/emulator."
}

Push-Location $showcaseRoot
try {
  $sessionsJson = (& $lingxia dev status --json | Out-String).Trim()
  if ($LASTEXITCODE -ne 0) { throw 'Could not inspect existing LingXia dev sessions.' }
  $sessions = @($sessionsJson | ConvertFrom-Json)
  if ($sessions.Where({ $_.target -eq 'android' }).Count -gt 0) {
    throw 'An Android dev session already exists for this project. Stop it explicitly before running automation.'
  }

  $frameworks = if ($Framework -eq 'all') { @('react', 'vue') } else { @($Framework) }
  foreach ($currentFramework in $frameworks) {
    $started = $false
    try {
      $devArguments = @(
        'dev',
        '--background',
        '--platform',
        'android',
        '--framework',
        $currentFramework
      )
      if (-not [string]::IsNullOrWhiteSpace($Device)) { $devArguments += @('--device', $Device) }
      Invoke-Checked $lingxia $devArguments
      $started = $true
      Invoke-Checked $lingxia @('dev', 'status', '--json')

      Push-Location $lxappRoot
      try {
        $resultDirectory = "test-results/automation/android-$currentFramework"
        $entry = 'tests/entries/android.test.ts'
        & $lxdev test $entry `
          --timeout $($TimeoutSeconds.ToString()) `
          --arg 'platform=android' `
          --arg "framework=$currentFramework" `
          --output-dir $resultDirectory
        $testExitCode = $LASTEXITCODE

        New-Item -ItemType Directory -Force -Path $resultDirectory | Out-Null
        if ($testExitCode -eq 0) {
          Invoke-SameRouteRelaunchStress
          Invoke-NativeVideoLifecycleProbe $resultDirectory
        }
        & $lxdev logs --json --limit 5000 |
          Set-Content -LiteralPath (Join-Path $resultDirectory 'session.jsonl') -Encoding utf8
        if ($LASTEXITCODE -ne 0) { throw 'Failed to collect Android session logs.' }
        $errorLogs = (& $lxdev logs --level error --json --limit 1000 | Out-String).Trim()
        if ($LASTEXITCODE -ne 0) { throw 'Failed to inspect Android error logs.' }
        if (-not [string]::IsNullOrWhiteSpace($errorLogs)) {
          throw "Unexpected error-level Android session logs:`n$errorLogs"
        }
        if ($testExitCode -ne 0) {
          throw "Android $currentFramework automation exited with code $testExitCode"
        }
      } finally {
        Pop-Location
      }
    } finally {
      if ($started) {
        Invoke-Checked $lingxia @('dev', 'stop', 'android')
        $started = $false
      }
    }
  }
} finally {
  Pop-Location
}

Write-Host 'Portable Android automation and the OS-level native video lifecycle probe passed.'
