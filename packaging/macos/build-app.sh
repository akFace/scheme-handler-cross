#!/bin/sh
set -eu

TARGET="${TARGET:-$(rustc -vV | awk '/^host:/{print $2}')}"
VERSION="${VERSION:-$(sed -nE 's/^version = "([^"]+)"/\1/p' Cargo.toml | head -1)}"
APP="scheme-handler.app"
BIN="target/${TARGET}/release/scheme-handler"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/scheme-handler"
cp "packaging/macos/icon.icns" "$APP/Contents/Resources/icon.icns"

sed "s/__VERSION__/${VERSION}/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"
chmod +x "$APP/Contents/MacOS/scheme-handler"

printf '%s\n' "Built $APP for $TARGET ($VERSION)"
