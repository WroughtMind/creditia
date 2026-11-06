# GPUI acceptance evidence

These tools record results. They do not turn source inventory or a successful
compile into a completed desktop migration. All generated evidence belongs to a
full candidate commit, fixed inputs, exact fonts and the machine that ran it.

## Rendering matrix

Prepare two separate clean Git checkouts. The baseline must be the commit in
`docs/gpui-migration/inventory.json` (`ac82a574…`); install each checkout's locked
`packages/render` dependencies with `npm ci`. Use the same Node release. This is
deliberately separate from comparing two APIs in the candidate checkout.

```sh
node packaging/gpui-acceptance/render-matrix.mjs \
  --baseline-root ../creditia-baseline --candidate-root . \
  --output evidence/artworks --profile full --preferences all --motion --exports
```

`full` covers all 1,738 distinct renderer/style/piece/layout rows, including the
1,482 desktop-visible rows and the extra shared registrations. Hidden SVG aliases
use the explicit `renderer: "svg"` option, so a classic theme with the same ID
cannot accidentally replace them. `profile` and `blame` receive explicitly
synthetic presentation contexts instead of silently skipping those works.

For quicker feedback, `--profile smoke` selects the default tier, asterisk,
credits and code map across the styles, plus scrolling variants and contextual
works. `--shards 8 --shard 0` partitions the selected rows without dropping them;
run every shard before interpreting full coverage. Four language/group/name
combinations rotate through rows by default; `--preferences all` runs each on
every row. `--stress` adds a long Chinese-name fixture and an empty fixture.

The runner compares width, height, visible data, title, every hover hit, normalized
SVG and RGBA pixels. Only generated SVG namespace prefixes are normalized. New
selection metadata is recorded by the renderer but does not count as a visual
change. Font-file hashes must be identical before any artwork comparison starts.

`--motion` checks that explicit time samples match frame results and that reduced
motion returns the final frame. `--exports` checks all applicable graphical
formats against a declared 320×240 transformed artwork, including decoded pixel
content and GIF timing. Native playback, interruption, browser execution, CLI
invocation and save dialogs still need their own tests.

Each case is checkpointed to `manifest.json`. A failed or interrupted run preserves
its partial evidence; only a finished run has `complete: true`. Matched thumbnails
share one file when the PNG bytes are identical. Different cases retain both
images and full SVGs; `--save-all-svg` retains full SVGs for every case. `index.html`
is a searchable comparison gallery with lazy-loaded images. Input archives are
not copied into evidence by default.

### Real and synthetic inputs

The default `catnap.json` is **synthetic**. The committed real attribution replay
fixture begins at decoded engine inputs and is not a complete visual archive.
Do not relabel it, or a modified catnap fixture, as a real project sample.

A CI checkout can generate a real repository archive using the actual analysis
core and public Git history:

```sh
creditia-core analyze --repo . --out evidence/real-credits.json --json
node packaging/gpui-acceptance/render-matrix.mjs \
  --baseline-root ../creditia-baseline --candidate-root . \
  --fixture evidence/real-credits.json --fixture-kind real_public_git_archive \
  --output evidence/real-artworks --profile smoke
```

Record the analyzed repository commit and core build separately. Without native
agent logs this archive demonstrates public Git data, not rich real-agent-log
coverage. Derived stress fixtures always retain a synthetic label even when their
source archive was real. No private logs, prompt text or machine paths should be
committed as public visual fixtures.

## Export checks

The full matrix compares all four language/group/name preferences. It runs
motion and all graphical encoders once per row using a rotated representative
preference, a declared 320×240 canvas and 160-pixel GIF width. Half the rows request
transparency. This controls cost without omitting any row or format.

PNG and GIF are independently decoded. PNG pixels and PDF RGB/alpha streams must
match the expected transformed artwork exactly. PDF cross-reference offsets,
page dimensions and linked image objects are validated. SVG and HTML are
rasterized and compared; HTML also checks transformed hover areas, selected
language, offline content and every exported motion sample against the shared
timeline. GIF delays and alpha masks must agree; RGB quantization has an explicit
maximum mean error of 12/255 per channel, with actual mean and maximum recorded.
Failed encoded files and reference SVGs are retained in addition to hashes.

Static old/new pixel comparison decodes actual PNG bytes on both sides, so an
internal change from premultiplied to straight alpha cannot masquerade as a
visible artwork regression. Encoder checks are independent of CLI invocation,
browser rendering and native save dialogs, which have their own acceptance tests.

