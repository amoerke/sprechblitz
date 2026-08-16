#!/usr/bin/env bash
# SprechBlitz – kompletter Release-Ablauf (macOS-Pendant zu release.ps1):
#   Version setzen → bauen → latest.json mergen → GitHub-Release anlegen/ergänzen
#
#   ./release.sh 0.1.1
#   ./release.sh 0.2.0 "Neuer PTT-Modus, Bugfixes"
#
# Existiert das Release v<Version> bereits (z. B. von Windows aus angelegt), werden
# die macOS-Artefakte ergänzt und die latest.json um den darwin-Eintrag erweitert –
# vorhandene Plattform-Einträge bleiben erhalten.
#
# Voraussetzungen: gh CLI eingeloggt; Schlüssel-Passwort wird vom Buildscript
# abgefragt (oder vorab TAURI_SIGNING_PRIVATE_KEY_PASSWORD setzen).

set -euo pipefail
cd "$(dirname "$0")"

VERSION="${1:-}"
NOTES="${2:-}"
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Version bitte im Format x.y.z angeben (z. B. 0.1.1)" >&2
    exit 1
fi
[ -n "$NOTES" ] || NOTES="SprechBlitz $VERSION"

case "$(uname -m)" in
    arm64)  ARCH="aarch64"; PLATFORM="darwin-aarch64" ;;
    x86_64) ARCH="x64";     PLATFORM="darwin-x86_64" ;;
    *) echo "Unbekannte Architektur: $(uname -m)" >&2; exit 1 ;;
esac

# --- Version in tauri.conf.json setzen (Formatierung bleibt erhalten) ---
sed -i '' -E "s/\"version\": \"[0-9]+\.[0-9]+\.[0-9]+\"/\"version\": \"$VERSION\"/" src-tauri/tauri.conf.json
echo "• Version in tauri.conf.json: $VERSION"

# --- Bauen (Release + .app/.dmg + Signatur) ---
./build-release.sh

bundle="src-tauri/target/release/bundle"
app_tar="$bundle/macos/SprechBlitz.app.tar.gz"
if [ ! -f "$app_tar.sig" ]; then
    echo "Signatur fehlt: $app_tar.sig – Build fehlgeschlagen?" >&2
    exit 1
fi

# Versionierte Asset-Namen, damit im Release eindeutig ist, was drinsteckt.
updater_asset="SprechBlitz_${VERSION}_${ARCH}.app.tar.gz"
cp "$app_tar" "$bundle/macos/$updater_asset"
cp "$app_tar.sig" "$bundle/macos/$updater_asset.sig"
dmg="$bundle/dmg/SprechBlitz_${VERSION}_${ARCH}.dmg"
if [ ! -f "$dmg" ]; then
    dmg="$(find "$bundle/dmg" -name "*.dmg" -print -quit)"
fi

# --- latest.json erzeugen bzw. mit vorhandenem Release mergen ---
release_exists=false
rm -f "$bundle/macos/latest-existing.json"
if gh release view "v$VERSION" >/dev/null 2>&1; then
    release_exists=true
    gh release download "v$VERSION" --pattern latest.json --output "$bundle/macos/latest-existing.json" 2>/dev/null || true
fi

SIG="$(cat "$app_tar.sig")" \
node --input-type=module - "$bundle/macos/latest-existing.json" "$bundle/macos/latest.json" \
    "$VERSION" "$NOTES" "$PLATFORM" \
    "https://github.com/amoerke/sprechblitz/releases/download/v$VERSION/$updater_asset" <<'EOF'
import fs from 'node:fs';
const [existingPath, outPath, version, notes, platform, url] = process.argv.slice(2);
const manifest = {
    version,
    notes,
    pub_date: new Date().toISOString().replace(/\.\d+Z$/, 'Z'),
    platforms: {},
};
if (fs.existsSync(existingPath)) {
    const old = JSON.parse(fs.readFileSync(existingPath, 'utf8'));
    // Only entries of the same version may survive – anything else would let the
    // updater install an old build under the new version number.
    if (old.version === version && old.platforms) manifest.platforms = old.platforms;
}
manifest.platforms[platform] = { signature: process.env.SIG.trim(), url };
fs.writeFileSync(outPath, JSON.stringify(manifest, null, 2) + '\n');
EOF
rm -f "$bundle/macos/latest-existing.json"
echo "• latest.json erzeugt (Plattform: $PLATFORM)"

# --- GitHub-Release anlegen oder ergänzen ---
assets=("$dmg" "$bundle/macos/$updater_asset" "$bundle/macos/$updater_asset.sig" "$bundle/macos/latest.json")
if $release_exists; then
    gh release upload "v$VERSION" "${assets[@]}" --clobber
    echo "• Release v$VERSION um macOS-Artefakte ergänzt"
else
    gh release create "v$VERSION" "${assets[@]}" --title "SprechBlitz $VERSION" --notes "$NOTES"
    echo "• Release v$VERSION angelegt"
fi

# --- Versionsbump committen (Push bleibt manuell) ---
cd ..
git add client/src-tauri/tauri.conf.json
if git diff --cached --quiet -- client/src-tauri/tauri.conf.json; then
    echo "• Versionsbump war bereits committet"
else
    git commit -m "Release v$VERSION"
    echo "• Versionsbump ist lokal committet - bei Gelegenheit 'git push' nicht vergessen."
fi

echo ""
echo "Release v$VERSION veröffentlicht: https://github.com/amoerke/sprechblitz/releases/tag/v$VERSION"
