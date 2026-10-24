param(
    [ValidatePattern('^Gpu[A-Za-z]+Probe$')]
    [string]$Probe = 'GpuStreamProbe',
    [string]$Serial = $env:ANDROID_SERIAL,
    [string]$AndroidSdk = $(if ($env:ANDROID_SDK_ROOT) { $env:ANDROID_SDK_ROOT } else { $env:ANDROID_HOME }),
    [switch]$CompileOnly
)
$ErrorActionPreference = 'Stop'
if (-not $AndroidSdk) { throw 'Set ANDROID_HOME/ANDROID_SDK_ROOT or pass -AndroidSdk.' }
if (-not $CompileOnly -and -not $Serial) { throw 'Pass -Serial or set ANDROID_SERIAL.' }
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '../../../..')).Path
$sourceRoot = Join-Path $PSScriptRoot 'probes'
if (-not (Test-Path -LiteralPath (Join-Path $sourceRoot "$Probe.java"))) {
    throw "Unknown probe: $Probe"
}
$classes = Join-Path $repoRoot 'target/gpu-independent-probe-classes'
$jar = Join-Path $repoRoot 'target/gpu-independent-probe.jar'
$androidJar = Join-Path $AndroidSdk 'platforms/android-35/android.jar'
$d8 = Join-Path $AndroidSdk 'build-tools/35.0.0/d8.bat'
$adb = Join-Path $AndroidSdk 'platform-tools/adb.exe'
New-Item -ItemType Directory -Force -Path $classes | Out-Null
$sources = @(Get-ChildItem -LiteralPath $sourceRoot -Filter '*.java' -File | ForEach-Object FullName)
# Windows PowerShell treats redirected native stderr as an error record.
# Compiler diagnostics are judged by exit status below.
$ErrorActionPreference = 'Continue'
& javac -encoding UTF-8 -classpath $androidJar -d $classes @sources
if ($LASTEXITCODE -ne 0) { throw 'Probe compilation failed' }
# d8.bat inherits cmd.exe's short command-line limit; pass one archive instead
# of an absolute path for every generated class (long checkout paths exceed it).
$inputJar = Join-Path $repoRoot 'target/gpu-independent-probe-input.jar'
& jar cf $inputJar -C $classes .
if ($LASTEXITCODE -ne 0) { throw 'Probe class archive failed' }
& $d8 --lib $androidJar --output $jar $inputJar
if ($LASTEXITCODE -ne 0) { throw 'Probe dex compilation failed' }
if ($CompileOnly) { Write-Output "Compiled $($sources.Count) Android test probes"; return }
& $adb -s $Serial push $jar /data/local/tmp/gpu-independent-probe.jar
if ($LASTEXITCODE -ne 0) { throw 'Probe upload failed' }
& $adb -s $Serial shell "CLASSPATH=/data/local/tmp/gpu-independent-probe.jar app_process /system/bin $Probe"
if ($LASTEXITCODE -ne 0) { throw "$Probe failed on device" }
