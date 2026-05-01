#!/usr/bin/env bash
# scripts/build/macos.sh — Build macOS .app bundle and optional .dmg
#
# Usage:
#   ./scripts/build/macos.sh                      # .app (native arch)
#   ./scripts/build/macos.sh --arch arm64          # .app (arm64 only)
#   ./scripts/build/macos.sh --arch x86_64         # .app (x86_64 only)
#   ./scripts/build/macos.sh --universal           # .app (arm64 + x86_64)
#   ./scripts/build/macos.sh --dmg                 # .app + .dmg
#   ./scripts/build/macos.sh --arch arm64 --dmg    # arm64 .app + .dmg
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
SIGNING_IDENTITY="${CODESIGN_IDENTITY:-}"

# ── Parse args ────────────────────────────────────────────────────────────
BUILD_DMG=false
DO_SIGN=false
UNIVERSAL=false
TARGET_ARCH=""

for arg in "$@"; do
    case "$arg" in
        --dmg)       BUILD_DMG=true ;;
        --sign)      DO_SIGN=true ;;
        --universal) UNIVERSAL=true ;;
        --arch=*)    TARGET_ARCH="${arg#*=}" ;;
        --arch)      ;; # value comes as next arg, handled below
        --sign-identity=*) SIGNING_IDENTITY="${arg#*=}"; DO_SIGN=true ;;
        --help|-h)
            echo "Usage: $0 [--arch arm64|x86_64] [--universal] [--dmg] [--sign] [--sign-identity=ID]"
            exit 0
            ;;
        *)
            # Handle --arch <value> (space-separated)
            if [[ "${PREV_ARG:-}" == "--arch" ]]; then
                TARGET_ARCH="$arg"
                PREV_ARG=""
                continue
            fi
            echo "Unknown arg: $arg"; exit 1
            ;;
    esac
    PREV_ARG="$arg"
done

# Validate --arch value
if [[ -n "$TARGET_ARCH" ]]; then
    case "$TARGET_ARCH" in
        arm64)   TARGET_ARCH="aarch64-apple-darwin" ; ARCH_LABEL="arm64" ;;
        x86_64)  TARGET_ARCH="x86_64-apple-darwin"  ; ARCH_LABEL="x86_64" ;;
        aarch64-apple-darwin) ARCH_LABEL="arm64" ;;
        x86_64-apple-darwin)  ARCH_LABEL="x86_64" ;;
        *) echo "ERROR: Invalid --arch value '$TARGET_ARCH'. Use arm64 or x86_64."; exit 1 ;;
    esac
fi

# --arch and --universal are mutually exclusive
if [[ -n "$TARGET_ARCH" ]] && [[ "$UNIVERSAL" == true ]]; then
    echo "ERROR: --arch and --universal are mutually exclusive."
    exit 1
fi

# ── Step 1: Check prerequisites ──────────────────────────────────────────
echo "==> Checking prerequisites..."

if [[ "$BUILD_DMG" == true ]] && ! command -v create-dmg &>/dev/null; then
    echo "ERROR: create-dmg not found. Install with: brew install create-dmg"
    exit 1
fi

# Get version from Cargo.toml (needed for DMG filename)
APP_VERSION=$(grep '^version' crates/suprim-app/Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')

# ── Step 2: Build binary ─────────────────────────────────────────────────
if [[ "$UNIVERSAL" == true ]]; then
    echo "==> Building universal binary (arm64 + x86_64)..."
    cargo build --release --target aarch64-apple-darwin -p suprim-app
    cargo build --release --target x86_64-apple-darwin -p suprim-app

    ARM_BIN="target/aarch64-apple-darwin/release/${BIN_NAME}"
    X86_BIN="target/x86_64-apple-darwin/release/${BIN_NAME}"

    if [[ ! -f "$ARM_BIN" ]] || [[ ! -f "$X86_BIN" ]]; then
        echo "ERROR: One or both architecture builds failed."
        exit 1
    fi

    RELEASE_BIN="target/release/${BIN_NAME}"
    mkdir -p target/release
    lipo -create "$ARM_BIN" "$X86_BIN" -output "$RELEASE_BIN"
    echo "    ✓ Universal binary: $(lipo -archs "$RELEASE_BIN")"
    ARCH_LABEL="universal"
elif [[ -n "$TARGET_ARCH" ]]; then
    echo "==> Building ${ARCH_LABEL} binary (${TARGET_ARCH})..."
    cargo build --release --target "$TARGET_ARCH" -p suprim-app

    CROSS_BIN="target/${TARGET_ARCH}/release/${BIN_NAME}"
    if [[ ! -f "$CROSS_BIN" ]]; then
        echo "ERROR: Build failed for ${TARGET_ARCH}."
        exit 1
    fi

    RELEASE_BIN="target/release/${BIN_NAME}"
    mkdir -p target/release
    cp "$CROSS_BIN" "$RELEASE_BIN"
    echo "    ✓ ${ARCH_LABEL} binary built"
else
    echo "==> Building release binary..."
    cargo build --release -p suprim-app
    RELEASE_BIN="target/release/${BIN_NAME}"
    # Detect native arch label
    ARCH_LABEL="$(uname -m)"
    [[ "$ARCH_LABEL" == "arm64" ]] || [[ "$ARCH_LABEL" == "aarch64" ]] && ARCH_LABEL="arm64"
fi

# ── Step 2b: Assemble .app bundle (no cargo-bundle needed) ───────────────
echo "==> Assembling .app bundle..."
rm -rf "$APP_PATH"
mkdir -p "${APP_PATH}/Contents/MacOS"
mkdir -p "${APP_PATH}/Contents/Resources"

cp "$RELEASE_BIN" "${APP_PATH}/Contents/MacOS/${BIN_NAME}"

# Write Info.plist
cat > "${APP_PATH}/Contents/Info.plist" << PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleDisplayName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleIdentifier</key>
    <string>com.suprim.sql</string>
    <key>CFBundleVersion</key>
    <string>${APP_VERSION}</string>
    <key>CFBundleShortVersionString</key>
    <string>${APP_VERSION}</string>
    <key>CFBundleExecutable</key>
    <string>${BIN_NAME}</string>
    <key>CFBundleIconFile</key>
    <string>icon</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSMinimumSystemVersion</key>
    <string>12.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
</dict>
</plist>
PLIST

# Copy icon
ICNS_SRC="assets/icons/icon.icns"
if [[ -f "$ICNS_SRC" ]]; then
    cp "$ICNS_SRC" "${APP_PATH}/Contents/Resources/icon.icns"
    echo "    ✓ App icon copied"
else
    echo "    ⚠ No icon.icns found"
fi

echo "    ✓ ${APP_PATH}"

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
    DMG_OUTPUT="target/release/suprimsql-${APP_VERSION}-macos-${ARCH_LABEL}.dmg"
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
echo "  arch:  ${ARCH_LABEL}"
echo ""
echo "To run: open \"${APP_PATH}\""
