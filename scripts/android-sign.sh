#!/usr/bin/env bash
# Sign the APK built by `tauri android build` and put it in <out-dir>.
#
#   scripts/android-sign.sh <out-dir> [--release]
#
# The release key comes from two repository secrets (see CONTRIBUTING.md):
#   ANDROID_KEYSTORE           the keystore file, base64-encoded
#   ANDROID_KEYSTORE_PASSWORD  its password (alias "awan")
# Without them, test builds are signed with a throwaway test key kept in the
# Actions cache; --release refuses to, since phones only accept updates
# signed with the same key as the installed app.
set -euo pipefail

out=${1:?usage: android-sign.sh <out-dir> [--release]}
mode=${2:-}

unsigned=$(find src-tauri/gen/android/app/build/outputs/apk -name '*-unsigned.apk' | head -n 1)
[ -n "$unsigned" ] || { echo "::error title=No APK::Nothing to sign; did the Android build run?"; exit 1; }
tools=$(ls -d "$ANDROID_HOME"/build-tools/* | sort -V | tail -n 1)
version=$(node -p "require('./src-tauri/tauri.conf.json').version")
tmp=${RUNNER_TEMP:-$(mktemp -d)}

if [ -n "${ANDROID_KEYSTORE:-}" ]; then
  keystore="$tmp/awan-release.keystore"
  printf '%s' "$ANDROID_KEYSTORE" | base64 -d > "$keystore"
  export AWAN_KS_PASS="${ANDROID_KEYSTORE_PASSWORD:?ANDROID_KEYSTORE_PASSWORD is not set}"
  alias=awan
elif [ "$mode" = "--release" ]; then
  echo "::error title=No release signing key::Add the ANDROID_KEYSTORE and ANDROID_KEYSTORE_PASSWORD repository secrets (see CONTRIBUTING.md)."
  exit 1
else
  keystore="$HOME/.awan-test.keystore"
  export AWAN_KS_PASS=awan-test
  alias=awan-test
  if [ ! -f "$keystore" ]; then
    keytool -genkeypair -keystore "$keystore" -storepass "$AWAN_KS_PASS" -keypass "$AWAN_KS_PASS" \
      -alias "$alias" -keyalg RSA -keysize 2048 -validity 10000 -dname "CN=Awan test build"
  fi
  echo "::warning title=Test signing key::This APK is signed with a test key. Add the release key secrets so updates install over each other (see CONTRIBUTING.md)."
fi

mkdir -p "$out"
apk="$out/Awan_${version}_android-arm64.apk"
"$tools/apksigner" sign --ks "$keystore" --ks-pass env:AWAN_KS_PASS --key-pass env:AWAN_KS_PASS \
  --ks-key-alias "$alias" --out "$apk" "$unsigned"
"$tools/apksigner" verify --print-certs "$apk" | grep -m1 "SHA-256" || true
rm -f "$out"/*.idsig
