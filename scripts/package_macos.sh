#!/usr/bin/env bash
set -euo pipefail

NO_INSTALL=0
OFFLINE=0
UNIVERSAL=0
APP_VERSION=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --no-install)
            NO_INSTALL=1
            shift
            ;;
        --offline)
            OFFLINE=1
            shift
            ;;
        --universal)
            UNIVERSAL=1
            shift
            ;;
        --version)
            if [[ $# -lt 2 ]]; then
                echo "Missing value for --version" >&2
                exit 2
            fi
            APP_VERSION="${2#v}"
            shift 2
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 2
            ;;
    esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"
APP_DIR="$DIST_DIR/Pinleaf.app"
DMG_PATH="$DIST_DIR/Pinleaf.dmg"
STAGING_DIR="$DIST_DIR/.dmg-staging"
ICNS_SRC="$ROOT_DIR/assets/Pinleaf.icns"
PLIST_SRC="$ROOT_DIR/macos/Info.plist"
LSREGISTER="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"

if [[ "$OFFLINE" -eq 1 ]]; then
    CARGO_MODE_FLAG="--offline"
else
    CARGO_MODE_FLAG="--locked"
fi

mkdir -p "$DIST_DIR"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"

if [[ "$UNIVERSAL" -eq 1 ]]; then
    echo "==> Building Pinleaf release binaries (aarch64 + x86_64)..."
    cargo build --release "$CARGO_MODE_FLAG" --manifest-path "$ROOT_DIR/Cargo.toml" --target aarch64-apple-darwin -p pinleaf
    cargo build --release "$CARGO_MODE_FLAG" --manifest-path "$ROOT_DIR/Cargo.toml" --target x86_64-apple-darwin -p pinleaf

    ARM_BIN="$ROOT_DIR/target/aarch64-apple-darwin/release/pinleaf"
    X86_BIN="$ROOT_DIR/target/x86_64-apple-darwin/release/pinleaf"

    if [[ ! -f "$ICNS_SRC" ]]; then
        echo "==> Exporting Pinleaf icon bundle..."
        if [[ "$(uname -m)" == "arm64" ]]; then
            "$ARM_BIN" --export-icons "$ROOT_DIR/assets"
        else
            "$X86_BIN" --export-icons "$ROOT_DIR/assets"
        fi
    fi

    echo "==> Creating universal binary with lipo..."
    /usr/bin/lipo -create "$ARM_BIN" "$X86_BIN" -output "$APP_DIR/Contents/MacOS/pinleaf"
    echo "==> Binary architectures: $(/usr/bin/lipo -archs "$APP_DIR/Contents/MacOS/pinleaf")"
else
    echo "==> Building Pinleaf release binary..."
    cargo build --release "$CARGO_MODE_FLAG" --manifest-path "$ROOT_DIR/Cargo.toml" -p pinleaf

    BINARY_SRC="$ROOT_DIR/target/release/pinleaf"
    if [[ ! -f "$ICNS_SRC" ]]; then
        echo "==> Exporting Pinleaf icon bundle..."
        "$BINARY_SRC" --export-icons "$ROOT_DIR/assets"
    fi

    cp -X "$BINARY_SRC" "$APP_DIR/Contents/MacOS/pinleaf"
fi

echo "==> Assembling $APP_DIR..."
chmod 755 "$APP_DIR/Contents/MacOS/pinleaf"
cp -X "$ICNS_SRC" "$APP_DIR/Contents/Resources/Pinleaf.icns"
cp -X "$PLIST_SRC" "$APP_DIR/Contents/Info.plist"
printf 'APPL????' > "$APP_DIR/Contents/PkgInfo"

if [[ -n "$APP_VERSION" ]]; then
    echo "==> Setting bundle version to $APP_VERSION..."
    /usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $APP_VERSION" "$APP_DIR/Contents/Info.plist"
    /usr/libexec/PlistBuddy -c "Set :CFBundleVersion $APP_VERSION" "$APP_DIR/Contents/Info.plist"
fi

/usr/bin/xattr -cr "$APP_DIR"
/usr/bin/plutil -lint "$APP_DIR/Contents/Info.plist"

echo "==> Codesigning $APP_DIR (ad-hoc)..."
/usr/bin/codesign --force --deep --sign - "$APP_DIR"
/usr/bin/codesign --verify --deep --strict "$APP_DIR"

echo "==> Creating disk image $DMG_PATH..."
rm -rf "$STAGING_DIR" "$DMG_PATH"
mkdir -p "$STAGING_DIR"
cp -R -X "$APP_DIR" "$STAGING_DIR/Pinleaf.app"
ln -s /Applications "$STAGING_DIR/Applications"
/usr/bin/hdiutil create -volname "Pinleaf" -srcfolder "$STAGING_DIR" -ov -format UDZO "$DMG_PATH"
rm -rf "$STAGING_DIR"

if [[ "$NO_INSTALL" -eq 0 ]]; then
    USER_APPS="$HOME/Applications"
    if mkdir -p "$USER_APPS" 2>/dev/null && [[ -w "$USER_APPS" ]]; then
        INSTALLED_APP="$USER_APPS/Pinleaf.app"
        echo "==> Installing $INSTALLED_APP..."
        rm -rf "$INSTALLED_APP"
        cp -R -p "$APP_DIR" "$INSTALLED_APP"
        if [[ -x "$LSREGISTER" ]]; then
            "$LSREGISTER" -f "$INSTALLED_APP" >/dev/null 2>&1 || true
        fi
    fi
fi

if [[ -x "$LSREGISTER" ]]; then
    "$LSREGISTER" -f "$APP_DIR" >/dev/null 2>&1 || true
fi

echo "==> Packaging complete:"
echo "    App: $APP_DIR"
echo "    DMG: $DMG_PATH"
if [[ "$NO_INSTALL" -eq 0 ]] && [[ -d "$HOME/Applications/Pinleaf.app" ]]; then
    echo "    Installed: $HOME/Applications/Pinleaf.app"
fi
