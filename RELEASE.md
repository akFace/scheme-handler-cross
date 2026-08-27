# Release / 安装说明

## Windows

Release 提供两个文件：

- `scheme-handler-<version>-windows-x86_64.exe`
- `scheme-handler-<version>-windows-x86_64-portable.zip`

Windows 不使用 Setup 安装器。

直接双击 `.exe` 即可运行。

Linux 会在程序启动时自动注册 `ush://` URL Scheme；AppImage 会使用实际 AppImage 文件路径注册，因此退出应用后仍可被浏览器唤起。

也可以直接把 `.exe` 放到任意目录使用，不需要管理员权限。

## Linux

Release 提供：

```text
scheme-handler_<version>_amd64.deb
```

Ubuntu / Debian：

```bash
sudo apt install ./scheme-handler_<version>_amd64.deb
```

安装后会注册：

```text
x-scheme-handler/ush
```

卸载：

```bash
sudo apt remove scheme-handler
```

## macOS

Release 提供：

- Intel: `*-macos-x86_64.dmg`
- Apple Silicon: `*-macos-arm64.dmg`

DMG 未进行 Apple Developer 签名和 notarization。

安装：

1. 打开对应 DMG。
2. 将 `scheme-handler.app` 拖到 `Applications`。
3. 第一次打开时，如果 macOS 阻止应用运行，进入：

   **系统设置 → 隐私与安全性**

4. 在安全性提示中允许打开 `scheme-handler`。
5. 再次打开应用。

macOS URL Scheme 已在 `Info.plist` 中注册：

```text
ush://
```

应用不需要运行时修改 `Info.plist`。

## Release 文件

每个版本会自动生成：

```text
Windows:
  scheme-handler-<version>-windows-x86_64.exe
  scheme-handler-<version>-windows-x86_64-portable.zip

Linux:
  scheme-handler_<version>_amd64.deb

macOS:
  scheme-handler-<version>-macos-x86_64.dmg
  scheme-handler-<version>-macos-arm64.dmg
```
