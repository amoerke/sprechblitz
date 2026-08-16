# SprechBlitz – kompletter Release-Ablauf in einem Schritt:
#   Version setzen → bauen → latest.json mergen → GitHub-Release anlegen/ergänzen
#
#   .\release.ps1 -Version 0.1.1
#   .\release.ps1 -Version 0.2.0 -Notes "Neuer PTT-Modus, Bugfixes"
#
# Existiert das Release v<Version> bereits (z. B. vom Mac aus angelegt), werden die
# Windows-Artefakte ergänzt und die latest.json um den windows-Eintrag erweitert –
# vorhandene Plattform-Einträge bleiben erhalten.
#
# Voraussetzungen: gh CLI eingeloggt; Schlüssel-Passwort wird vom Buildscript
# abgefragt (oder vorab $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD setzen).

param(
    [Parameter(Mandatory)][string]$Version,
    [string]$Notes = ""
)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    throw "Version bitte im Format x.y.z angeben (z. B. 0.1.1)"
}
if (-not $Notes) { $Notes = "SprechBlitz $Version" }

# --- Version in tauri.conf.json setzen (Formatierung bleibt erhalten) ---
$confPath = "src-tauri\tauri.conf.json"
$conf = Get-Content $confPath -Raw
$conf = $conf -replace '"version": "\d+\.\d+\.\d+"', "`"version`": `"$Version`""
$conf | Out-File $confPath -Encoding utf8 -NoNewline
Write-Host "• Version in tauri.conf.json: $Version"

# --- Bauen (Release + Installer + Signatur) ---
& "$PSScriptRoot\build-release.ps1"

$bundle = "src-tauri\target\release\bundle\nsis"
$setup = "SprechBlitz_${Version}_x64-setup.exe"
if (-not (Test-Path "$bundle\$setup.sig")) {
    throw "Signatur fehlt: $bundle\$setup.sig – Build fehlgeschlagen?"
}

# --- latest.json erzeugen bzw. mit vorhandenem Release mergen ---
$sig = (Get-Content "$bundle\$setup.sig" -Raw).Trim()
$manifest = [ordered]@{
    version   = $Version
    notes     = $Notes
    pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    platforms = [ordered]@{}
}

# EAP kurz lockern: stderr-Umleitung bei nativen Kommandos wirft sonst unter PS 5.1.
$prevEAP = $ErrorActionPreference
$ErrorActionPreference = "Continue"
gh release view "v$Version" *> $null
$releaseExists = ($LASTEXITCODE -eq 0)
if ($releaseExists) {
    $existing = Join-Path $env:TEMP "sprechblitz-latest-existing.json"
    Remove-Item $existing -ErrorAction SilentlyContinue
    gh release download "v$Version" --pattern latest.json --output $existing *> $null
    if (Test-Path $existing) {
        $old = Get-Content $existing -Raw | ConvertFrom-Json
        # Nur Einträge derselben Version übernehmen – alles andere würde dem
        # Updater einen alten Build unter neuer Versionsnummer unterschieben.
        if ($old.version -eq $Version -and $old.platforms) {
            foreach ($p in $old.platforms.PSObject.Properties) {
                $manifest.platforms[$p.Name] = [ordered]@{
                    signature = $p.Value.signature
                    url       = $p.Value.url
                }
            }
        }
        Remove-Item $existing -ErrorAction SilentlyContinue
    }
}
$ErrorActionPreference = $prevEAP

$manifest.platforms["windows-x86_64"] = [ordered]@{
    signature = $sig
    url       = "https://github.com/amoerke/sprechblitz/releases/download/v$Version/$setup"
}
$manifest | ConvertTo-Json -Depth 4 | Out-File "$bundle\latest.json" -Encoding ascii
Write-Host "• latest.json erzeugt"

# --- GitHub-Release anlegen oder ergänzen ---
if ($releaseExists) {
    gh release upload "v$Version" "$bundle\$setup" "$bundle\$setup.sig" "$bundle\latest.json" --clobber
    if ($LASTEXITCODE -ne 0) { throw "gh release upload fehlgeschlagen" }
    Write-Host "• Release v$Version um Windows-Artefakte ergänzt"
} else {
    gh release create "v$Version" "$bundle\$setup" "$bundle\$setup.sig" "$bundle\latest.json" `
        --title "SprechBlitz $Version" --notes $Notes
    if ($LASTEXITCODE -ne 0) { throw "gh release create fehlgeschlagen" }
}

# --- Versionsbump committen (Push bleibt manuell) ---
Set-Location "$PSScriptRoot\.."
git add client/src-tauri/tauri.conf.json
git diff --cached --quiet -- client/src-tauri/tauri.conf.json
if ($LASTEXITCODE -eq 0) {
    Write-Host "• Versionsbump war bereits committet"
} else {
    git commit -m "Release v$Version"
}

Write-Host ""
Write-Host "Release v$Version veröffentlicht: https://github.com/amoerke/sprechblitz/releases/tag/v$Version"
Write-Host "Versionsbump ist lokal committet - bei Gelegenheit 'git push' nicht vergessen."
