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
  --output evidence/artworks --profile full --motion --exports
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
formats with a selected square size and selected language. Native playback,
interruption, GIF timing inspection, browser execution, CLI invocation and save
dialogs still need their own tests; these checks make no claim to cover them.

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

## Whole-process resources

The sampler launches a program directly, observes descendants and retains children
after reparenting. It records process identity as well as PID to avoid attributing
a reused PID to Creditia. It writes raw samples, logs and a summary.

```sh
python3 packaging/gpui-acceptance/resources.py \
  --output evidence/perf/gpui-idle-cold --fixture evidence/real-credits.json \
  --commit CANDIDATE_SHA --comparison-role candidate \
  --scenario idle --cache cold --styles 1 --artworks 27 \
  --window 1360x880 --dpi 1 --seconds 30 -- /path/to/creditia-gpui
```

For the old app use `--comparison-role baseline --measured-commit BASELINE_SHA`
while keeping `--commit CANDIDATE_SHA` as the target of the comparison. Run both
programs on the same machine with identical data, style/work counts, window, DPI,
scenario and cache preparation. Run startup, scroll, zoom, idle and hidden-animation
in cold and warm conditions. The script does not flush system caches; preparation
must be documented by the driver.

The driver/app receives `CREDITIA_PERF_EVENTS`. It appends JSONL events only after
performing the intended workload, including:

```json
{"event":"scenario-complete","scenario":"scroll","success":true,"operations":40}
```

Without a matching successful completion event the workload is **unverified**, so
the report does not accept it as a matched performance scenario. Include actual
event timing if a driver measures interaction latency; the sampler never derives
latency from CPU usage. Its sums are RSS (shared pages may be counted more than
once), not PSS. Short-lived processes can be missed between samples, making CPU a
lower bound. GPU memory and unmeasured latency stay `null`, never zero.

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

The final gate additionally requires four platform package acceptance records,
ten matched performance scenarios and a detailed review file, supplied with
`--manual-review review.json`. The review records reviewer, timestamp and candidate,
every artwork's `visual`/`motion` verdict, and every named check of all 37 feature
groups. Each verdict links to a concrete `manifest.json#case-key` from this run.
Blanket boolean statements such as “all visuals passed” do not satisfy this gate.
`--require-complete` exits nonzero until every gate is met.

Tooling checks require no renderer dependencies:

```sh
node --test packaging/gpui-acceptance/common.test.mjs
python3 packaging/gpui-acceptance/test_resources.py
```
