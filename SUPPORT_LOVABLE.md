# Lovable trajectory reader: verified execution-to-rendering chain

Checked 2026-10-17. This continues the public-client investigation in `SUPPORT_CLOUD_NATIVE.md`. Only publicly referenced application scripts, official documentation and public GitHub pages were read. No account login, authenticated history request, model invocation or downloaded application execution was performed.

## What the official current client directly establishes

The [trajectory client](https://lovable.dev/assets/app-lazy-shared-ClOWhgKT.js) requests `/projects/{id}/trajectory?filter=frontend&inclusive=true&chain=view&head_id=...` (also other resource scopes and paginated `before` requests). The response is `{events:[EventWrapper...]}`. Each original event is parsed using protobuf `fromJson`, so JSON oneofs use their named members and bytes use base64 strings. `EventWrapper` has `id:{value}`, `parent:{value}`, `timestamp`, `payload` and optional `copiedFrom`.

The [descriptor/reducer](https://lovable.dev/assets/dashboard-shared-2rv9s-Mi.js), including its embedded `trajectory/events.proto`, establishes:

- `agentOutputToolCall` carries `toolId`, `toolName`, `toolUseId` and serialized `params`. The event's native `id` is the reference used by subsequent `toolCall:{value}` fields; the provider's `toolUseId` is a separate identity.
- `toolParametersValidated` carries the referenced call and validated JSON-string `params`. Approval, execution start and execution end are separate events.
- `toolExecutionEnd` has exactly one result branch: `hardError`, `softError`, or `output`. The successful branch contains `changes`, `diffs` and other output fields.
- For `output.diffs[]`, the reducer decodes `content.inlineData` as UTF-8 and calls `JSON.parse`; it retains the parsed value as `{filePath,hunks}`. It keeps blob references separately. It does **not** interpret those bytes as a unified-diff string.
- `agentOutputEnd.model.modelName` is a per-inference actual-model field. A project selection, event timestamp, final model elsewhere in the session or an unrelated inference does not establish a particular call's model.
- The native `Change` variants `addition`, `edit` and `spliceEdit` identify paths. Their protobuf descriptors also declare `content:InlineOrRef`; `spliceEdit` has `start`/`end`. The current reducer discards this content when reducing changes. **Its exact content encoding and its before/after interpretation have not been established, so the reader does not decode it.** This corrects the earlier over-narrow statement that Change only declares paths.

The [tool registry](https://lovable.dev/assets/registry-CMXIDPt4.js) identifies `lov-write`, `lov-search-replace`, `lov-line-replace` and `lov-apply-patch` as editing tools with diff content and a completion callback; their declared aliases are respectively `write`, `search_replace`, `line_replace` and `apply_patch`. The first three expose `params.file_path`; apply-patch exposes `params.patch`. Read, shell, preview, code-review and arbitrary tool names are not included in the reader.

The [official HunkDiffViewer](https://lovable.dev/assets/HunkDiffViewer-Cg0PYYre.js), functions `ef` and `i_`, converts the following representation into a unified patch:

```json
[
  {
    "oldStart": 1, "oldCount": 2,
    "newStart": 1, "newCount": 2,
    "lines": [
      {"type":"context", "content":"unchanged"},
      {"type":"remove", "content":"old_line"},
      {"type":"add", "content":"new_line"}
    ]
  }
]
```

Its `ef` maps add to `+`, remove to `-` and other rows to context. The [official ActivityView](https://lovable.dev/assets/ActivityView-DfoLjBcY.js) independently uses the same fields and the explicit `{add:"+",remove:"-",context:" "}` mapping for the real commit-diff API response's `hunks`. The reader accepts only those three known row types, single-line content, nonnegative integer coordinates, nonoverlapping hunk ranges and exact old/new line counts; all required fields must be present with no additional fields. Each hunk's removed and added rows are passed through the shared `added_lines(old,new)` occurrence subtraction: a human-authored line re-emitted as both remove and add does not become new AI authorship. Only the additional occurrences supply evidence; context rows never do.

## Direct execution-to-rendering chain

Continuing five dependency levels beyond the initial project/chat modules found the missing consumer in [ThinkingTimelineItem](https://lovable.dev/assets/ThinkingTimelineItem-89gfR0Am.js). The exact shipping chain is now closed:

1. `dashboard-shared` function `CA` decodes `FileDiff.content.inlineData` with TextDecoder then JSON.parse, retaining `{filePath,hunks:parsedJSON}`. Its successful-tool reducer stores this array on the tool call as `diffs`.
2. `ThinkingTimelineItem` function `an` iterates those exact tool calls and executes `a.diffs.map(f=>({file_path:f.filePath,hunks:f.hunks}))`, keyed by the native provider `tool_use_id`. It does not reconstruct hunks from tool parameters or the current project files.
3. [VerboseActivityTimeline](https://lovable.dev/assets/VerboseActivityTimeline-B3n_E5rY.js) imports `an` as `I`, calls it with its actual tool calls, looks up `r[n.toolCall.tool_use_id]`, and passes that array into its `W` row, which calls the imported `Nt` (`O`) with those diffs.
4. `ThinkingTimelineItem.Nt` handles `content.type === "diff"` for completed tools, passes that array to `gt`, and `gt` renders `et({hunks:t.hunks,filePath:t.file_path})`.
5. `et` is the imported `r` export from `HunkDiffViewer-Cg0PYYre.js`, the actual `M_` component. Its `i_` consumes the coordinates and typed lines described above.

This is direct current official source evidence, not a guessed schema based only on the unrelated commit/code-review viewers. The initial generic-tool renderer investigation was insufficient because the verbose timeline is a deeper lazy dependency. The investigation read 4,064 nonempty public modules (approximately 60 MB); only relevant resource identities are recorded below.

`crates/adapters/src/lovable.rs` reads a successful native trajectory diff **only when its decoded JSON matches that actual consumed `Hunk[]` representation in full**. Different representations, extra/unknown fields, malformed counts, whole-file text, blob references and too-large bodies are rejected. No fallback guesses `newLines`, `added`, unified diff, or resulting whole-file content. The backend serializer itself was not published in the inspected graph, but the complete client decode/consume path establishes the representation's semantics. Real-account codec prevalence and acceptance remain separate and unverified.

`an` also shows that blob references can be resolved by `GET /projects/{project_id}/trajectory/tools/{event_id}/diff`; it expects `tool_diffs[tool_use_id]` with `file_path`/`hunks`. That response alone lacks the call's success, actual model and execution timestamp. This implementation does not make authenticated requests or treat an unbound saved blob-response body as sufficient provenance.

## Original HAR project-to-repository mapping for relative paths

Typical Lovable paths are repository-relative, so an absolute-path-only fixture does not establish practical import coverage. The current official [route queries](https://lovable.dev/assets/route-shared-leaves-Hz8d0PVl.js), function `us`, request `GET /workspaces/{workspace_id}/projects/{project_id}/gitsync`. The [Git settings UI](https://lovable.dev/assets/ProjectGitSettingsTab-C7jFV406.js) consumes its returned `synced`, `config.repo_url`, `config.owner_name`, `config.repo_name`, `config.provider_type`, and `config.connection_id`; it uses that `repo_url` for the connected repository and clone commands. The [shared API client](https://lovable.dev/assets/app-shared-ljm5BCJR.js), function `Fv`, sets the production API base to `https://api.lovable.dev`.

The reader now accepts original HAR 1.2 `log.entries` records containing successful GET responses from that exact API origin. It binds:

1. `/projects/P/trajectory?chain=view&filter=frontend&inclusive=true&...` → those original EventWrapper IDs belong to native project `P`.
2. `/workspaces/W/projects/P/gitsync` → a returned `synced:true` and `config.repo_url` identify that same project's connected repository.
3. The repository URL must pass `PathMap.in_project(remote)`. For a relative write, every event from the result back to its call must have that same original project scope. The full relative path must be safe, present in the project's path set, and map to exactly itself after applying normal project exclusions.

The path is then used as repository-relative evidence; no local cwd is invented. Repository observations that disagree, are unsynced, or lack a valid repo URL make that project's relative mapping unusable. Cross-project event-ID collisions, another project's sync response, an unrelated host, non-GET/error responses, absent frontend/view flags, traversal, or a matching filename without this context do not establish mapping. HAR response `content.text` supports its standard absent encoding or `encoding:"base64"` form. Arbitrary request headers, cookies and credentials are neither needed nor inspected. Standalone unscoped trajectory response bodies still support only absolute paths.

The reader uses the current explicit Git-sync binding; it does not infer a current repository from a project name, `creation_options.source_repo_url` (an import source), legacy default GitHub organization, a template or a discovered local filename. `discover()` can report the confirmed remote for a HAR-bound project while leaving cwd empty and discovery time unknown.

Additional gates:

- The referenced call must be a known write tool with validated parameters on the same parent chain. Its requested path, exactly one actual output change path and exactly one inline diff path must agree.
- Success is the original output branch, not a done/status label, approval, parameters, proposed review fix, current file snapshot or git commit. Hard/soft failures and rejection observations for the same native call veto success.
- Absolute paths use the normal `PathMap` boundary. Relative paths require the original HAR mapping above; unscoped bodies cannot borrow a repository based on import-directory location or a matching filename. A Lovable project ID is not treated as a filesystem cwd.
- The model comes only from exactly one `agentOutputEnd` between that call and its result, with an `agentOutputStart` on the call's same inference ancestry and no intervening new inference/run/iteration/user message. Missing or ambiguous ancestry leaves the model empty. The tool result's own timestamp is used; missing time stays zero/unknown.
- Repeated original native event IDs merge across files. Conflicting copies and multiple completion identities for a single call are rejected. Distinct call IDs with the same added line retain distinct occurrences. Identical text is never the deduplication key.
- Copied/fork ancestry and cyclic parent chains are rejected. Because an unscoped raw history body can omit the selected head or part of the replaced branch, any observed revert makes the combined raw import set unusable for line evidence. This is conservative and may suppress unrelated captures in that directory; no stale branch is called a current write.
- The import directory is rebuilt into one `lovable:native-import:` group on every scan. All original observations retain project-bound session identity before graph conflict resolution; altered paths, tools or parent links cannot hide the session that must retract old evidence. Original HAR owner and repository observations also remain available for retraction when contradictory project/sync bindings make positive evidence unusable. An unrelated repository alone has no authority to retract this project's history. The library atomically synchronizes these sessions, including an empty set after a rejection or revert. Completely absent imports leave historical library evidence available; available pages, as for other live sessions, replace older stored interpretations.

Original `{events:[...]}` response JSON, arrays of original event wrappers, JSONL wrappers, and original `.har` files are read from the normal Lovable source directory (`~/.lovable`, or `CREDITIA_LOVABLE_DIR`). Original response records may be split across HAR files; they are joined by project and native event identity. The reader is offline; it does not fetch history or require a new account export to run. This work does not claim Lovable provides a built-in export button, or that a logged-in user's files were observed.

## Integration status

`registry.rs` routes this reader into the shared CLI/App scan, discovery and doctor; the old unsupported Lovable gap row in `cloud.rs` is removed. Real-binary CLI regressions (`bolt_lovable_devin_cloud_native_imports_reach_final_attribution` and `relative_cloud_repository_conflicts_retract_persisted_attribution` in `crates/cli/tests/e2e.rs`) verify final `credits.json` attribution, session metadata, incremental idempotence and rejection of persisted evidence after relative-path HAR repository or owner conflicts.

## Other official avenues checked

The [official Lovable MCP documentation](https://docs.lovable.dev/integrations/lovable-mcp-server) and [official public MCP repository](https://github.com/lovablelabs/mcp) document `get_diff` (message or commit unified diff), `list_edits`, and original message retrieval. These establish available read capabilities, but the inspected public reference does not establish a per-tool successful-result/body/time response schema. Message-level or commit-level diffs were not relabeled as per-call writes. No authenticated MCP request was made.

## Reproducible public resources

Downloaded scripts remain outside the repository at `/private/tmp/creditia-cloud-client/lovable/`. Hashes cover actual response bytes. Application filenames are versioned by their hash and may change on the next release.

| Resource | Bytes | SHA-256 |
|---|---:|---|
| `dashboard-shared-2rv9s-Mi.js` | 453178 | `35ae3e12efca6c8759bbc480be1bc2fe9097cc95a23df8301f9dcb99e0a24a25` |
| `HunkDiffViewer-Cg0PYYre.js` | 818970 | `e6944f75a05400c1360e09eb2aabd037f4f59d6d735e240a2246a971258c116c` |
| `ActivityView-DfoLjBcY.js` | 24209 | `8d49108dfecac70da973bbc55432f9d39ed4d5f60f73f454ebad609e9e2daa17` |
| `registry-CMXIDPt4.js` | 270107 | `da941a0c7c476a16d78d6bf78f7369e4855862571340b8def788d27ea1426d79` |
| `CopyMessageLinkButton-ByiTyUuR.js` | 70525 | `ab63e22fbe8893f247f2948c1a4d79b5380de68d14b1c31624f9f9656aa9fce9` |
| `ThinkingTimelineItem-89gfR0Am.js` | 18554 | `5ced229dc20f37bc1c01be32838ea0c29f914d874a232eb3eae3f2b4e1097f16` |
| `VerboseActivityTimeline-B3n_E5rY.js` | 4554 | `9e4abfd88aff69baf6b67f17faa42958d7dd4639624a5c00bf9dde9d4ab4fa3c` |
| `route-shared-leaves-Hz8d0PVl.js` | 80964 | `f21fb9f35e58e952e628fc8b050bceb7cf76aa9e19b44774db974576533be76c` |
| `ProjectGitSettingsTab-C7jFV406.js` | 66475 | `ab846c6e481f6393f5bda783d1e4b04da5ffe5b926144eb1a871aad31f68b645` |
| `app-shared-ljm5BCJR.js` | 1302767 | `7c2aa149ab94992696fa26bd4a57fb1d7a2368728759699f78a0476e8379cafa` |

`CARGO_TARGET_DIR=/private/tmp/creditia-agent-target cargo test -p creditia-adapters lovable --offline` passes 15 tests. They construct the exact public outer event and consumed hunk contracts above and are **not authenticated real-account captures**. They cover successful add rows and inference binding, invalid/unknown hunks, failures/pending/read/preview, absent or mismatched changes, unknown model/time, duplicate versus distinct call identities, conflicting observations, copied/reverted/cyclic ancestry, whole-file/blob/unscoped-relative rejection, reversed event order and split pages, multiple completion/diff ambiguity, and deletion/conflict/rebuild behavior. The new regressions additionally cover remove/add re-emission of `human_original_line` alongside `actual_added`, valid original-HAR repository mapping (including base64 response text), wrong project/host/view/status, missing or unsynced/conflicting repository context, project/repository conflict retraction, traversal, and filename-suffix-only matches. Real-account codec prevalence and user acceptance remain unverified.
