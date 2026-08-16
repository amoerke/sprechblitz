#!/usr/bin/env bash
# SprechBlitz – Release-Build in einem Schritt (macOS-Pendant zu build-release.ps1).
#
#   ./build-release.sh            → Release + .app/.dmg + signierte Updater-Artefakte
#   ./build-release.sh --debug    → schneller Debug-Build ohne Bundle/Signierung
#
# Das Passwort des Signaturschlüssels wird abgefragt (oder vorab via
# TAURI_SIGNING_PRIVATE_KEY_PASSWORD setzen).

set -euo pipefail
cd "$(dirname "$0")"

# Rust in den PATH holen (frische Terminals kennen ihn ggf. noch nicht)
if ! command -v cargo >/dev/null 2>&1; then
    export PATH="$HOME/.cargo/bin:$PATH"
fi
echo "• $(cargo --version)"

if [ ! -d node_modules ]; then
    echo "• npm install …"
    npm install
fi

if [ "${1:-}" = "--debug" ]; then
    npx tauri build --debug --no-bundle
    echo ""
    echo "Fertig: src-tauri/target/debug/sprechblitz"
    exit 0
fi

# Updater-Signierung
key_path="../updater-signing.key"
if [ ! -f "$key_path" ]; then
    echo "Signaturschlüssel nicht gefunden: $key_path" >&2
    exit 1
fi
TAURI_SIGNING_PRIVATE_KEY="$(cat "$key_path")"
export TAURI_SIGNING_PRIVATE_KEY
if [ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ]; then
    read -r -s -p "Passwort für updater-signing.key: " TAURI_SIGNING_PRIVATE_KEY_PASSWORD
    echo
    export TAURI_SIGNING_PRIVATE_KEY_PASSWORD
fi

npx tauri build

echo ""
echo "Fertig! Artefakte:"
for dir in src-tauri/target/release/bundle/macos src-tauri/target/release/bundle/dmg; do
    if [ -d "$dir" ]; then
        find "$dir" -maxdepth 1 \( -name "*.app" -o -name "*.dmg" -o -name "*.tar.gz" -o -name "*.sig" \) \
            | sed 's/^/  /'
    fi
done
