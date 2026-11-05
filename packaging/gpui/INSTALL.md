# Creditia GPUI candidate

This package contains Creditia's native application, the analysis executable, a
matching Node runtime, the shared artwork renderer, and its fonts. Installing
Node or downloading fonts is not required. Keep the application and its resources
together. `package-info.json` records the source commit and every packaged file.

## Install

- macOS 15 or later: open the DMG and copy `Creditia.app` to Applications. Apple
  Silicon and Intel packages are separate. Candidate builds are ad-hoc signed;
  public distribution requires Developer ID signing and notarization.
- Windows 10 or later, x64: run the installer, or extract the complete portable
  ZIP to a writable location and run `creditia-gpui.exe`. The `resources` folder
  and `creditia-core.exe` must remain beside the application.
- Linux x64: extract the archive and run `./creditia-gpui`. The build baseline is
  Ubuntu 24.04 with Wayland or X11, fontconfig, xkbcommon and a Vulkan driver.
  The bundled desktop file may be installed into the user's applications folder
  with an absolute `Exec` path. Runtime system libraries are recorded by CI.

The first launch reuses the existing Creditia library and archives. Preferences
are versioned separately from the archive data. Keep a backup of the library
before using a migration candidate with valuable projects.

## Upgrade and rollback

1. Quit both desktop versions and let active analysis/export jobs finish or cancel.
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
