#!/bin/sh
set -eu

if [ "$(uname -s)" != Darwin ]; then
    echo "Run this script on macOS with Rust and Xcode command-line tools installed." >&2
    exit 1
fi

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cargo build --release --locked --manifest-path "$project_dir/Cargo.toml" --target-dir "$project_dir/target"
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$project_dir/Cargo.toml" | head -n 1)
test -n "$version"
app="$project_dir/target/macos/Media Shelf.app"
mkdir -p "$app/Contents/MacOS"
cp "$project_dir/target/release/media-shelf" "$app/Contents/MacOS/media-shelf"
cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key><string>media-shelf</string>
    <key>CFBundleIdentifier</key><string>com.alinooshabadi.media-shelf</string>
    <key>CFBundleName</key><string>Media Shelf</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$version</string>
    <key>CFBundleVersion</key><string>$version</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSPrincipalClass</key><string>NSApplication</string>
</dict>
</plist>
EOF
test -x "$app/Contents/MacOS/media-shelf"
plutil -lint "$app/Contents/Info.plist"
codesign --force --sign - "$app"
codesign --verify "$app"
echo "Built $app ($(uname -m), locally signed)"
