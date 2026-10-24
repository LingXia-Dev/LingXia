param(
    [Parameter(Mandatory=$true)][string]$ApkPath,
    [Parameter(Mandatory=$true)][ValidatePattern('^[A-Za-z0-9._:-]+$')][string]$Serial,
    [string]$AndroidSdk = $(if ($env:ANDROID_SDK_ROOT) { $env:ANDROID_SDK_ROOT } else { $env:ANDROID_HOME }),
    [string]$OutputDirectory
)
$ErrorActionPreference='Stop'
if (-not $AndroidSdk) { throw 'Set ANDROID_HOME/ANDROID_SDK_ROOT or pass -AndroidSdk.' }
$repo=(Resolve-Path "$PSScriptRoot/../../../..").Path
$manifest=Get-Content -LiteralPath "$PSScriptRoot/core-suite.json" -Raw | ConvertFrom-Json
if (-not $OutputDirectory) { $OutputDirectory=Join-Path $repo ('target/gpu-core-'+(Get-Date -Format 'yyyyMMdd-HHmmss')) }
$out=(New-Item -ItemType Directory -Force -Path $OutputDirectory).FullName
$adb=Join-Path $AndroidSdk 'platform-tools/adb.exe'
function Adb([string[]]$Arguments) {
    # PowerShell 5 treats adb's successful progress output on stderr as an error.
    $ErrorActionPreference='Continue'
    $result=& $adb -s $Serial @Arguments
    if($LASTEXITCODE -ne 0){throw "adb failed: $Arguments"}
    return $result
}
$apk=(Resolve-Path -LiteralPath $ApkPath).Path
$expected=(Get-FileHash -LiteralPath $apk -Algorithm SHA256).Hash.ToLowerInvariant()
$remote=@(Adb @('shell','pm','path',$manifest.package))[0].Trim().Substring(8)
$installed=(@(Adb @('shell','sha256sum',$remote))[0] -split '\s+')[0]
if($installed -ne $expected){throw 'Installed APK differs from the supplied artifact; install that APK before testing.'}
& "$PSScriptRoot/run-probe.ps1" -CompileOnly -Probe GpuHomeParityProbe -Serial $Serial -AndroidSdk $AndroidSdk
Adb @('push',"$repo/target/gpu-independent-probe.jar",'/data/local/tmp/gpu-independent-probe.jar') | Out-Null
$evidence=[ordered]@{
    startedAt=(Get-Date).ToUniversalTime().ToString('o'); apkSha256=$expected; serial=$Serial;
    model=(Adb @('shell','getprop','ro.product.model')); os=(Adb @('shell','getprop','ro.build.version.release'));
    viewport=(Adb @('shell','wm','size')); fontScale=(Adb @('shell','settings','get','system','font_scale'));
    package=(Adb @('shell','dumpsys','package',$manifest.package) | Select-String 'versionCode=|versionName=' | ForEach-Object { $_.Line.Trim() });
    sourceHead=(& git -C $repo rev-parse HEAD); dirtyPaths=@(& git -C $repo status --porcelain);
    scenarios=@(); status='running'
}
try {
    foreach($scenario in $manifest.scenarios) {
        if($scenario.probe -notmatch '^Gpu[A-Za-z]+Probe$'){throw 'Invalid probe name'}
        Adb @('shell','am','force-stop',$manifest.package) | Out-Null
        Adb @('shell','monkey','-p',$manifest.package,'1') | Out-Null
        $log=Join-Path $out ($scenario.probe+'.log'); $err=Join-Path $out ($scenario.probe+'.stderr.log')
        $process=Start-Process -FilePath $adb -ArgumentList @('-s',$Serial,'shell','CLASSPATH=/data/local/tmp/gpu-independent-probe.jar','app_process','/system/bin',$scenario.probe) -WindowStyle Hidden -PassThru -RedirectStandardOutput $log -RedirectStandardError $err
        # Retain the process handle so Windows PowerShell can read ExitCode after exit.
        $processHandle=$process.Handle
        $deadline=(Get-Date).AddSeconds($scenario.timeoutSeconds)
        while(-not $process.HasExited -and (Get-Date) -lt $deadline){Start-Sleep -Milliseconds 250;$process.Refresh()}
        $passed=$false
        if(-not $process.HasExited){$process.Kill()}else{
            $process.WaitForExit()
            $passed=$process.ExitCode -eq 0 -and [bool](Select-String -LiteralPath $log -Pattern '^PASS ')
        }
        $evidence.scenarios+=@{probe=$scenario.probe;status=$(if($passed){'pass'}else{'fail'});exitCode=$process.ExitCode;log=$log}
        $evidence | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath "$out/results.json" -Encoding UTF8
        if(-not $passed){throw "Probe failed or exceeded deadline: $($scenario.probe)"}
        Write-Output "PASS $($scenario.probe)"
    }
    $evidence.status='pass'
} catch {
    $evidence.status='fail';$evidence.error=$_.Exception.Message;throw
} finally {
    $evidence.finishedAt=(Get-Date).ToUniversalTime().ToString('o')
    $evidence | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath "$out/results.json" -Encoding UTF8
}
