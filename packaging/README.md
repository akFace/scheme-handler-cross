# Packaging assets

`packaging/icon.png` is the canonical Linux/AppImage icon source.

Platform-specific formats are kept where the native toolchain requires them:

- `windows/icon.ico` - embedded into the Windows EXE by `build.rs` via `winres`.
- `macos/icon.icns` - embedded in the `.app` bundle by `build-app.sh`.
- `linux/url-scheme-handler.desktop` - Linux desktop entry used by DEB/AppImage.

The Linux desktop entry uses `Icon=url-scheme-handler`; the GitHub Actions workflow copies `packaging/icon.png` into the standard AppDir hicolor icon directory before running `linuxdeploy`.
