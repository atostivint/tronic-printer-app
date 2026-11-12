#!/bin/sh
# Wrap the release build in "Tronic Printer.app" so macOS can ask for Bluetooth permission
# (a bare binary has no Info.plist, so the system refuses it Bluetooth). Run on a Mac:
#
#     cargo build --release && scripts/bundle-macos.sh
#
# The result is target/release/bundle/Tronic Printer.app, ad-hoc signed (not notarized).
set -eu

cd "$(dirname "$0")/.."
target="${CARGO_TARGET_DIR:-target}/release"
app="$target/bundle/Tronic Printer.app"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
sed "s/@VERSION@/$version/g" packaging/macos/Info.plist > "$app/Contents/Info.plist"
cp "$target/tronic-printer" "$app/Contents/MacOS/"
# The helpers live next to the app so `tronic-mcp` is found by the app's MCP setup text.
cp "$target/tronic-print" "$target/tronic-mcp" "$app/Contents/MacOS/"

# An .icns from the biggest icon the repository has, when the macOS tools are there. The 512 px
# source the script first asked for does not exist (only icon-256.png and logo-96.png do), and a
# missing file made `sips` fail, `iconutil` fail, and `set -eu` stop the script before the
# signature: the bundle shipped unsigned and unusable for Bluetooth. A failure here now only
# costs the icon.
if command -v sips >/dev/null && command -v iconutil >/dev/null; then
    source_png=assets/icon-256.png
    set_dir="$(mktemp -d)/icon.iconset"
    mkdir -p "$set_dir"
    for size in 16 32 64 128 256 512; do
        sips -z "$size" "$size" "$source_png" --out "$set_dir/icon_${size}x${size}.png" >/dev/null 2>&1 || true
        sips -z $((size * 2)) $((size * 2)) "$source_png" --out "$set_dir/icon_${size}x${size}@2x.png" >/dev/null 2>&1 || true
    done
    if ! iconutil -c icns "$set_dir" -o "$app/Contents/Resources/icon.icns" 2>/dev/null; then
        echo "warning: no icon.icns (iconutil refused $set_dir); the app still runs" >&2
    fi
fi

# Ad-hoc signature: enough to run locally. A build downloaded from the internet is quarantined
# until the user right-clicks > Open (or runs: xattr -dr com.apple.quarantine "Tronic Printer.app").
codesign --force --deep --sign - "$app"
echo "Built $app"
