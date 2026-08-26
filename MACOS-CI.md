# macOS CI / URL Scheme behavior

- Intel builds use `macos-15-intel`.
- Apple Silicon builds use `macos-14`.
- `build-app.sh` is invoked through `bash` so checkout file mode does not matter.
- macOS builds are unsigned and not notarized.
- `ush://` is declared in the app bundle's `Info.plist` and registered with Launch Services.
- URL events are handled by a native `NSAppleEventManager` callback and a background Rust worker, independent of the eframe GUI update loop.
- The settings window is initially hidden for URL-triggered launches. A normal manual launch shows the settings window after a short startup grace period.
- The configured macOS `.app` target is launched through `/usr/bin/open -a ... --args ...`, not by executing the `.app` directory.
