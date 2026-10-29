# Real native excerpts through the CLI

These bounded fixtures start at actual Codex and Claude Code records, before
the native readers. `real_native_replay.rs` runs the real CLI with an isolated
profile and evidence library, comparing the complete `credits.json`, historical
line attribution and surviving-line attribution with `expected.json`.

## Sources and scope

Captured locally on 2026-10-29 from the same archived sessions used to investigate
#121. Each case retains one successful write and the native records needed to
interpret it: Codex's session header, model context and completed `FileChange`;
Claude's `Write` call and matching successful tool result. Other conversation
records, prompts and outputs are omitted. This is a selected excerpt, not a
whole-session or whole-project accuracy claim.

The two versions of each target file come from these public Git commits:

| Case | Initial version | Later version |
|---|---|---|
| Codex | `f42cfbc5106a6a0c61b624f707e8757e12de9d93` | `cd8e41c3fd8d45f59331f7a27c64f7e4be272dab` |
| Claude Code | `e5e95a1999f4abe17d7a4869be3dcb5fbeb2f285` | `9fcb65d36fcf35c099e7295f700528347a4c9738` |

In both cases the native write body exactly matched the initial Git file. The
test rebuilds a two-commit, one-file repository from those versions. Original
commit messages, unrelated files, PR topology and source declarations are not
included. Authors, paths, session/message/call IDs and commit subjects are
aliases; dates share a case-local shift by whole weeks. Extension, file area,
relative receipt/commit timing and weekdays are retained.

## Content redaction and verification

Every original content line is replaced. Lines with the same Creditia-normalized
value receive the same opaque token. Short lines remain short; blank lines stay
blank. Whitespace variants preserve exact-line distinctions for Git diff without
changing normalized equality. Code and prompt text are absent. The input's JSON
keys, labels, timestamps and opaque content are allowlisted by a test.

Before committing, each selected excerpt and its two file versions ran twice in
isolated repositories: once with the original bodies kept privately, once with
opaque bodies. The metadata aliases were identical in both runs. The entire
report and every historical/surviving attribution row matched after removing
only `generatedAt` and replacing generated Git commit IDs. This checks content
redaction for these excerpts; it is not a general-purpose sanitizer guarantee.

| Case | Added / verified / unknown | Surviving / verified / unknown |
|---|---|---|
| Codex | 31 / 30 / 1 | 30 / 29 / 1 |
| Claude Code | 20 / 13 / 7 | 18 / 11 / 7 |

The golden files are observational regression baselines, not independently
labeled real-world accuracy. Later additions without a matching included write
stay unknown.

## Regressions

```sh
cargo test -p creditia-cli --test real_native_replay
```

- A positive ingestion assertion prevents an empty reader from passing merely
  because both outputs contain only unknown lines.
- Removing only the temporary native file, then analyzing with fresh scan
  caches, must reproduce the full report from the persisted evidence library.
- Changing the success receipt to failure in a fresh library must produce no
  verified lines. This is a controlled negative variant.

Controlled removal is not an observed natural cleanup. Team evidence and a
naturally cleaned-log case remain missing for #121. These two native formats do
not establish equivalent coverage for all other tools, and no production export
format or general native-log sanitizer is added here.
