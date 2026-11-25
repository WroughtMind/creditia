# Devin Cloud: native ACP session updates from the web client

Protocol checked 2026-10-17; attribution regressions checked 2026-10-19.
`crates/adapters/src/devin_cloud.rs` reads the session-update
stream that the current Devin web client (app.devin.ai) itself consumes, as an
offline import. Only publicly downloaded client modules, the official npm
manifest and `v3-openapi.yaml` were read; no account login, WebSocket
connection, model invocation or downloaded binary execution was performed.

## What the public client establishes

The research artifacts (identities in `resource-identities.json` under
`/private/tmp/creditia-devin-followthrough/`) contain the client's generated
Zod schemas, which mirror the wire protocol exactly:

- The web app opens one WebSocket per session at `{base}/acp/live?token=...`
  (`helpers-CWg7run2.js`, `connectSocket`, the `[acp-ws]` logs). History is
  replayed through ACP methods `_cognition.ai/session/loadPage` and
  `_cognition.ai/session/loadSpine`, live updates arrive as `session/update`
  notifications and batched `_cognition.ai/session_update_batch` ext
  notifications (`app-initial-E8xsbGEo.js` method constants;
  `helpers-CWg7run2.js` batch dispatch). A HAR capture cannot carry these
  bodies; the reader therefore consumes the native JSON envelopes directly.
- loadPage request (strict schema): `{sessionId, range: {start: cursor|null,
  end: cursor}, limit}`; response: `{updates: [{sessionId, update}],
  coveredRange: {start: {cursor, sortKey}|null, end: {cursor, sortKey}},
  hasMore, replayFilterVersion}` (`helpers-CWg7run2.js` schema init, the `Pb`,
  `Kw`, `Yw`, `Xw` definitions).
- The `update` objects are ACP SessionUpdates discriminated on `sessionUpdate`
  (`UO` union): `user_message_chunk`, `agent_message_chunk`,
  `agent_thought_chunk`, `tool_call`, `tool_call_update`, plan/config/usage
  updates. Tool status enum: `pending | in_progress | completed | failed`.
  Tool kind enum: `read | edit | delete | move | search | execute | think |
  fetch | switch_mode | other`.
- `tool_call`/`tool_call_update` carry `{toolCallId, title, name?, kind?,
  status?, content?, locations?, rawInput?, rawOutput?, _meta?}`; content items
  are `{type:"content"...}`, `{type:"terminal"...}` or `{type:"diff", path,
  oldText?, newText, _meta?}`. The official reducer
  (`acpSessionState-yfjwsDrG.js`, `lf`/`Rf`/`zf`) treats a diff with
  `oldText == null` as a create, and prefers `_meta["cognition.ai/contentsKey"]`
  / `"cognition.ai/prevContentsKey"]` over the inline texts when present: a
  referenced body is authoritative and the inline copy is not independent
  evidence.
- Per-record metadata keys (schema in `helpers-CWg7run2.js`):
  `"cognition.ai/timestamp"` (string, per message/tool), `"cognition.ai/model"`
  and `"cognition.ai/modelProvider"` (agent-message level; tool calls carry no
  model), `"cognition.ai/owner"` + `"cognition.ai/repo"` and
  `"cognition.ai/prUrl"` (PR-card messages), `"cognition.ai/toolName"`,
  `"cognition.ai/toolArgs"`, `"cognition.ai/fileUpdates"`,
  `"cognition.ai/eventId"`.
- The REST endpoint `GET events/{sessionId}/skeleton` returns metadata-only
  rows (`{items: [{id, type, created_at_ms, ...}], has_next_page, key_space}`);
  the documented v3 OpenAPI has no message-content endpoints. Neither is used
  as line evidence.
- `session/new`/`session/load` responses carry `{sessionId, cwd, ...}`, so a
  live client knows the workspace; the import reader cannot assume any cwd.

## Accepted input and attribution rule

Place original captured envelopes in `source_dir(Src::Devin)` (`~/.devin`, or
`CREDITIA_DEVIN_DIR`), as `.json`/`.jsonl`. Accepted shapes — nothing else:

1. A bare loadPage response (`{updates, coveredRange, hasMore,
   replayFilterVersion}` with each update carrying a non-empty `sessionId`).
2. A JSON-RPC response frame whose `result` is shape 1.
3. A `session/update` notification (`{method:"session/update", params:
   {sessionId, update}}`).
4. An `ext/notification` frame wrapping `_cognition.ai/session_update_batch`.

A line is emitted only when all of these hold:

1. A tool call reconciled by native `(sessionId, toolCallId)` has kind `edit`
   and final status `completed`, with no observation ending in `failed`.
   `tool_call_update` replaces exactly the fields it carries, per file; the
   latest non-empty content array is the call's content. Overlapping page
   captures merge by native id, and disagreeing kinds, statuses or contents
   across files suppress the call.
