param([string]$Sdk = $env:ANDROID_HOME, [string]$Device = '', [string]$Project = '')
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path "$PSScriptRoot/../..").Path
if (-not $Project) { $Project = "$repo/examples/hello-ui" }
$Project = (Resolve-Path -LiteralPath $Project).Path
if (-not $Sdk) { throw 'Set ANDROID_HOME or pass -Sdk.' }
$ndk = Get-ChildItem "$Sdk/ndk" -Directory | Sort-Object Name -Descending | Select-Object -First 1
if (-not $ndk) { throw 'Install an Android NDK in the SDK.' }
$llvm = Join-Path $ndk.FullName 'toolchains/llvm/prebuilt/windows-x86_64/bin'
$env:ANDROID_HOME = $Sdk
$env:ANDROID_NDK_HOME = $ndk.FullName
$env:ANDROID_NDK_ROOT = $ndk.FullName
$env:ANDROID_API_LEVEL = '23'
$env:CC_aarch64_linux_android = "$llvm/aarch64-linux-android23-clang.cmd"
$env:AR_aarch64_linux_android = "$llvm/llvm-ar.exe"
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = "$llvm/aarch64-linux-android23-clang.cmd"
$resource = (& "$llvm/clang.exe" -print-resource-dir).Trim().Replace('\', '/')
$sysroot = (Join-Path $ndk.FullName 'toolchains/llvm/prebuilt/windows-x86_64/sysroot/usr/include').Replace('\', '/')
# Rong sets the generic bindgen args; the target-qualified args retain NDK builtins.
$env:BINDGEN_EXTRA_CLANG_ARGS_aarch64_linux_android = "-I`"$resource/include`" -I`"$sysroot`" -I`"$sysroot/aarch64-linux-android`""
function Checked([string]$Exe, [string[]]$Arguments) {
    & $Exe @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Exe failed ($LASTEXITCODE)" }
}
Push-Location $repo
try {
    Checked cargo @('run', '-p', 'lingxia-ui', '--example', 'generate')
    Push-Location $Project
    try { Checked "$repo/target/debug/lingxia.exe" @('build') }
    finally { Pop-Location }
    Checked cargo @('build', '-p', 'lingxia-ui', '--features', 'gpu', '--target', 'aarch64-linux-android', '--release')
    $native = "$PSScriptRoot/src/main/jniLibs/arm64-v8a"
    New-Item -ItemType Directory -Force $native | Out-Null
    Copy-Item -LiteralPath "$repo/target/aarch64-linux-android/release/liblingxia_ui.so" -Destination $native
    Checked "$repo/lingxia-sdk/android/gradlew.bat" @('-p', $PSScriptRoot, "-PuiAssets=$Project/dist", 'assembleDebug', '--console=plain')
    $adb = "$Sdk/platform-tools/adb.exe"
    $selector = @()
    if ($Device) { $selector = @('-s', $Device) }
    Checked $adb ($selector + @('install', '-r', "$PSScriptRoot/build/outputs/apk/debug/LingXiaUiSmoke-debug.apk"))
    Checked $adb ($selector + @('shell', 'am', 'start', '-n', 'com.lingxia.ui.smoke/com.lingxia.ui.SmokeActivity'))
} finally { Pop-Location }
