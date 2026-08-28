# Scheme-Handler — Rust 构建跨平台版

基于 [LuckyPuppy514/url-scheme-handler](https://github.com/LuckyPuppy514/url-scheme-handler) 的实际功能，以 Rust 语言重写，构建跨平台，支持 Windows、Linux、macOS。

## 简介

- 为应用添加自定义 URL Scheme 以便从浏览器调用
- 支持 Windows、Linux、macOS。
- 支持自定义参数
- 支持可选的本地 HTTP Bridge，把浏览器脚本通过获取到的 m3u8 文本内容提供给 mpv
- 使用方法可参考此项目：[play-with-mpv](https://github.com/akFace/play-with-mpv)

## 🧱 下载安装

- 🎯 [前往 releases 下载安装](https://github.com/akFace/scheme-handler-cross/releases)，选择对应的平台
- 👉 详细安装方法： [查看 RELEASE.md](https://github.com/akFace/scheme-handler-cross/blob/main/RELEASE.md)

## ✍️ 使用

1. 点击 `+` 添加应用
2. 在左边输入框填写应用名称，在右边选择需要调用的应用
3. 点击 `+ Add to Registry` 添加注册表

- 以 MPV 播放器为例：如图 ② 步骤中 -->Windows 系列选择播放器安装目录下的`mpv.exe`，macOS 系统选择 `/Applications/mpv.app`，Linux 系统选择`/bin/mpv`

![20241125202543](https://github.com/akFace/scheme-handler-cross/blob/main/screenshot/Snipaste_2026-07-16_17-53-00.jpg?raw=true)

> **注意：应用名称应与脚本中的唤起应用名称保持一致（大小写也需一致）**

## ✍️ 用法（以下所有内容为开发者文档）

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

## m3u8 HTTP Bridge（可选）

浏览器 JavaScript 无法把 Blob URL / Blob 中的 m3u8 内容直接作为文件交给 mpv 时，可以启用本地 HTTP Bridge。

### 1. 先通过 URL Scheme 请求启动 Bridge

```text
ush://play?needServer=1
```

这是一个控制 URL，不携带 m3u8 内容。scheme-handler 收到后会启动独立的 HTTP Bridge 进程；Linux AppImage 会使用实际的 `.AppImage` 文件重新启动 Bridge，而不是依赖 AppImage 临时挂载目录。

只要 URL 中存在：

```text
needServer=1
```

scheme-handler 就会启动独立的本地 HTTP Server。

不带 `needServer=1` 的 URL 不会启动 Bridge，原有 `ush://` 行为完全不变。

### 2. 油猴脚本等待 Bridge

Bridge 默认监听：

```text
http://127.0.0.1:17891
```

状态接口：

```http
GET /api/status
```

### 3. 上传 m3u8

不要把 m3u8 Base64 后塞进 `ush://`。直接 POST 原始文本：

```javascript
const response = await fetch("http://127.0.0.1:17891/api/m3u8", {
  method: "POST",
  headers: {
    "Content-Type": "application/vnd.apple.mpegurl",
  },
  body: m3u8Text,
});

const { url } = await response.json();
```

返回：

```json
{
  "url": "http://127.0.0.1:17891/m3u8/xxxxxxxx"
}
```

把这个 URL 交给 mpv 即可播放。

### Bridge 的职责

Bridge **只负责保存和提供 m3u8 文本**：

```text
油猴 fetch Blob
      ↓
blob.text()
      ↓
POST /api/m3u8
      ↓
scheme-handler 内存缓存
      ↓
GET /m3u8/{id} 并返回http形式的m3u8给script
      ↓
script按照原有方式：ush://${app_name}?xxx
```

Bridge 不代理视频分片，不修改 m3u8，也不处理 Referer、Cookie、Authorization 等鉴权信息。m3u8 中的完整 HTTP 分片 URL 由 播放器 自己请求；原来由油猴脚本传给 播放器 的请求头继续由 播放器 使用。

缓存只存在内存中，默认最后访问 30 分钟后自动清理，单个 m3u8 最大 16 MiB。

Bridge 仅绑定 `127.0.0.1`，不会监听局域网地址。

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