2. A content item `{type:"diff"}` with inline `oldText`/`newText` and **no**
   `contentsKey`/`prevContentsKey` meta. Bodies that exist only as reference
   keys are skipped; a key or current file contents are never treated as the
   historical body. Duplicate diff items for one path invalidate that path.
   `oldText == null` is accepted as an official create.
3. Only `added_lines(oldText, newText)` is emitted; retained context and
   re-emitted human lines never become AI evidence.
4. Absolute paths go through the normal PathMap boundary. Repository-relative
   paths additionally require the session's own PR-card messages to report
   exactly one repository (`cognition.ai/owner`+`repo` or a single `prUrl`
   base), matched by `PathMap.in_project`, with the relative path present in
   the project's path set and surviving exclusions unchanged.
5. The execution time is the tool's own `_meta["cognition.ai/timestamp"]`
   (ISO or epoch string); conflicting captures leave it unknown. The model is
   bound only when every observation of that call agrees on the same user
   `messageId` and one distinct `cognition.ai/model` on that turn's agent
   messages. Conflicting models, incomplete captures, missing user boundaries,
   or an update crossing into a later user turn leave the model unknown in
   every file order. Interleaved sessions retain separate turn boundaries.
6. Sessions use the native sessionId; prompts count distinct user
   `messageId`s; only sessions with emitted evidence are recorded. cloud.rs
   removes only CLI line occurrences actually covered by native evidence in
   the current project, matched by session, mapped path and line hash with
   occurrence counts. Unmapped, unchanged, duplicate-path or conflicting
   native diffs cannot discard CLI edits; uncovered edits in a partial capture
   remain available. When the native reader emits mapped evidence and a session
   record, that single native session owns prompt/time metadata; the CLI group
   retains uncovered lines without a duplicate session record. Evidence-only
   import groups are retained. Native edit sessions with a diff path mapped to this
   project also mark their current import set for atomic evidence-library
   reconciliation. This boundary uses every observed diff path and the same
   PathMap and session-repository rules as emitted evidence, independent of
   status/model conflicts. For revocation, raw PR-card repository observations
   retain a known binding to this project even when a later conflicting card
   makes positive relative-path evidence unmappable. New line evidence still
   requires exactly one repository. Captures containing only another project's
   paths or no locatable diff path cannot revoke this project's history. A newly observed
   failure or conflict retracts stale evidence rather than retaining an old
   model or line. Missing or malformed captures and pure read sessions do not
   declare authority to erase historical evidence.

## Boundaries

The web client's WebSocket transport means there is no one-click export behind
this reader; a user must capture the frames themselves (for example from a
logging proxy or devtools). That capture step is not provided or automated
here, no authenticated request was made, and no real-account capture was
available or attempted: the fixtures are contract-derived constructions, not an
acceptance claim. Cross-file ordering of updates is never inferred, so a turn
split across two files keeps its model unknown. `replayFilterVersion` changes
are not tracked across files. The `cognition.ai/fileUpdates` whole-body
records and `switch_mode`/plan updates are not line evidence.

## Reproducible artifacts

Schema and conversion evidence live in the prettified public modules under
`/private/tmp/creditia-devin-followthrough/pretty/` (byte identities recorded
in `resource-identities.json`; fetched from app.devin.ai public assets by
`fetch_public.py`). Key files: `helpers-CWg7run2.js` (protocol schemas,
transport, meta keys), `acpSessionState-yfjwsDrG.js` (tool state to display
events), `app-initial-6IIFOHGW.js` (loadPage pagination), `app-initial-W006L-IM.js`
(events skeleton REST), `app-initial-E8xsbGEo.js` (method constants).

`CARGO_TARGET_DIR=/private/tmp/creditia-devin-fix-target cargo test -p creditia-adapters devin --offline`
checks completed-edit evidence with turn-bound model and own timestamp,
creates, failed/read/unfinished vetoes, referenced keys and duplicate paths,
all four native envelope shapes, duplicate-merge versus conflicting-copy
suppression, repository binding, conflicting/incomplete model copies in both
file orders, late updates crossing turns, missing user boundaries, actual
project evidence before CLI precedence, partial CLI overlap and occurrence
counts, project-bounded authoritative session reconciliation, and scan rebuilds. New regression
tests were run against the old implementation first: four failed with the
incorrect chosen model or the out-of-project session override (local logs:
`/private/tmp/creditia-devin-before.log`,
`/private/tmp/creditia-devin-after.log`).
`cargo test -p creditia-cli --offline --test e2e` adds a real-CLI regression
(`bolt_lovable_devin_cloud_native_imports_reach_final_attribution`) that runs
init/doctor/analyze as binaries and checks final `credits.json` attribution,
session metadata, incremental idempotence and the evidence library.
