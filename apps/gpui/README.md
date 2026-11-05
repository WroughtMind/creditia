# Creditia GPUI desktop

The native desktop reuses `creditia-desktop-service`, the existing Rust analysis
worker and the shared artwork definitions in `packages/render`. Migration scope
and evidence are tracked in `docs/gpui-migration`. Source implementation and a
successful build are separate from completed visual, performance and release
acceptance.

## Run from source

Use Rust 1.92 or later and the [GPUI Kit platform prerequisites](https://gpui-kit.com/docs/installation/).
The Kit baseline is macOS 15+, Windows 10 with MSVC, and Linux with Wayland/X11
and a Vulkan driver. The CI Linux distribution is Ubuntu 24.04. Both Mac
architectures have their own build and package.

```sh
npm ci --prefix packages/render
cargo build -p creditia-cli
CREDITIA_CORE="$PWD/target/debug/creditia-core" cargo run --manifest-path apps/gpui/Cargo.toml
```

On Windows use `creditia-core.exe` and set `CREDITIA_CORE` with the shell's normal
environment syntax. Development needs Node 22.18 or later. CI and packaged
candidates use a fixed Node version recorded in `package-info.json`.

`CREDITIA_NODE` and `CREDITIA_RENDER_BRIDGE` are explicit development overrides.
Release builds discover their own resource directory next to the executable,
or inside `Creditia.app/Contents/Resources` on macOS. An installed application
does not depend on a source checkout, a globally installed Node, or remote fonts.

## Runtime boundaries

- The Rust service owns project libraries, analysis, cancellation, preferences,
  settings and background work; both desktop clients use the same archive and
  evidence formats.
- The shared renderer owns layout, style, artwork data, hover regions and
  animation/export definitions. A persistent preview service reuses assets;
  exports have an independent cancelable execution path.
- GPUI owns window and input behavior, the exhibition wall and roam viewport,
  hit testing, controls and image lifetime. The native preview module rasterizes
  at a scale chosen for the current viewport.
- Legacy WebView preferences are read by the bundled one-time helper under the
  original application identity. Old storage is retained for rollback.

## Verify locally

```sh
cargo test --manifest-path apps/gpui/Cargo.toml --lib --bins --locked
node --test packaging/gpui/package.test.mjs
node docs/gpui-migration/capture.mjs --check
```

Real native pixel tests use the production `Creditia` view, canonical artwork
fixtures, and GPUI's Metal/WGPU offscreen renderer. They wait for actual image
decoding, exercise pointer actions and preserve screenshots and provenance.
They are selected explicitly because AppKit must initialize on the main thread:

```sh
node packaging/gpui/fixtures.mjs --output evidence/fixtures
CREDITIA_VISUAL_FIXTURES="$PWD/evidence/fixtures" \
CREDITIA_VISUAL_OUTPUT="$PWD/evidence/native" \
cargo test --manifest-path apps/gpui/Cargo.toml --features visual-test --test rendering --locked
node packaging/gpui/seal-evidence.mjs evidence/native
```

The default fixture scope covers 26 desktop styles in Chinese and English,
captured with light and dark application themes. Pass `--profile smoke` to the
fixture generator for the four representative styles during development. The
synthetic render fixture is labeled as such in every evidence manifest. Windows
uses the packaged executable in a real window: four language/theme cases wait
for decoded artwork, send keyboard input, assert view state and retain screenshots.
This requires an interactive desktop and a working graphics driver; unavailable
native rendering fails the acceptance gate.

These captures establish rendering and the interactions asserted in the test.
They do not by themselves establish visual equivalence of every offscreen
artwork, all animations, system dialogs, accessibility, or physical-device GPU
behavior. The matrix comparison and process-tree benchmark tools keep those
results separate and record incomplete checks explicitly.

## Packages and release evidence

The `gpui` workflow builds and links four platform targets, executes contracts,
captures native views on Mac/Linux, and produces downloadable candidates:

| Platform | Candidate artifacts |
|---|---|
| macOS Apple Silicon / Intel | `.app` inside DMG and ZIP |
| Windows x64 | Portable ZIP and per-user NSIS installer |
| Linux x64 | Portable tar archive and DEB |

Each package contains the exact analysis executable, Node runtime (official
archive SHA-256 verified), shared renderer and fonts. License files and a file
checksum manifest travel with it. A smoke test runs from a temporary directory
outside the checkout, checks all five graphical export formats, then calls the
native executable's `--check-resources` entry point.
Every archive/installer is then extracted or installed into a fresh directory,
its payload is checked and its installed executables are launched. The Windows
installer's uninstall path is also checked for preserving the user library.

The `gpui-artworks` workflow compares all 1,738 catalog combinations and all
declared preference cases against the pinned original renderer. It also analyzes
the original public Git repository and compares representative artworks from that
real archive. Neither input is relabeled as private agent-log evidence.

`packaging/gpui/INSTALL.md` documents installation and reversible upgrades.
Candidate artifacts are not a published stable release. macOS candidate signing
is ad hoc; Developer ID signing/notarization and Windows signing require the
release credentials. Final cutover requires the complete migration report,
including actual package installation, native interactions and process-tree
performance on the supported systems.
