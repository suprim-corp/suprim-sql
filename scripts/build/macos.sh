#!/usr/bin/env bash
# scripts/build/macos.sh — Build macOS .app bundle and optional .dmg
#
# Usage:
#   ./scripts/build/macos.sh                      # .app (native arch)
#   ./scripts/build/macos.sh --universal           # .app (arm64 + x86_64)
#   ./scripts/build/macos.sh --dmg                 # .app + .dmg
#   ./scripts/build/macos.sh --universal --dmg     # universal .app + .dmg
#   ./scripts/build/macos.sh --sign                # .app + codesign
#   ./scripts/build/macos.sh --universal --dmg --sign  # full pipeline
#
# Prerequisites:
#   cargo install cargo-bundle
#   brew install create-dmg        (only for --dmg)
#   rustup target add x86_64-apple-darwin    (only for --universal on arm64 mac)
#   rustup target add aarch64-apple-darwin   (only for --universal on x86 mac)

set -euo pipefail

# ── Config ────────────────────────────────────────────────────────────────
APP_NAME="SuprimSQL"
BIN_NAME="SuprimSQL"
BUNDLE_DIR="target/release/bundle/osx"
APP_PATH="${BUNDLE_DIR}/${APP_NAME}.app"
DMG_NAME="${APP_NAME}"
DMG_OUTPUT="target/release/${DMG_NAME}.dmg"
SIGNING_IDENTITY="${CODESIGN_IDENTITY:-}"

# ── Parse args ────────────────────────────────────────────────────────────
BUILD_DMG=false
DO_SIGN=false
UNIVERSAL=false

for arg in "$@"; do
    case "$arg" in
        --dmg)       BUILD_DMG=true ;;
        --sign)      DO_SIGN=true ;;
        --universal) UNIVERSAL=true ;;
        --sign-identity=*) SIGNING_IDENTITY="${arg#*=}"; DO_SIGN=true ;;
        --help|-h)
            echo "Usage: $0 [--universal] [--dmg] [--sign] [--sign-identity=ID]"
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

# ── Step 2: Build binary ─────────────────────────────────────────────────
if [[ "$UNIVERSAL" == true ]]; then
    echo "==> Building universal binary (arm64 + x86_64)..."

    # Build both architectures
    cargo build --release --target aarch64-apple-darwin -p suprim-app
    cargo build --release --target x86_64-apple-darwin -p suprim-app

    ARM_BIN="target/aarch64-apple-darwin/release/${BIN_NAME}"
    X86_BIN="target/x86_64-apple-darwin/release/${BIN_NAME}"

    if [[ ! -f "$ARM_BIN" ]] || [[ ! -f "$X86_BIN" ]]; then
        echo "ERROR: One or both architecture builds failed."
        echo "  arm64: ${ARM_BIN} $([ -f "$ARM_BIN" ] && echo '✓' || echo '✗')"
        echo "  x86_64: ${X86_BIN} $([ -f "$X86_BIN" ] && echo '✓' || echo '✗')"
        exit 1
    fi

    # Create universal binary with lipo
    UNIVERSAL_BIN="target/release/${BIN_NAME}"
    mkdir -p target/release
    lipo -create "$ARM_BIN" "$X86_BIN" -output "$UNIVERSAL_BIN"
    echo "    ✓ Universal binary: $(file "$UNIVERSAL_BIN" | sed 's/.*: //')"

    # Now bundle using cargo-bundle (it will use the binary we just placed)
    echo "==> Bundling .app..."
    cargo bundle --release --format osx -p suprim-app

    # Replace the single-arch binary in the bundle with our universal one
    cp "$UNIVERSAL_BIN" "${APP_PATH}/Contents/MacOS/${BIN_NAME}"
    echo "    ✓ Replaced bundle binary with universal binary"
else
    echo "==> Building release binary + .app bundle..."
    cargo bundle --release --format osx -p suprim-app
fi

if [[ ! -d "$APP_PATH" ]]; then
    echo "ERROR: Expected .app not found at ${APP_PATH}"
    exit 1
fi

echo "    ✓ ${APP_PATH}"

# ── Step 2b: Patch Info.plist ────────────────────────────────────────────
PLIST="${APP_PATH}/Contents/Info.plist"
echo "==> Patching Info.plist..."

/usr/libexec/PlistBuddy -c "Delete :LSRequiresCarbon" "$PLIST" 2>/dev/null || true
/usr/libexec/PlistBuddy -c "Delete :CSResourcesFileMapped" "$PLIST" 2>/dev/null || true

# Set minimum OS version
/usr/libexec/PlistBuddy -c "Delete :LSMinimumSystemVersion" "$PLIST" 2>/dev/null || true
/usr/libexec/PlistBuddy -c "Add :LSMinimumSystemVersion string 12.0" "$PLIST"

echo "    ✓ Info.plist patched"

# ── Step 2c: Copy app icon into bundle ───────────────────────────────────
ICNS_SRC="assets/icons/icon.icns"
if [[ -f "$ICNS_SRC" ]]; then
    RESOURCES_DIR="${APP_PATH}/Contents/Resources"
    mkdir -p "$RESOURCES_DIR"
    cp "$ICNS_SRC" "$RESOURCES_DIR/icon.icns"
    /usr/libexec/PlistBuddy -c "Delete :CFBundleIconFile" "$PLIST" 2>/dev/null || true
    /usr/libexec/PlistBuddy -c "Add :CFBundleIconFile string icon" "$PLIST"
    echo "    ✓ App icon copied"
else
    echo "    ⚠ No icon.icns found at ${ICNS_SRC}"
fi

# ── Step 3: Code signing (optional) ──────────────────────────────────────
if [[ "$DO_SIGN" == true ]]; then
    if [[ -z "$SIGNING_IDENTITY" ]]; then
        echo "ERROR: No signing identity. Set CODESIGN_IDENTITY env var or use --sign-identity=ID"
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
    rm -f "$DMG_OUTPUT"

    # Stage a clean folder with ONLY the .app — no hidden files, no extras.
    DMG_STAGING="target/release/dmg-staging"
    rm -rf "$DMG_STAGING"
    mkdir -p "$DMG_STAGING"
    cp -R "$APP_PATH" "$DMG_STAGING/"

    create-dmg \
        --volname "${APP_NAME}" \
        --window-pos 200 120 \
        --window-size 600 400 \
        --icon-size 100 \
        --icon "${APP_NAME}.app" 175 190 \
        --hide-extension "${APP_NAME}.app" \
        --app-drop-link 425 190 \
        --no-internet-enable \
        "$DMG_OUTPUT" \
        "$DMG_STAGING"

    rm -rf "$DMG_STAGING"
    echo "    ✓ ${DMG_OUTPUT}"
fi

# ── Done ──────────────────────────────────────────────────────────────────
echo ""
echo "=== Build complete ==="
echo "  .app:  ${APP_PATH}"
[[ "$BUILD_DMG" == true ]] && echo "  .dmg:  ${DMG_OUTPUT}"
[[ "$UNIVERSAL" == true ]] && echo "  arch:  universal (arm64 + x86_64)"
echo ""
echo "To run: open \"${APP_PATH}\""
