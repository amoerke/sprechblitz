# SprechBlitz – Release-Build in einem Schritt.
#
#   .\build-release.ps1            → Release + NSIS-Installer + signierte Updater-Artefakte
#   .\build-release.ps1 -Debug     → schneller Debug-Build ohne Installer/Signierung
#
# Das Passwort des Signaturschlüssels wird abgefragt (oder vorab via
# $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD gesetzt).

param([switch]$Debug)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

# Rust in den PATH holen (frische Terminals kennen ihn ggf. noch nicht)
$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if ($env:Path -notlike "*$cargoBin*") {
    $env:Path = "$cargoBin;$env:Path"
}
$cargoVersion = & cargo --version
Write-Host "• $cargoVersion"

if (-not (Test-Path "node_modules")) {
    Write-Host "• npm install …"
    npm install
    if ($LASTEXITCODE -ne 0) { throw "npm install fehlgeschlagen" }
}

if ($Debug) {
    npx tauri build --debug --no-bundle
    if ($LASTEXITCODE -ne 0) { throw "Build fehlgeschlagen" }
    Write-Host ""
    Write-Host "Fertig: src-tauri\target\debug\sprechblitz.exe"
    exit 0
}

# Updater-Signierung
$keyPath = Join-Path $PSScriptRoot "..\updater-signing.key"
if (-not (Test-Path $keyPath)) {
    throw "Signaturschlüssel nicht gefunden: $keyPath"
}
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $keyPath -Raw
if (-not $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD) {
    $secure = Read-Host "Passwort für updater-signing.key" -AsSecureString
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [Runtime.InteropServices.Marshal]::PtrToStringAuto(
        [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure))
}

npx tauri build
if ($LASTEXITCODE -ne 0) { throw "Build fehlgeschlagen" }

Write-Host ""
Write-Host "Fertig! Artefakte:"
Get-ChildItem "src-tauri\target\release\bundle\nsis" | ForEach-Object { Write-Host "  $($_.FullName)" }
