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
cargo test --manifest-path apps/gpui/Cargo.toml --all-targets
```

For artwork preview, install Node 22.18 or later and run `npm ci` in
`packages/render`. The application starts the shared JSONL renderer on a
background worker when Preview artwork is pressed. `CREDITIA_NODE` may select a
Node executable; `CREDITIA_RENDER_BRIDGE` may select the absolute bridge script
path. Development defaults point to `packages/render/tools/bridge.mjs` relative
to the build checkout. Packaged renderer discovery and bundled Node are still
release work.

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
- Full-color single artwork preview from the shared renderer, with catalog-driven
  style, piece and layout cycling. Fonts are outlined by the shared renderer.
  Preview requests are cancelled when the selection changes, old results are
  rejected, and replaced image assets are evicted. Context-only pieces requiring
  cross-project or blame input are omitted from this single-archive preview.
- Native save-dialog export for PNG, SVG, PDF, GIF and HTML, using the same
  renderer and catalog sizes. Non-animated pieces disable GIF. Each click captures
  an owned archive/style/piece/layout/language/group/naming/size request before the
  dialog opens; changing the UI later does not change that export. Cancel and
  failure leave an existing destination intact; completed bytes replace it from
  a synchronized temporary file on the same filesystem.
- Cmd/Ctrl+O: open, Cmd/Ctrl+R: refresh, Cmd/Ctrl+Enter: analyze,
  Escape: cancel the active export or analysis, Cmd/Ctrl+E: export,
  Cmd/Ctrl+Shift+C: copy archive, Cmd/Ctrl+Q: quit.

PNG/SVG/PDF and static HTML artwork use the selected dimensions. As in the
shared exporter, GIF uses the selected aspect ratio with a maximum width of
800 px. Scrolling HTML currently requires native size; selecting another size
returns the renderer's explicit `unsupported_size` error without writing a file.
Export progress reports choosing a location,
rendering and writing; the shared synchronous encoder does not report percentages.

## Still required before cutover

The full exhibition wall and final artwork/style/layout selectors, zoom/roam/hit
regions, animation playback, export dock/drag interactions, text/embed export UIs, full project
settings/integrations and background collection UI remain migration work. The
reduced-motion preference is persisted for that work; this foundation does not
animate artwork. System appearance is applied when selected and at startup;
live OS appearance changes need their own acceptance test.

The native dependency lockfile was resolved by CI. The authoring environment has
no Rust toolchain; the dedicated GPUI workflow checks all application and test
targets on four platforms. Type checking does not execute the unit tests or
prove that an application links and launches. Run the test command above in a
configured native environment and inspect the workflow's actual result.
Screenshot,
keyboard/accessibility, native dialogs, GPU, font and platform package validation
are all still pending; source implementation is not a completed acceptance gate.

The single-image preview has a bounded SVG surface and a renderer process
deadline. GPUI rasterizes an SVG preview at its intrinsic scale; this does not
meet the future deep-zoom acceptance requirement. The full wall needs visible
region scheduling and scale-aware caching before cutover.

The current renderer protocol does not offer transparency override or selectable
font-embedding modes. These remain explicit migration gaps; vector exports use
the shared renderer's existing outlined-font behavior.
