# Real attribution replay fixtures

## Observational sample: `multi-tool.json`

Captured on 2026-10-29 from this project's archived successful-write evidence and Git history through main `6597e2bbeff466d3d357ee7beb12e717a064db00`. The library snapshot had last analyzed `7f964f226c2c5376ee2e69c8a63a2c6706be953f`; this is not a claim that every current log was ingested.

- Three real, non-web commits: 151 added positions, 234 Claude Code / Codex events and six session metadata records.
- Selection retains every candidate event for the selected line, joined-line and derived short-line keys, across all recorded times. Competing tools are retained. Other commits are omitted, so this is a bounded replay, not a full-project accuracy measurement.
- Each output position records tool/model and evidence level. The baseline contains 50 Codex verified, 35 Claude Code verified, 53 Claude Code declared, eight Claude Code inferred and five unknown positions.
- The golden output is an **observational regression baseline**, not independently labeled ground truth. The synthetic CLI benchmark retains its independent per-line labels and thresholds.

The full private research snapshot replayed 134,825 positions identically before and after sanitization. The committed subset independently replayed all 151 positions identically. Raw snapshots and alias mappings are not committed. The committed input's SHA-256 is `74bb52e23a145097df47dd3779cb27c4748bb55292bdae17b910cbbaedf1d683`.

This fixture covers the Engine's historical attribution only. Separate [real native excerpts](../../../../cli/tests/fixtures/real-native/README.md) now run Codex and Claude Code records through the native readers, CLI, full report and survival paths; they do not extend this 151-position fixture's scope. Removing Codex records in this test is a **controlled evidence-loss experiment**, not a naturally observed cleanup: verified Codex disappears, its full 48-line receipt remains declared, and two competing positions match existing Claude records. This does not prove which of the competing writes is independently correct.

## Fixture contract

The [replay input](../../support/replay.rs) is one JSON object with these five required fields. Use the [core data types](../../../src/model.rs) for nested fields and their defaults. Keep input order for tied commits and for the files, lines and events within a snapshot.

| Field | Value |
|---|---|
| `commits` | Array of serialized `Commit` records for a pinned Git range, including author/commit times, authors, relevant trailers and `FileAdd` records. Preserve file order, addition positions, hunk boundaries, and the keys for long, short and joined lines. |
| `stamps` | Map from commit alias to `[harness, modelOrNull]` for whole-commit declarations. Keep per-commit `lines_stamp` receipts on the commits. |
| `events` | Array of `Ev` line evidence, including the relative file alias, key, source, model, session, time, tab flag, next key and occurrence semantics. Retain all candidate events for selected positions, including competing and repeated records. |
| `sessions` | Array of `Session` metadata, with shifted start/end times and aliased IDs and branches. Set `provider` to `null` or omit it. Set `tokens` and `prompts` to zero. |
| `config` | Serialized [`Config`](../../../src/config.rs) containing the attribution level, aliased `directors` and evidence rules needed by the sample. Clear `repo`, `paths`, `remotes`, `markers`, `excludeMarkers`, `privacy.aliases` and `display.sign`. |

The sibling `*.expected.json` contains `rows`, an ordered array of `[contributor, evidenceLevel]`, and `counts`, totals keyed by `harness-or-kind:Level`. Rows are ordered by commit time, then author time, then input order for ties, followed by each commit's file and added-line order. Agent identities retain their canonical tool/model labels. Human and unattributed identities use replay-local `kind:index` labels. The snapshot records observable output. Record any independent attribution evidence separately.

## Sanitization checklist

- [ ] Use an approved capture. Keep original logs, database files, raw replay inputs, reference annotations and the alias mapping in private storage outside the checkout. Use one private copy per snapshot and record the pinned Git revision, tools, versions, log formats and sample-selection criteria.
- [ ] Use consistent aliases across commits, files, events, sessions and config. Use `file-N.data`, `session-N`, `Person N`, `h:owner-N`, `branch-N` and aliased emails such as `person-N@example.invalid`. Commit aliases are 40 lowercase hex digits starting with at least 32 zeroes. Use a common timestamp offset across related snapshots and preserve the relative windows and unknown-time sentinel `0`.
- [ ] Preserve which author emails refer to the same GitHub account, including the numbered and unnumbered noreply forms. Keep only reviewed public agent/bot emails, trailer syntax, tool/model labels and branch-prefix meanings needed for attribution. Replace commit subjects with `sample commit`.
- [ ] Replace original line fingerprints with opaque equality keys. Preserve zero sentinels, join keys, repeated lines and event `next` links. Recompute derived short-line keys with `creditia_core::hash::short_key` using the aliased path, nearby long-line anchor, direction, ordinal and short-line key. Reject conflicting aliases and new collisions.
- [ ] Remove original code and prompt text, tool arguments/results, secrets, auth headers, local paths, remotes, real names/emails, private hostnames, provider and usage metadata. Review all remaining fields and dictionary keys in inputs, expected output and provenance notes. The test guard checks strings and dictionary keys against exact aliases or reviewed labels and rejects duplicate keys. Review numeric fingerprints and alias mappings manually.
- [ ] Compare the private and sanitized snapshots position by position and evidence level by evidence level. Review the public expected outputs before adding them as regression baselines.

