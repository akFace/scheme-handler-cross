#!/bin/sh
set -eu

TARGET="${TARGET:-$(rustc -vV | awk '/^host:/{print $2}')}"
VERSION="${VERSION:-$(sed -nE 's/^version = "([^"]+)"/\1/p' Cargo.toml | head -1)}"
APP="URL Scheme Handler.app"
BIN="target/${TARGET}/release/url-scheme-handler"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/url-scheme-handler"

sed "s/__VERSION__/${VERSION}/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"
chmod +x "$APP/Contents/MacOS/url-scheme-handler"

printf '%s\n' "Built $APP for $TARGET ($VERSION)"