## Native window evidence

The native UI tests render the production view, exercise input and write their
own manifest. Use this shape (all artifact paths are relative to the manifest):

```json
{
  "schema_version": 1,
  "kind": "native-ui",
  "complete": true,
  "provenance": {
    "candidate_sha": "FULL_GIT_COMMIT",
    "environment": { "platform": "linux", "arch": "x64", "dpi": 1 },
    "fixture": { "path": "fixture.json", "sha256": "INPUT_SHA256", "kind": "synthetic_render_sample" }
  },
  "cases": [{
    "key": "zh-light-wall",
    "status": "passed",
    "checks": [{ "id": "hover-after-zoom", "passed": true }],
    "artifacts": [{ "path": "zh-light-wall.png", "sha256": "IMAGE_SHA256", "bytes": 123 }]
  }]
}
```

An existence check on a PNG is not enough: tests must assert the actual view,
selected artwork, interaction result and layout. Screenshot review compares
Chinese/English, light/dark, DPI, native font appearance and empty/error states.
Platform-specific dialogs and OS integration require native installed-app runs.

The installed Linux DPI2 animation driver is sealed separately:

```sh
node packaging/gpui-acceptance/native-animation-evidence.mjs \
  --input evidence/native-animation/animation-evidence.json \
  --candidate-sha "$GITHUB_SHA" --output evidence/native-animation/sealed
```

This retains real OS actions, decoder epochs, bounded frame chunks and screenshots
for one `minimal-light/share` streaming path. Its cadence data measures decoded
frame selection before compositor presentation, with separate sampled OS pixels.
It does not imply native animation coverage of every style or a sharpness review.

## Behavioral feature evidence

The checklist tracks implementation and checked behavior independently. Collect
actual Rust/Node test outcomes using inspected assertion mappings:

```sh
node packaging/gpui-acceptance/feature-evidence.mjs \
  --logs evidence/test-logs --bindings packaging/gpui-acceptance/feature-bindings.json \
  --candidate-sha "$GITHUB_SHA" --output evidence/feature-checks
```

`test-logs/provenance.json` identifies the candidate and each log's file, run,
job ID or job name, and platform. Each command log starts with the actual
`git rev-parse HEAD`; decoded GitHub job logs already contain the checkout commit.
The collector reads explicit pass/fail/ignored outcomes. Merely finding a test
name, compiling a test, or having a green job does not verify a checklist item.
The report can combine required test outcomes across valid same-candidate suites.
Any failed required test remains failed, and missing tests remain unverified.

Native input and browser cases can include `feature_checks` with the exact
`feature_id` and `check` from `features.json`, a `test_id`, a concrete `behavior`
description, and `passed`. Such claims inherit their case artifacts and count only
after the complete manifest, candidate and artifacts validate. A passed claim in
an unsuccessful case is rejected. Backend tests establish only their stated
backend behavior; they do not establish that a menu or dialog invokes it.

## Whole-process resources

The sampler launches a program directly, observes descendants and retains children
after reparenting. It records process identity as well as PID to avoid attributing
a reused PID to Creditia. It writes raw samples, logs and a summary.

The `gpui-performance.yml` job builds the original Tauri app at the immutable
baseline and the GPUI candidate, then runs both on one Xvfb/software Vulkan host.
`instrument-tauri.mjs` records hashes before and after adding fixture access and
passive state observation to the old build. Original rendering, layout and input
handlers remain in use. Background collection and project writes are disabled
for both benchmark constructors. The release app does not include the baseline
overlay.

`public-fixture.mjs` analyzes the public pinned Git checkout on a fresh hosted
runner before instrumentation. It verifies zero local-log sessions, tokens and
written-line evidence; otherwise it deletes the archive and fails. This is a real
Git project fixture, not evidence that private agent logs have been verified.

```sh
dbus-run-session -- xvfb-run -a -s '-screen 0 1600x1000x24' \
  python3 packaging/gpui-acceptance/x11-session.py -- \
  python3 packaging/gpui-acceptance/compare.py \
    --output evidence/performance --fixture evidence/public-creditia.json \
    --baseline-bin /path/to/original/creditia-app \
    --candidate-bin /path/to/current/creditia-gpui \
    --baseline-sha BASELINE_SHA --candidate-sha CANDIDATE_SHA \
    --baseline-overlay evidence/baseline-overlay.json
```