`creditia evidence export` is a private transfer format whose evidence can help assemble a private replay input. A reusable native-log sanitizer and independent real-data labels remain follow-up work for [#121](https://github.com/WroughtMind/creditia/issues/121).

## Onboard a team or naturally pruned sample

1. Make a private working directory from the repository root.

   ```sh
   umask 077
   replay_work=$(mktemp -d "${TMPDIR:-/tmp}/creditia-replay.XXXXXX")
   ```

2. Select an observed case and assemble the private JSON using the fixture contract. Use `creditia_adapters::git::read_history` for the pinned commits and stamps, and the captured reader output for `events` and `sessions`. For a point-in-time evidence export, copy its `evidence` field to `events` and use its `sessions` field. Keep the private attribution config with the sample.

   - **Team**. Select a real repository with distinct Git authors, a local author's evidence-free additions, and a teammate whose additions are credited to that teammate. Include independently reviewed agent edits or commit declarations where present. Record which Git author submitted each selected commit and which lines are supported by edit events or declarations.
   - **Naturally pruned logs**. Use a pair of observed original snapshots around a real log loss or trim. Record how the loss was observed and which receipts remain. Use identical Git commits, file positions, stamps, config and alias tables in both inputs. Capture cold reader evidence at each point from isolated libraries. A library already populated from the earlier snapshot may retain the missing native evidence even when analysis uses `--fresh`. The [native CLI regression](../../../../cli/tests/fixtures/real-native/README.md) covers that warm-library behavior.

3. Sanitize the input using the checklist. Save the private and sanitized versions in `$replay_work`. Run these commands once for `team`, or once each with `sample=pruned-before` and `sample=pruned-after`.

   ```sh
   sample=team
   cargo run -p creditia-core --example replay -- \
     "$replay_work/$sample.private.json" "$replay_work/$sample.private.result.json"
   cargo run -p creditia-core --example replay -- \
     "$replay_work/$sample.sanitized.json" "$replay_work/$sample.expected.json"
   cmp "$replay_work/$sample.private.result.json" "$replay_work/$sample.expected.json"
   ```

4. After equality and privacy review, copy only the sanitized input and reviewed output. Repeat for each snapshot.

   ```sh
   fixture_dir=crates/core/tests/fixtures/real
   cp "$replay_work/$sample.sanitized.json" "$fixture_dir/$sample.json"
   cp "$replay_work/$sample.expected.json" "$fixture_dir/$sample.expected.json"
   ```

5. Wire the files into [the regression tests](../../real_replay.rs) with `include_str!`. Load inputs with `public_input(raw, reviewed_keys, reviewed_labels)` and expected files with `public_json(raw, keys, labels)`. Use explicit reviewed strings for model labels, rule keys and output labels as shown by the existing multi-tool test.

   - **Team**. Assert `replay::replay(&team) == expected`. Add `team.authors.expected.json` keyed by commit alias. For each commit, record the reviewed `name` alias, boolean `me` for local authorship and a `by` object of line counts. Use `author` for lines credited to that commit's author, `other-human` for any other human credit, the canonical `a:<harness>[/<model>]` for agent credit, and `unattributed` for unknown lines. Compare it with `team_authors(&team)`.
   - **Naturally pruned logs**. Assert `replay::replay(&before) == expected_before` and `assert_pruned_replay(&before, &after) == expected_after`. The helper checks unchanged Git positions and declarations, a strict subset of edit occurrences, a verified position that loses verification, and retained evidence for any still-verified contributor. Assert the reviewed fallback on affected positions, such as `declared` when a surviving trailer applies and `unattributed` when independent evidence is absent. Retained competing events can still verify a position.

6. Record the sanitized provenance, sample scope and verification boundary in this README. Run both replay suites.

   ```sh
   cargo test -p creditia-core --test real_replay
   cargo test -p creditia-cli --test real_native_replay
   ```

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
| Real team project with independent contributor truth | Author/line validation exercised with synthetic inputs. Real sample pending. |
| Naturally cleaned native logs with before/after truth | Degradation validation exercised with synthetic inputs and controlled deletion. Real snapshot pair pending. |
| Sanitized native logs through the readers | Bounded Codex / Claude Code excerpts included separately; other formats remain uncovered |
| Full report and survival replay equivalence | Verified for the two native excerpts; not for this full 151-position Engine fixture |

Do not close #121 or label synthetic benchmark percentages as real-world accuracy based on this batch.
