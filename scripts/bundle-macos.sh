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

# An .icns from the PNG logo, when the macOS tools are there.
if command -v sips >/dev/null && command -v iconutil >/dev/null; then
    set_dir="$(mktemp -d)/icon.iconset"
    mkdir -p "$set_dir"
    for size in 16 32 64 128 256 512; do
        sips -z "$size" "$size" assets/logo-512.png --out "$set_dir/icon_${size}x${size}.png" >/dev/null
        sips -z $((size * 2)) $((size * 2)) assets/logo-512.png --out "$set_dir/icon_${size}x${size}@2x.png" >/dev/null
    done
    iconutil -c icns "$set_dir" -o "$app/Contents/Resources/icon.icns"
fi

# Ad-hoc signature: enough to run locally. A build downloaded from the internet is quarantined
# until the user right-clicks > Open (or runs: xattr -dr com.apple.quarantine "Tronic Printer.app").
codesign --force --deep --sign - "$app"
echo "Built $app"
