[CmdletBinding()]
param(
  [ValidateSet('react', 'vue', 'all')]
  [string]$Framework = 'all',
  [int]$TimeoutSeconds = 300,
  [int]$DevReadyTimeoutSeconds = 1800,
  [ValidateRange(1, 64)]
  [int]$BuildJobs = 2
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$showcaseRoot = Join-Path $repoRoot 'examples\lingxia-showcase'
$lxappRoot = Join-Path $showcaseRoot 'lxapp'
$userProfile = [Environment]::GetFolderPath([Environment+SpecialFolder]::UserProfile)
$installRoot = Join-Path $userProfile '.local\bin'
$env:CARGO_BUILD_JOBS = $BuildJobs.ToString()

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

function Copy-ExecutableWithRetry {
  param(
    [string]$Source,
    [string]$Destination
  )

  $deadline = [DateTime]::UtcNow.AddSeconds(5)
  do {
    try {
      Copy-Item -LiteralPath $Source -Destination $Destination -Force
      return
    } catch [System.IO.IOException] {
      if ([DateTime]::UtcNow -ge $deadline) { throw }
      Start-Sleep -Milliseconds 100
    }
  } while ($true)
}

function Stop-IdleAutomationBrokers {
  Get-CimInstance Win32_Process |
    Where-Object {
      $_.Name -eq 'lingxia.exe' -and $_.CommandLine -match '\bdev-broker\b'
    } |
    ForEach-Object {
      Write-Host "Stopping idle automation broker $($_.ProcessId) ($($_.ExecutablePath))"
      Stop-Process -Id $_.ProcessId -Force
      Wait-Process -Id $_.ProcessId -Timeout 5 -ErrorAction SilentlyContinue
    }
}

function Install-AutomationTools {
  Write-Host 'Building automation CLIs from the current checkout...'
  Invoke-Checked 'cargo' @('build', '-p', 'lingxia-cli', '-p', 'lingxia-devtools-cli')
  New-Item -ItemType Directory -Force -Path $installRoot | Out-Null

  $builtLingXia = Join-Path $repoRoot 'target\debug\lingxia.exe'
  $builtLxdev = Join-Path $repoRoot 'target\debug\lxdev.exe'
  if (-not (Test-Path -LiteralPath $builtLingXia) -or -not (Test-Path -LiteralPath $builtLxdev)) {
    throw 'Cargo completed without producing both automation CLI executables.'
  }

  # Start the singleton outside the captured command pipeline. If lxdev has to
  # spawn it while stdout is captured by Out-String, the detached broker keeps
  # that pipe open and PowerShell waits forever for EOF.
  Start-Process -FilePath $builtLingXia -ArgumentList 'dev-broker' -WindowStyle Hidden
  Start-Sleep -Milliseconds 500
  $liveSessionsJson = (& $builtLxdev session list --json | Out-String).Trim()
  if ($LASTEXITCODE -ne 0) { throw 'Could not inspect live dev sessions before installing CLIs.' }
  $liveSessions = $liveSessionsJson | ConvertFrom-Json
  if (@($liveSessions).Count -gt 0) {
    throw 'Live LingXia dev sessions exist. Stop them before replacing the locally installed CLIs.'
  }

  $installedLingXia = Join-Path $installRoot 'lingxia.exe'
  # Querying through the freshly built lxdev may start a broker next to the
  # build output. With no registered sessions, restart either known broker so
  # the rest of the run uses only the installed CLI generation.
  Stop-IdleAutomationBrokers

  Write-Host "Installing automation CLIs to $installRoot..."
  Copy-ExecutableWithRetry $builtLingXia (Join-Path $installRoot 'lingxia.exe')
  Copy-ExecutableWithRetry $builtLxdev (Join-Path $installRoot 'lxdev.exe')

  Start-Process -FilePath $installedLingXia -ArgumentList 'dev-broker' -WindowStyle Hidden
  Start-Sleep -Milliseconds 500
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

function Test-BenignWindowsSessionError {
  param([string]$Message)
  # setData already landed in this.data. These WebView races recover on the
  # next snapshot and have been failing otherwise-green Windows CI.
  if ($Message -match 'Error in setData:.*(WebView not ready|WebView UI thread did not reply|timed out waiting on channel)') {
    return $true
  }
  # WebView2 status 9 is CONNECTION_ABORTED, which is also how it reports
  # Chromium's ERR_ABORTED: navigating away cancels the outgoing page's own
  # load, and the failure lands after its onUnload. Only status 14 short
  # circuits to Cancelled in windows_load_error_kind, so this arrives as an
  # error-level log. A load that genuinely fails still fails the test that
  # navigated there.
  $Message -match 'page load failed: WebView2 web error status 9 \(Network\)'
}

function Get-UnexpectedWindowsSessionErrors {
  param([string]$ErrorLogs)
  if ([string]::IsNullOrWhiteSpace($ErrorLogs)) { return $null }

  $entries = @()
  $trimmed = $ErrorLogs.Trim()
  if ($trimmed.StartsWith('[')) {
    $entries = @($trimmed | ConvertFrom-Json)
  } else {
    foreach ($line in ($ErrorLogs -split '\r?\n')) {
      if ([string]::IsNullOrWhiteSpace($line)) { continue }
      $entries += ($line | ConvertFrom-Json)
    }
  }

  $unexpected = @(
    $entries | Where-Object { -not (Test-BenignWindowsSessionError ([string]$_.data.message)) }
  )
  if ($unexpected.Count -eq 0) { return $null }
  ($unexpected | ConvertTo-Json -Compress -Depth 8)
}

function Wait-ShowcaseSessionReady {
  $readyDeadline = [DateTime]::UtcNow.AddSeconds($DevReadyTimeoutSeconds)
  do {
    Start-Sleep -Seconds 5
    $statusJson = (& $lingxia dev status --json | Out-String).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Could not inspect LingXia dev session readiness.' }
    $ready = -not [string]::IsNullOrWhiteSpace($statusJson) `
      -and @($statusJson | ConvertFrom-Json | Where-Object { $_.runtime_connected }).Count -gt 0
  } until ($ready -or [DateTime]::UtcNow -ge $readyDeadline)
  if (-not $ready) {
    throw "Windows dev session did not become ready within $DevReadyTimeoutSeconds seconds."
  }
  Invoke-Checked $lingxia @('dev', 'status', '--json')
}

function Start-ShowcaseSession {
  param(
    [string]$Framework,
    [switch]$SkipNative
  )
  $devArguments = @(
    'dev', '--background', '--platform', 'windows', '--framework', $Framework
  )
  if ($SkipNative) { $devArguments += '--skip-native' }
  Invoke-Checked $lingxia $devArguments
  Wait-ShowcaseSessionReady
}

function Stop-ShowcaseSession {
  Invoke-Checked $lingxia @('dev', 'stop', 'windows')
}

function Invoke-ShowcaseSuite {
  param(
    [string]$Framework,
    [string]$ResultDirectory
  )
  Write-Host "Running Windows Showcase automation ($Framework)..."
  & $lxdev test tests/entries/windows.test.ts `
    --timeout $($TimeoutSeconds.ToString()) `
    --arg 'platform=windows' `
    --arg "framework=$Framework" `
    --output-dir $ResultDirectory
  $LASTEXITCODE
}

function Assert-InteractiveDesktop {
  if (-not ('LingXiaAutomation.NativeDesktop' -as [type])) {
    Add-Type -Namespace LingXiaAutomation -Name NativeDesktop -MemberDefinition '
      [System.Runtime.InteropServices.DllImport("user32.dll")]
      public static extern System.IntPtr OpenInputDesktop(uint flags, bool inherit, uint access);
      [System.Runtime.InteropServices.DllImport("user32.dll")]
      public static extern bool CloseDesktop(System.IntPtr handle);
    '
  }
  # The suite injects OS-level pointer input and probes screen pixels; both
  # silently fail while the session is locked or an RDP session is disconnected.
  $desktop = [LingXiaAutomation.NativeDesktop]::OpenInputDesktop(0, $false, 0x0001)
  if ($desktop -eq [System.IntPtr]::Zero) {
    throw 'The interactive desktop is unavailable (session locked or RDP disconnected). Keep the session connected and unlocked, then rerun.'
  }
  [LingXiaAutomation.NativeDesktop]::CloseDesktop($desktop) | Out-Null
}

Assert-InteractiveDesktop
Install-AutomationTools
$lingxia = Join-Path $installRoot 'lingxia.exe'
$lxdev = Join-Path $installRoot 'lxdev.exe'
Write-Host "Installed current automation CLIs to $installRoot"

Invoke-Checked $lingxia @('doctor', '--platform', 'windows')

Push-Location $showcaseRoot
try {
  $sessionsJson = (& $lingxia dev status --json | Out-String).Trim()
  if ($LASTEXITCODE -ne 0) { throw 'Could not inspect existing LingXia dev sessions.' }
  $sessions = $sessionsJson | ConvertFrom-Json
  if ($sessions.Where({ $_.target -eq 'windows' }).Count -gt 0) {
    throw 'A Windows dev session already exists for this project. Stop it explicitly before running automation.'
  }

  $frameworks = if ($Framework -eq 'all') { @('react', 'vue') } else { @($Framework) }
  $frameworks = @($frameworks)
  for ($frameworkIndex = 0; $frameworkIndex -lt $frameworks.Count; $frameworkIndex += 1) {
    $currentFramework = $frameworks[$frameworkIndex]
    Write-Host "Starting Windows Showcase ($currentFramework)..."
    $started = $false
    try {
      # React and Vue exercise the same native host. Keep the second pass in
      # this job and only rebuild its lxapp assets.
      Start-ShowcaseSession -Framework $currentFramework -SkipNative:($frameworkIndex -gt 0)
      $started = $true

      Push-Location $lxappRoot
      try {
        $resultDirectory = "test-results/automation/windows-$currentFramework"
        $testExitCode = Invoke-ShowcaseSuite -Framework $currentFramework -ResultDirectory $resultDirectory
        if ($testExitCode -ne 0) {
          # A wedged WebView UI thread surfaces as `request timed out` and
          # kills the suite. Recycle the session once before failing the job.
          Write-Host "Retrying Windows Showcase automation ($currentFramework) after recycling the session..."
          Set-Location $showcaseRoot
          Write-Host "Stopping Windows Showcase ($currentFramework)..."
          Stop-ShowcaseSession
          $started = $false
          Start-ShowcaseSession -Framework $currentFramework -SkipNative
          $started = $true
          Set-Location $lxappRoot
          $testExitCode = Invoke-ShowcaseSuite -Framework $currentFramework -ResultDirectory $resultDirectory
        }

        New-Item -ItemType Directory -Force -Path $resultDirectory | Out-Null
        if ($testExitCode -eq 0) {
          Write-Host "Running same-route relaunch stress ($currentFramework)..."
          Invoke-SameRouteRelaunchStress
        }
        Write-Host "Collecting Windows session logs ($currentFramework)..."
        & $lxdev logs --json --limit 5000 |
          Set-Content -LiteralPath (Join-Path $resultDirectory 'session.jsonl') -Encoding utf8
        if ($LASTEXITCODE -ne 0) { throw 'Failed to collect Windows session logs.' }
        $errorLogs = (& $lxdev logs --level error --json --limit 1000 | Out-String).Trim()
        if ($LASTEXITCODE -ne 0) { throw 'Failed to inspect Windows error logs.' }
        $unexpected = Get-UnexpectedWindowsSessionErrors $errorLogs
        if ($unexpected) {
          throw "Unexpected error-level Windows session logs:`n$unexpected"
        }
        if ($testExitCode -ne 0) {
          throw "Windows $currentFramework automation exited with code $testExitCode"
        }
      } finally {
        Pop-Location
      }
    } finally {
      if ($started) {
        Write-Host "Stopping Windows Showcase ($currentFramework)..."
        Stop-ShowcaseSession
        $started = $false
      }
    }
  }
} finally {
  Pop-Location
}

Write-Host 'Windows Showcase automation passed using locally installed CLIs.'
