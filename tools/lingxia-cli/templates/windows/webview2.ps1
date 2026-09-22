# Embedded by NSIS; run only when the WebView2 runtime is missing.
$ErrorActionPreference = 'Stop'
$target = Join-Path $PSScriptRoot 'MicrosoftEdgeWebview2Setup.exe'
try {
    Invoke-WebRequest -UseBasicParsing -Uri 'https://go.microsoft.com/fwlink/p/?LinkId=2124703' -OutFile $target
    $signature = Get-AuthenticodeSignature -LiteralPath $target
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch '(?:^|,\s*)O=Microsoft Corporation(?:,|$)') {
        throw 'WebView2 bootstrapper does not have a valid Microsoft signature.'
    }
    $process = Start-Process -FilePath $target -ArgumentList '/silent','/install' -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "WebView2 installation failed: $($process.ExitCode)" }
    exit 0
} catch {
    Write-Error $_ -ErrorAction Continue
    exit 1
} finally {
    Remove-Item -LiteralPath $target -Force -ErrorAction SilentlyContinue
}
