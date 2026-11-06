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
architectures have their own build and package. Windows execution tests use the
Windows Server 2022 runner; the declared Windows 10 minimum is not a claim of
automated execution on Windows 10.

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
captured with light and dark application themes. Empty, loading, failure,
analysis-progress and export-progress states are also captured in both languages
and themes. The input case opens an artwork and changes its actual layout.
CI uses `--profile smoke`: five style/language/theme views and one sample of each transient state. The full combinations remain available for targeted investigation. The
synthetic render fixture is labeled as such in every evidence manifest. Windows
uses the packaged executable in a real window: four language/theme cases wait
for decoded artwork, send keyboard input, assert view state and retain screenshots.
This requires an interactive desktop and a working graphics driver; unavailable
native rendering fails the acceptance gate.

The separate native input suite starts the production benchmark view from raw
archive data and uses the actual shared rendering worker. Pointer and keyboard
actions verify layout cycling, interrupted selections, focused style trials and
restoration, grouping, contributor names, language, theme, transformed hover,
roam navigation and the export dock. It supplies no precomputed artwork and
makes no claim about filesystem persistence or project-service behavior:

```sh
CREDITIA_VISUAL_FIXTURES="$PWD/evidence/fixtures" \
CREDITIA_RUNTIME_OUTPUT="$PWD/evidence/native-input" \
cargo test --manifest-path apps/gpui/Cargo.toml --features visual-test --test rendering --locked
node packaging/gpui/seal-evidence.mjs evidence/native-input
```

The service-backed suite starts `Creditia::new` with one isolated project library
for its entire process. It types into native controls, opens real Git archives,
writes settings, closes and reopens the window, and checks removal against the
actual SQLite evidence, snapshot, cache and repository files. Run the same harness
in a separate process with `CREDITIA_SERVICE_OUTPUT` and `CREDITIA_CORE` pointing
to the built analysis executable. Its input archives and results are retained in
`evidence/native-services`; it does not claim to exercise an OS folder dialog.

Linux additionally exercises the packaged streaming player through actual X11
input at native scale 2. It checks pause, replay, zoom and pan while paused,
visibility suspension, decoded frame density, bounded texture chunks and whole
process samples. The manifest and OS screenshots retain the observed state;
this Linux gate does not imply equivalent GPU execution on other platforms.

A separate Linux catalog job selects all 1,482 desktop artwork/layout rows through
those production controls, waits for native image decoding, checks dimensions,
visible bounds and hit-region counts, and captures every focused artwork. Its
contact sheets contain actual native pixels. Every full-resolution window PNG is
also retained in `captures.tar.gz`; the manifest records archive and individual
screenshot hashes for independent review. The source fixture includes profile and blame
contexts; empty outputs fail this gate.

```sh
node packaging/gpui/catalog-fixtures.mjs --output evidence/catalog-inputs
CREDITIA_NATIVE_CATALOG="$PWD/evidence/catalog-inputs" \
CREDITIA_NATIVE_CATALOG_CAPTURE="$PWD/evidence/catalog-captures" \
cargo test --manifest-path apps/gpui/Cargo.toml --features visual-test --test rendering --locked
node packaging/gpui/catalog-evidence.mjs --captures evidence/catalog-captures --output evidence/native-catalog
```

These captures establish rendering and the interactions asserted in the tests.
They do not by themselves establish animation behavior, system dialogs,
accessibility, or physical-device GPU behavior. The matrix comparison and process-tree benchmark tools keep those
results separate and record incomplete checks explicitly.

## Packages and release evidence

The `gpui` workflow builds and links four platform targets, executes contracts,
checks the installed window on each target and produces downloadable candidates:

| Platform | Candidate artifacts |
|---|---|
| macOS Apple Silicon / Intel | `.app` inside DMG and ZIP |
| Windows x64 | Portable ZIP and per-user NSIS installer |
| Linux x64 | Portable tar archive and DEB |

Each package contains the exact analysis executable, Node runtime (official
archive SHA-256 verified), shared renderer and fonts. License files and a file
checksum manifest travel with it. A smoke test runs from a temporary directory
outside the checkout, checks all five graphical export formats, then calls the
native executable's `--check-resources` entry point. On Linux the installed desktop also analyzes new Git changes, cancels the running process, and retries against the same on-disk project. The DPI2 player is observed across multiple authored playback cycles.
Both Mac architectures install the same verified Icon Composer catalog from
macOS 26/Xcode 26.6, including the original static icon for macOS 15.
Windows PE imports are recorded. Required Visual C++ release runtime DLLs are
copied unmodified from the builder's redistributable directory beside the
relevant executables, with Microsoft's runtime license and provenance retained.
Every archive/installer is then extracted or installed into a fresh directory
and its sealed payload and resource entry points are checked. Each Mac/Linux
format also launches the actual installed application with raw archive data,
waits for its bundled renderer to paint decoded artwork, retains an OS window
capture and verifies normal quit. The Windows
installer's uninstall path is also checked for preserving the user library.

The `gpui-artworks` workflow compares all 1,738 catalog combinations and all
declared preference cases against the pinned original renderer. It also analyzes
the original public Git repository and compares representative artworks from that
real archive. Neither input is relabeled as private agent-log evidence.

`packaging/gpui/INSTALL.md` documents installation and reversible upgrades.
The gated `gpui-release` workflow can create a draft prerelease from already
validated artifacts without rebuilding them. An explicit branch request or
manual run supplies the immutable candidate, successful evidence runs and
recorded review; the combined acceptance gate must pass first. Requests default
to disabled. See `packaging/gpui/RELEASE.md` for the exact procedure.
Candidate artifacts are not a published stable release. They follow the existing
distribution policy: macOS signatures are ad hoc, and Windows binaries are
unsigned. The installation guide documents the corresponding OS trust prompts.
Developer ID/notarization and Windows code signing can be configured separately;
their absence does not prevent generating or installing these packages.
Final cutover requires the complete migration report,
including actual package installation, native interactions and process-tree
performance on the supported systems.
