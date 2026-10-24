param([Parameter(Mandatory=$true)][string]$Serial, [string]$AndroidSdk = $(if ($env:ANDROID_SDK_ROOT) { $env:ANDROID_SDK_ROOT } else { $env:ANDROID_HOME }))
$ErrorActionPreference='Stop'
$repoRoot=(Resolve-Path (Join-Path $PSScriptRoot '../../../..')).Path
$source=Join-Path $PSScriptRoot 'receiver'
$output=Join-Path $repoRoot 'target/gpu-share-receiver'
$sdk=$AndroidSdk
if (-not $sdk) { throw 'Set ANDROID_HOME/ANDROID_SDK_ROOT or pass -AndroidSdk.' }
$buildTools=Join-Path $sdk 'build-tools/35.0.0'
$androidJar=Join-Path $sdk 'platforms/android-35/android.jar'
New-Item -ItemType Directory -Force "$output/classes","$output/dex" | Out-Null
$ErrorActionPreference='Continue'
& javac -classpath $androidJar -d "$output/classes" "$source/Receiver.java"
if($LASTEXITCODE -ne 0){throw 'Receiver javac failed'}
& "$buildTools/d8.bat" --lib $androidJar --output "$output/dex" "$output/classes/com/lingxia/test/receiver/Receiver.class"
if($LASTEXITCODE -ne 0){throw 'Receiver dex failed'}
& "$buildTools/aapt2.exe" link -I $androidJar --manifest "$source/AndroidManifest.xml" -o "$output/unsigned.apk"
if($LASTEXITCODE -ne 0){throw 'Receiver package failed'}
& jar uf "$output/unsigned.apk" -C "$output/dex" classes.dex
if($LASTEXITCODE -ne 0){throw 'Receiver archive failed'}
& "$buildTools/zipalign.exe" -f 4 "$output/unsigned.apk" "$output/aligned.apk"
if($LASTEXITCODE -ne 0){throw 'Receiver alignment failed'}
& "$buildTools/apksigner.bat" sign --ks "$env:USERPROFILE/.android/debug.keystore" --ks-pass pass:android --key-pass pass:android --out "$output/receiver.apk" "$output/aligned.apk"
if($LASTEXITCODE -ne 0){throw 'Receiver signing failed'}
& "$sdk/platform-tools/adb.exe" -s $Serial install -r "$output/receiver.apk"
if($LASTEXITCODE -ne 0){throw 'Receiver install failed'}
