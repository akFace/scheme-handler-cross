# scheme-handler-cross — Rust 构建跨平台版

基于 [LuckyPuppy514/url-scheme-handler](https://github.com/LuckyPuppy514/url-scheme-handler) 的实际功能，以 Rust 语言重写，构建跨平台，支持 Windows、Linux、macOS。

## 简介

- 为应用添加自定义 URL Scheme 以便从浏览器调用
- 支持 Windows、Linux、macOS。
- 目前 Windows、macOS 已经测试功能正常、Linux 未测试（没设备）

## 🧱 下载安装

- 🎯 [前往 releases 下载安装](https://github.com/akFace/scheme-handler-cross/releases)，选择对应的平台
- 👉 详细安装方法： [查看 RELEASE.md](https://github.com/akFace/scheme-handler-cross/blob/main/RELEASE.md)

## ✍️ 使用

1. 点击 `+ Add to Registry` 添加注册表
2. 点击 `+` 添加应用
3. 在左边输入框填写应用名称
4. 在右边选择需要调用的应用

- 以 MPV 播放器为例：如图 ② 步骤中 -->Windows 系列选择播放器安装目录下的`mpv.exe`，macOS 系统选择 `/Applications/mpv.app`，Linux 未测试（没设备）

![20241125202543](https://github.com/akFace/scheme-handler-cross/blob/main/screenshot/Snipaste_2026-07-16_17-53-00.jpg?raw=true)

> **注意：应用名称应与脚本中的唤起应用名称保持一致（大小写也需一致）**

## ✍️ 用法

```text
ush://${app_name}?${gzip_args}
```

参考代码

```text
// @require                 https://lf26-cdn-tos.bytecdntp.com/cdn/expire-1-y/pako/2.0.4/pako.min.js
```

```javascript
function compress(str) {
  return btoa(String.fromCharCode(...pako.gzip(str)));
}

const app_name = "MPV";
const args = [
  '"https://example.com/example.mp4"',
  '--force-media-title="scheme-handler"',
];

window.open(`ush://${app_name}?${compress(args.join(" "))}`, "_self");
```

实际执行命令

```bat
app_path "https://example.com/example.mp4" --force-media-title="scheme-handler"
```

## 文档

[查看 wiki](https://github.com/akFace/scheme-handler-cross/wiki)

## Release packages

GitHub Actions publishes install-oriented packages:

- Windows: `*-windows-x86_64-portable EXE` — Inno Setup installer, registers `ush://` for the current user.
- Linux: `*_amd64.deb` — Debian/Ubuntu package, installs the binary and desktop handler.
- macOS Intel: `*-macos-x86_64.dmg` — drag the app into Applications.
- macOS Apple Silicon: `*-macos-arm64.dmg` — drag the app into Applications.

Run the `Release` workflow from GitHub Actions and select `patch`, `minor`, or `major`. The workflow updates the Cargo version, refreshes `Cargo.lock`, prepends a changelog entry, commits, tags, builds all packages, and publishes the GitHub Release.

For public macOS distribution, add Apple Developer signing and notarization secrets later; the unsigned DMG is suitable for development/testing but may trigger Gatekeeper warnings.
