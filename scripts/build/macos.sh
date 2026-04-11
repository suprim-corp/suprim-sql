#!/usr/bin/env bash
# scripts/build/macos.sh — Build macOS .app bundle and optional .dmg
#
# Usage:
#   ./scripts/build/macos.sh              # .app only
#   ./scripts/build/macos.sh --dmg        # .app + .dmg
#   ./scripts/build/macos.sh --sign       # .app + codesign
#   ./scripts/build/macos.sh --dmg --sign # full pipeline
#
# Prerequisites:
#   cargo install cargo-bundle
#   brew install create-dmg  (only for --dmg)

set -euo pipefail

# ── Config ────────────────────────────────────────────────────────────────
APP_NAME="SuprimSQL"
BUNDLE_DIR="target/release/bundle/osx"
APP_PATH="${BUNDLE_DIR}/${APP_NAME}.app"
DMG_NAME="${APP_NAME}"
DMG_OUTPUT="target/release/${DMG_NAME}.dmg"
SIGNING_IDENTITY="${CODESIGN_IDENTITY:-}"  # set via env or --sign-identity

# ── Parse args ────────────────────────────────────────────────────────────
BUILD_DMG=false
DO_SIGN=false

for arg in "$@"; do
    case "$arg" in
        --dmg)  BUILD_DMG=true ;;
        --sign) DO_SIGN=true ;;
        --sign-identity=*) SIGNING_IDENTITY="${arg#*=}"; DO_SIGN=true ;;
        --help|-h)
            echo "Usage: $0 [--dmg] [--sign] [--sign-identity=ID]"
            echo ""
            echo "  --dmg                Create .dmg installer"
            echo "  --sign               Code sign the .app (uses CODESIGN_IDENTITY env var)"
            echo "  --sign-identity=ID   Code sign with specific identity"
            exit 0
            ;;
        *) echo "Unknown arg: $arg"; exit 1 ;;
    esac
done

# ── Step 1: Check prerequisites ──────────────────────────────────────────
echo "==> Checking prerequisites..."

if ! command -v cargo-bundle &>/dev/null; then
    echo "ERROR: cargo-bundle not found. Install with: cargo install cargo-bundle"
    exit 1
fi

if [[ "$BUILD_DMG" == true ]] && ! command -v create-dmg &>/dev/null; then
    echo "ERROR: create-dmg not found. Install with: brew install create-dmg"
    exit 1
fi

# ── Step 2: Build .app bundle ────────────────────────────────────────────
echo "==> Building release binary + .app bundle..."
cargo bundle --release --format osx

if [[ ! -d "$APP_PATH" ]]; then
    echo "ERROR: Expected .app not found at ${APP_PATH}"
    exit 1
fi

echo "    ✓ ${APP_PATH}"

# ── Step 2b: Patch Info.plist ────────────────────────────────────────────
# cargo-bundle generates LSRequiresCarbon=true which causes macOS to open
# a Terminal window alongside the app. Remove it + other unnecessary keys.
PLIST="${APP_PATH}/Contents/Info.plist"
echo "==> Patching Info.plist..."

# Remove LSRequiresCarbon (causes Terminal to open for GUI apps)
/usr/libexec/PlistBuddy -c "Delete :LSRequiresCarbon" "$PLIST" 2>/dev/null || true

# Remove CSResourcesFileMapped (legacy key, not needed)
/usr/libexec/PlistBuddy -c "Delete :CSResourcesFileMapped" "$PLIST" 2>/dev/null || true

echo "    ✓ Info.plist patched"

# ── Step 3: Code signing (optional) ──────────────────────────────────────
if [[ "$DO_SIGN" == true ]]; then
    if [[ -z "$SIGNING_IDENTITY" ]]; then
        echo "ERROR: No signing identity. Set CODESIGN_IDENTITY env var or use --sign-identity=ID"
        echo "  Available identities: security find-identity -v -p codesigning"
        exit 1
    fi

    echo "==> Code signing with identity: ${SIGNING_IDENTITY}..."
    codesign --force --deep --options runtime \
        --sign "$SIGNING_IDENTITY" \
        "$APP_PATH"
    echo "    ✓ Signed"

    echo "==> Verifying signature..."
    codesign --verify --verbose=2 "$APP_PATH"
    echo "    ✓ Signature valid"
fi

# ── Step 4: Create .dmg (optional) ───────────────────────────────────────
if [[ "$BUILD_DMG" == true ]]; then
    echo "==> Creating DMG installer..."

    # Remove old DMG if exists
    rm -f "$DMG_OUTPUT"

    create-dmg \
        --volname "${APP_NAME}" \
        --volicon "assets/icons/icon.icns" \
        --window-pos 200 120 \
        --window-size 600 400 \
        --icon-size 100 \
        --icon "${APP_NAME}.app" 175 190 \
        --hide-extension "${APP_NAME}.app" \
        --app-drop-link 425 190 \
        --no-internet-enable \
        "$DMG_OUTPUT" \
        "$APP_PATH"

    echo "    ✓ ${DMG_OUTPUT}"
fi

# ── Done ──────────────────────────────────────────────────────────────────
echo ""
echo "=== Build complete ==="
echo "  .app:  ${APP_PATH}"
[[ "$BUILD_DMG" == true ]] && echo "  .dmg:  ${DMG_OUTPUT}"
echo ""
echo "To run: open \"${APP_PATH}\""
