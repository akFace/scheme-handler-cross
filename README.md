- # URL Scheme Handler — Rust 2.1

基于 `LuckyPuppy514/url-scheme-handler` 的实际行为利用 AI 重写，支持 Windows、Linux、macOS。

## 直接构建

### Windows

安装 Rust MSVC 工具链和 Visual Studio Build Tools 2022（Desktop development with C++），然后：

```powershell
cargo build --release
```

输出：

```text
target\\release\\url-scheme-handler.exe
```

### Linux

Ubuntu/Debian 示例依赖：

```bash
sudo apt update
sudo apt install -y build-essential pkg-config libgtk-3-dev libxdo-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev
```

然后：

```bash
cargo build --release
```

输出：

```text
target/release/url-scheme-handler
```

### macOS

安装 Xcode Command Line Tools：

```bash
xcode-select --install
```

然后：

```bash
cargo build --release
```

macOS 还提供 `.app` 打包脚本：

```bash
./packaging/macos/build-app.sh
open "URL Scheme Handler.app"
```

> macOS 的 Objective-C URL Event bridge 在 `macos/url_handler.m` 中。它通过 `NSAppleEventManager` 处理 App 已运行时收到的 `ush://` URL，不依赖替换 eframe 的 AppDelegate。

## URL 格式

```text
ush://${app_name}?${gzip_args}
```

其中 `gzip_args` 是原项目使用的：

```text
Base64(Gzip(command_line))
```

例如：

```javascript
function compress(str) {
  return btoa(String.fromCharCode(...pako.gzip(str)));
}

const app_name = "MPV";
const args = [
  '"https://example.com/example.mp4"',
  '--force-media-title="URL Scheme Handler"',
];

window.open(`ush://${app_name}?${compress(args.join(" "))}`, "_self");
```

## 平台行为

### Windows

注册：

```text
HKCU\\Software\\Classes\\ush
```

命令：

```text
"<current exe>" run "%1"
```

Windows 保留原项目的 `raw_arg()` 语义，因此原项目的命令行行为不会因为迁移 Rust 而改变。

### Linux

注册：

```text
~/.local/share/applications/url-scheme-handler-ush.desktop
```

并执行：

```text
xdg-mime default url-scheme-handler-ush.desktop x-scheme-handler/ush
```

注销时删除 desktop 文件，并刷新 desktop database，不会把 MIME Handler 指向一个已经不存在的文件。

### macOS

macOS 必须作为 `.app` 使用。`Info.plist` 已经包含：

```xml
CFBundleURLTypes -> CFBundleURLSchemes -> ush
```

程序运行时还注册：

```text
NSAppleEventManager
  kInternetEventClass
  kAEGetURL
```

因此：

1. App 尚未运行：URL 启动 `.app`，URL 可以从启动参数进入。
2. App 已经运行：Launch Services 将 URL 作为 Apple Event 交给现有进程，Objective-C bridge 将 URL 转给 Rust。

## 配置文件

Windows：

```text
<exe目录>/config.json
```

Linux：

```text
$XDG_CONFIG_HOME/url-scheme-handler/config.json
```

或：

```text
~/.config/url-scheme-handler/config.json
```

macOS：

```text
~/Library/Application Support/url-scheme-handler/config.json
```

示例：

```json
{
  "is_registry_added": true,
  "apps": [
    {
      "name": "MPV",
      "path": "/usr/local/bin/mpv"
    }
  ]
}
```

## 注意

1. Linux/macOS 的目标程序参数采用 shell-style 参数解析；Windows 保持原项目的 raw command line 行为。
2. URL 内容最终会启动本地用户配置的程序，不要允许不可信网页任意构造 `ush://` 参数。
3. macOS 发布时建议对 `.app` 进行代码签名和 notarization。
4. `cargo build --release` 需要在目标平台本机执行，或使用对应的 Rust target 和完整的系统 SDK/toolchain。

## Windows 编译

```powershell
cargo clean
cargo build --release
```

本版本已修复 eframe `App::save(&mut self, &mut dyn Storage)` 与项目自身 `Config::save()` 同名导致的编译错误；项目方法已改名为 `persist()`。同时修复 Windows 下的条件编译警告。

## v2.1.2 build note

This release fixes a Rust syntax error in the conditional macOS imports. The
`#[cfg(target_os = "macos")]` attribute is now applied to a separate `use`
declaration, so Windows and Linux no longer parse a cfg attribute inside the
standard-library `use` tree.

## Release packages

GitHub Actions publishes install-oriented packages:

- Windows: `*-windows-x86_64-portable EXE` — Inno Setup installer, registers `ush://` for the current user.
- Linux: `*_amd64.deb` — Debian/Ubuntu package, installs the binary and desktop handler.
- macOS Intel: `*-macos-x86_64.dmg` — drag the app into Applications.
- macOS Apple Silicon: `*-macos-arm64.dmg` — drag the app into Applications.

Run the `Release` workflow from GitHub Actions and select `patch`, `minor`, or `major`. The workflow updates the Cargo version, refreshes `Cargo.lock`, prepends a changelog entry, commits, tags, builds all packages, and publishes the GitHub Release.

For public macOS distribution, add Apple Developer signing and notarization secrets later; the unsigned DMG is suitable for development/testing but may trigger Gatekeeper warnings.