The driver sends actual X11 wheel, keyboard and pointer input and waits for
observed production UI state changes. It captures real window screenshots. App
state events and the driver's hash-chained action records are retained alongside
raw process samples. A lone `scenario-complete` claim without the complete valid
chain is rejected. Hashes provide artifact integrity, not a security attestation
against a malicious application or runner.

The common scenarios are startup, wall scrolling, roam panning, focus enlargement,
visible idle and hidden idle, each with cold/warm app profiles. The original app
has no continuous zoom or artwork playback, so `zoom` explicitly means opening
and closing its enlarged focus view. GPUI's continuous zoom is an additional
candidate-only scenario; it must not be presented as a matched old capability.
Cold means a fresh isolated app profile; the OS page cache is uncontrolled. Warm
means the same profile after a successful first-viewport launch, with process
memory reset. Warmup action evidence is retained.

Both measured process groups include the same driver and its X11 input utilities.
Startup and idle phase summaries are separate from overall resource summaries.
Observation latency includes driver polling and is not input-to-photon latency.
RSS sums may count shared pages more than once; they are not PSS. Short-lived
children can be missed between samples, making observed CPU a lower bound. GPU
memory and unmeasured interaction latency stay `null`. The sampler refuses hosts
whose process IDs and `/proc` use different namespaces. No local values from
such an executor are accepted.

After all 24 measurements finish, seal their behavioral evidence:

```sh
python3 packaging/gpui-acceptance/performance-evidence.py \
  --input evidence/performance --candidate-sha "$GITHUB_SHA" \
  --output evidence/performance-features
```

This rechecks the 12 paired identities, original action chains, captured image
hashes and raw per-process RSS/CPU arithmetic. It maps only the measured scope to
the checklist. The combined idle-CPU/hidden-animation item remains incomplete
until the independent installed animation assertion is also present. GPU
telemetry stays explicitly unmeasured; it is outside the original RSS/CPU
comparison requirement.

## Consolidation and review

```sh
node packaging/gpui-acceptance/report.mjs \
  --evidence evidence --candidate-sha CANDIDATE_SHA --output evidence/report
```

The report validates artifact checksums, candidate identity and completion before
counting evidence. It reports every missing matrix row and does not modify the
source inventory's acceptance columns. Empty-only renderings remain unresolved.
Performance results are comparable only when machine/data/workload identities
match and both workloads actually completed.

Delivery requires the four installed package workflows, the primary-platform user journey,
five representative visual cases and same-host startup, scroll, roam and hidden-idle comparisons.
Each reviewer entry records its observation and references verified evidence for this candidate.
See [the review format](../gpui/RELEASE.md). The complete artwork and feature tables remain available
for investigation. Full native, web, artwork and cold/warm matrices run on demand.
`--require-complete` exits nonzero until the delivery checks pass.

Tooling checks require no renderer dependencies:

```sh
node --test packaging/gpui-acceptance/*.test.mjs
python3 -m unittest discover -s packaging/gpui-acceptance -p 'test_*.py'
```

Baseline equivalence is limited to two explicitly recorded rules: known legacy
badge font stacks pinned to their existing bundled fallback, and terminal rain
glyphs lying more than 20px below the canvas (a resvg nonintersecting-text abort in
the original). Raw SVG hashes and differences remain recorded. These cases must
match data/hit regions and exact decoded pixels at native 1x and 2x sizes. The latter
rule records that the original raster needed an accommodation; it does not claim
the original crashing raster succeeded. Other geometry or XML differences fail.

For CI, use `matrix-batches.mjs` with the same options as `render-matrix.mjs`.
It expands each configured shard into eight disjoint, sequential child processes
(default `--batches 8`). Every row and preference remains covered, while exiting
each child releases resvg native font allocations. Each batch has its own complete
manifest and gallery; `orchestration.json` records exit codes and case counts.
An interrupted or failed batch keeps the overall run failed.

After the production browser run, seal its exhaustive catalog evidence with:

```sh
node packaging/gpui-acceptance/browser-evidence.mjs \
  --browser-report packages/embed/browser-evidence/report.json \
  --catalog-root packages/embed/dist/acceptance \
  --candidate-sha "$GITHUB_SHA" --output evidence/browser-catalog
```

The wrapper checks all catalog, source and bundle hashes and refuses a partial,
failed or differently tagged run. The tested GitHub merge commit remains explicit;
PR-head and merge-commit identities are not silently interchanged.
