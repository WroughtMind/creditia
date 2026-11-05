# Creditia GPUI desktop

This is the migration development application. It uses GPUI Kit 0.9.0 and the
same `creditia-desktop-service` as the existing Tauri desktop. It is not yet the
replacement release. The complete acceptance list is in `docs/gpui-migration/`.

## Run

Install Rust 1.92 or later and the [GPUI Kit platform prerequisites](https://gpui-kit.com/docs/installation/).
The documented Kit baseline is macOS 15+, Windows 10 with MSVC, and Linux with
Wayland/X11 and a Vulkan driver. Both Mac architectures still require release
acceptance on real hardware.

From the repository root:

```sh
cargo build -p creditia-cli
```

Set `CREDITIA_CORE` to the absolute path of the built `creditia-core` executable
(including `.exe` on Windows), then:

```sh
cargo run --manifest-path apps/gpui/Cargo.toml
cargo test --manifest-path apps/gpui/Cargo.toml --lib
```

The sidecar is the existing Rust analysis CLI. No model API or separate analysis
implementation is introduced. The application reads the existing project library
and live or stored archives. New UI preferences are stored under `gpui` in the
existing app settings, preserving unrelated fields. WebView localStorage
preferences have not yet been imported.

## Implemented paths

- Native GPUI window, Root overlay host, system menus and folder selection.
- Existing project library, project switching and actual archive contribution
  summary. Unknown contribution and non-verified evidence remain distinct.
- Add a Git project and run the existing analyzer on the background executor.
- Bounded progress channel, cancellation, loading/error/empty states, and stale
  archive request rejection. Closing the view requests worker cancellation.
- Chinese/English UI, light/dark/system appearance selection, remembered project,
  archive copy and project-folder reveal.
- Cmd/Ctrl+O: open, Cmd/Ctrl+R: refresh, Cmd/Ctrl+Enter: analyze,
  Escape: cancel, Cmd/Ctrl+Shift+C: copy archive, Cmd/Ctrl+Q: quit.

## Still required before cutover

The full exhibition wall, artwork/style/layout controls, zoom/roam/hit regions,
shared artwork preview and animation playback, all export UIs, full project
settings/integrations and background collection UI remain migration work. The
reduced-motion preference is persisted for that work; this foundation does not
animate artwork. System appearance is applied when selected and at startup;
live OS appearance changes need their own acceptance test.

Native dependencies have not been resolved or compiled in the authoring
environment, which has no Rust toolchain. No `Cargo.lock` has been fabricated.
The first successful native dependency resolution must produce and commit the
lockfile before this application can become a release candidate. Unit tests are
provided but must run in CI or a configured native environment. Screenshot,
keyboard/accessibility, native dialogs, GPU, font and platform package validation
are all still pending; source implementation is not a completed acceptance gate.
