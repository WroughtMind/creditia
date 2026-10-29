# Real attribution replay fixtures

## Observational sample: `multi-tool.json`

Captured on 2026-10-29 from this project's archived successful-write evidence and Git history through main `6597e2bbeff466d3d357ee7beb12e717a064db00`. The library snapshot had last analyzed `7f964f226c2c5376ee2e69c8a63a2c6706be953f`; this is not a claim that every current log was ingested.

- Three real, non-web commits: 151 added positions, 234 Claude Code / Codex events and six session metadata records.
- Selection retains every candidate event for the selected line, joined-line and derived short-line keys, across all recorded times. Competing tools are retained. Other commits are omitted, so this is a bounded replay, not a full-project accuracy measurement.
- Each output position records tool/model and evidence level. The baseline contains 50 Codex verified, 35 Claude Code verified, 53 Claude Code declared, eight Claude Code inferred and five unknown positions.
- The golden output is an **observational regression baseline**, not independently labeled ground truth. The synthetic CLI benchmark retains its independent per-line labels and thresholds.

The full private research snapshot replayed 134,825 positions identically before and after sanitization. The committed subset independently replayed all 151 positions identically. Raw snapshots and alias mappings are not committed. The committed input's SHA-256 is `74bb52e23a145097df47dd3779cb27c4748bb55292bdae17b910cbbaedf1d683`.

## Sanitization contract

Paths, commits, people and session identifiers are aliases; timestamps share one offset so relative windows remain unchanged. Known tool/model labels remain explicit. Provider, usage and prompt counts are removed; project paths, remotes, markers and display metadata are cleared. There is no source code or prompt text.

Original line fingerprints are replaced by opaque equality keys. Renaming a path alone is insufficient: a short line's derived key depends on its path, nearby long-line anchor, direction, ordinal and short-line hash. Recompute those keys with `creditia_core::hash::short_key`, preserving event `next` relationships and rejecting conflicting aliases or new collisions. Known tool emails and GitHub account alias equivalence must retain their attribution semantics. Re-run the same engine on private and sanitized inputs and compare every output position and level before accepting a new fixture. Review all remaining strings, including dictionary keys. The current fixture has a CI allowlist for reviewed aliases/labels; it is not a general-purpose anonymization guarantee for arbitrary future exports.

`creditia evidence export` is a private transfer format, **not** a public-safe fixture exporter. No production exporter behavior changes here. A reusable native-log sanitizer and independent real-data labels remain follow-up work for #121.

```sh
cargo run -p creditia-core --example replay -- private-input.json private-result.json
cargo run -p creditia-core --example replay -- sanitized-input.json sanitized-result.json
cmp private-result.json sanitized-result.json
cargo test -p creditia-core --test real_replay
```

This fixture covers the Engine's historical attribution only. Separate [real native excerpts](../../../../cli/tests/fixtures/real-native/README.md) now run Codex and Claude Code records through the native readers, CLI, full report and survival paths; they do not extend this 151-position fixture's scope. Removing Codex records in this test is a **controlled evidence-loss experiment**, not a naturally observed cleanup: verified Codex disappears, its full 48-line receipt remains declared, and two competing positions match existing Claude records. This does not prove which of the competing writes is independently correct.

## Public squash provenance samples

`crates/adapters/tests/fixtures/squash/pr124.json` and `pr125.json` come from:

| PR | Squash commit | Original head | Safely positioned declarations |
|---|---|---|---|
| [#124](https://github.com/WroughtMind/creditia/pull/124) | `5deeba90be16873e21143a322d0285a96d4e137b` | `a6c518c891c54a3de0390b670fb18eed8665e15e` | 262 / 428 |
| [#125](https://github.com/WroughtMind/creditia/pull/125) | `6597e2bbeff466d3d357ee7beb12e717a064db00` | `8a0ad5487cbe297e1d4874561f6a886609705c8e` | 533 / 609 |

Origins were captured with Git's incremental blame on the pinned original head, after matching final file blobs. Paths and identities are aliases; source fingerprints are replaced with placeholders because these fixtures test declaration projection, not native-event matching. Public Git commit IDs and actual positions remain. Full or unambiguous single-source declarations are projected; mixed count-only receipts remain unknown. These results are **declared**, never verified, and original commits are not added to main's totals.

The `squash_replay` adapter example captures raw provenance locally; sanitize and inspect its output before adding it to Git. `cargo test -p creditia-adapters --test squash` also constructs real temporary Git branches to test overwrite, delete, rename, repeated lines, conflict changes, ordinary merges and offline cache reuse.

## Coverage still needed for #121

| Coverage | State |
|---|---|
| Real project using multiple tools | Bounded observational fixture included |
| Real team project with independent contributor truth | Missing |
| Naturally cleaned native logs with before/after truth | Missing; controlled deletion test only |
| Sanitized native logs through the readers | Bounded Codex / Claude Code excerpts included separately; other formats remain uncovered |
| Full report and survival replay equivalence | Verified for the two native excerpts; not for this full 151-position Engine fixture |

Do not close #121 or label synthetic benchmark percentages as real-world accuracy based on this batch.
