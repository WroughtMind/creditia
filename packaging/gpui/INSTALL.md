# Creditia GPUI candidate

This package contains Creditia's native application, the analysis executable, a
matching Node runtime, the shared artwork renderer, and its fonts. Installing
Node or downloading fonts is not required. Keep the application and its resources
together. `package-info.json` records the source commit and every packaged file.

## Install

- **macOS 15 or later.** Choose the Apple Silicon or Intel build. Open the DMG
  and drag `Creditia.app` into Applications. These builds are ad-hoc signed and
  have not been notarized by Apple. If macOS blocks first launch, verify the
  download and try opening Creditia from Applications once. Then go to
  **System Settings → Privacy & Security → Open Anyway** and confirm.
  [Apple's installation guidance](https://support.apple.com/102445)

  **macOS 15 或更新版本。** 选择对应芯片的安装包，打开 DMG，将 `Creditia.app` 拖入「应用程序」。
  此版本未经 Apple 公证。若已核对下载来源与校验值，且 macOS 提示无法验证或已损坏，
  先从「应用程序」尝试打开一次，再到**系统设置 → 隐私与安全性 → 仍要打开**并确认。

  You can also copy the command below into Terminal and then open Creditia from
  Applications. DMG users can copy it from `首次打开说明 - First Launch.txt`.
  也可以在「终端」运行下方命令，再从「应用程序」打开 Creditia。DMG 内的首次打开说明可直接复制命令。

  ```sh
  xattr -dr com.apple.quarantine /Applications/Creditia.app
  ```

- **Windows 10 or later, x64.** Run the unsigned installer. If the verified
  download triggers a SmartScreen **Windows protected your PC** prompt, choose
  **More info → Run anyway**. At the end of setup, you can select **Launch
  Creditia**. For the portable ZIP, extract the entire archive to a writable
  location and run `creditia-gpui.exe`. Keep `resources` and
  `creditia-core.exe` beside the app in either installation.

  **Windows 10 或更新版本，x64。** 运行未签名的安装程序。确认下载来源与校验值后，
  若 SmartScreen 提示「Windows 已保护你的电脑」，请选择**更多信息 → 仍要运行**。
  安装完成后可以勾选「打开 Creditia」。使用便携 ZIP 时，完整解压到可写目录，
  再运行 `creditia-gpui.exe`。请将 `resources` 文件夹和 `creditia-core.exe` 与应用保留在同一目录。

- Linux x64: extract the archive and run `./creditia-gpui`. The build baseline is
  Ubuntu 24.04 with Wayland or X11, fontconfig, xkbcommon and a Vulkan driver.
  Install the DEB with `sudo apt install ./creditia-gpui-*.deb` so required system
  libraries are resolved. The DEB includes its own Node runtime.
  The project chooser and Save dialog use xdg-desktop-portal with a FileChooser backend.
  Tarball users need the GTK, GNOME, KDE or XApp portal used by their desktop session.
  The bundled desktop file may be installed into the user's applications folder
  with an absolute `Exec` path. Runtime system libraries are recorded by CI.

These are the GPUI Kit platform minimums, not a claim that every OS version has
been tested. The automated runners are macOS 15 on both architectures, Windows
Server 2022 x64, and Ubuntu 24.04 x64; their actual versions and execution evidence
travel with the build. Developer ID/notarization and Windows code signing are
optional distribution choices; certificates are not required to build these
installable packages.

The first launch reuses the existing Creditia library, archives and `app.json`
settings. Keep a backup of the library before upgrading.

## Upgrade and rollback

Previous installers and their checksums are available on the [Releases page](https://github.com/WroughtMind/creditia/releases).

1. Finish or cancel active analysis and export jobs, then quit Creditia.
2. Preserve the previous application directory or installer. Do not delete the
   Creditia library, project `.creditia` directories, or evidence database.
3. Install/extract the new version into a new directory. The resource manifest,
   renderer and executable must come from the same build.
4. Start the candidate and verify project counts, remembered settings, a real
   analysis and an export. CI reports travel with the candidate; they do not
   replace real-device acceptance of the display server, dialogs and GPU driver.
5. To roll back, quit the candidate and launch the previous application. No
   uninstall action deletes user archives or evidence. If a later schema requires
   a data migration, the release must first provide a tested reversible migrator.

To validate runtime discovery without opening a window, run
`creditia-gpui --check-resources` (on macOS the executable is inside
`Creditia.app/Contents/MacOS`). It uses the bundled renderer and analysis core.

## Support evidence

Report the commit and target from `package-info.json`, system version, graphics
driver, and the failing action. Do not attach private agent logs or archives
without reviewing their contents. The automated native screenshots, package
smoke report and shared-renderer comparison are separate acceptance records.
