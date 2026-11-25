# Current public cloud-client contracts

Checked 2026-10-17. This extends `SUPPORT_CLOUD.md`: it inspects the actual current public web application assets, rather than treating the old Bolt repository or public marketing/API documentation as the current implementation. The Bolt and Lovable sections below record the state at that checkpoint; both gaps were subsequently closed and are now implemented — see [SUPPORT_BOLT.md](SUPPORT_BOLT.md) and [SUPPORT_LOVABLE.md](SUPPORT_LOVABLE.md). The remaining sections are current.

Only public HTML, script resources and source-map URLs were fetched. No login, account token, private API request, model call, application installation or execution was involved. Minified JavaScript was read as text; embedded protobuf descriptors were decoded as data. The fetched artifacts remain outside the repository at `/private/tmp/creditia-cloud-client/`.

## Implemented: Replit Agent

`crates/adapters/src/cloud_native.rs` reads either of these original official JSON shapes:

1. Protobuf-JSON `chateau.frontend.v1.SyncBlocksResponse`: `{ "ops": [ { "block": ... }, { "update": ... }, { "close": ... }, { "reset": ... } ] }`. Save one response as JSON, an array of response bodies as JSON, or response bodies as JSONL. A raw snapshot must carry nonempty `id`, `agentId` and `turnId`. The latter two identify the session as `agentId/turnId`; they are not a guessed conversation ID.
2. The client's `downloadFeedRecording` format: `{ "version": 1, "recordedAtMs": ..., "constants": { "replId": ..., "agentId": ..., "conversationId": ... }, "events": [ { "t": ..., "stream": "blocks", "key": "replId/agentId/turnId", "type": "chateau.frontend.v1.SyncBlocksResponse", "payload": { "ops": [...] } } ] }`.

Place these files in the existing Replit import directory returned by `source_dir(Src::Replit)` (normally `~/.replit`, or `CREDITIA_REPLIT_DIR`). The reader is offline. It does not fetch authenticated history. The shipping public client explicitly sets `IS_FEED_RECORDER_AVAILABLE=false` and makes `withFeedRecording` an identity function: this work does **not** claim users can currently click a working built-in export. Manually saved existing original response bodies are supported independently of that switch.

The current [activity and Timeline protocol bundle](https://cdn.replit.com/_next/static/chunks/0lww5pc-3bpz9.js) contains both the wire descriptor `chateau/frontend/v1/timeline.proto` and the runtime block content schemas:

- `BlockSnapshot`: `id`, `kind`, `parent`, `content` (bytes), `open`, `turnId`, `openedAt`, `status`, `agentId`, `closedAt`.
- `BlockUpdate`: `id`, `patches[]`, `status`. Each patch has `op` (ADD=1, REPLACE=2, REMOVE=3), JSON Pointer `path`, and JSON bytes `value`. The reader supports object and array operations; an invalid target or unknown operation invalidates that block.
- `BlockClose`: `id`, `closedAt`; `BlockReset`: `turnId`.
- `CreateFile` content: `{path, chunks: string[], ranIn?, result}`.
- `FileEdit` content: `{path, unifiedDiff: string, ranIn?, result}`.
- `ActivityResult`: `{status:"live"}`, `{status:"completed"}`, or `{status:"cancelled",code,reason}`. The independent block lifecycle is `LIVE`, `COMPLETED` or `CANCELLED`.

The [feed recorder bundle](https://cdn.replit.com/_next/static/chunks/25mnqzltzupus.js), module `79807`, explicitly decodes block content using `JSON.parse(TextDecoder.decode(bytes))`, and serializes wire payloads with protobuf `toJson`. Accordingly bytes in saved protobuf JSON are base64 strings and oneofs are named members (`block`, `update`, etc.), not JavaScript runtime `{case,value}` objects.

Evidence is emitted only for a completed `CreateFile` or `FileEdit` with no contradictory live/cancelled lifecycle. Repeated snapshots replace state; updates and closes are replayed; reset removes prior state. Raw responses route id-only updates only after an unambiguous snapshot identity. Creates use the actual returned chunks; edits use only additions from a single verified unified diff matching the content's path. Read blocks, shell output, cancelled/pending blocks, invalid patches, generic `{success,path,content}` objects and project snapshots are rejected.

The verified contract has no working-directory field or per-write actual model in these blocks. `ranIn` identifies a repl/conversation/sandbox, not a filesystem cwd. Therefore relative paths are rejected; absolute paths must match the normal project mapping, including explicit user links. No cwd is invented from a Repl ID, and no final model selection is copied onto old rows. Only `closedAt` is used as an edit timestamp; a missing time stays `0`/unknown. No session/recording time fills that gap. Since no trustworthy project root is available, `discover()` does not invent one from a file's parent directory.

`cloud_native::scan` rebuilds the whole import directory into one `replit:native-import:` group, so it can coexist with `cloud::scan` metadata. Every export is replayed separately, then reconciled by native `replId/agentId/turnId/blockId`: repeated exports do not multiply evidence or sessions, while distinct block IDs with identical text retain distinct occurrences. Replaying separately prevents overlapping ADD patches from being appended twice. A compatible live content prefix can be superseded by a completed observation. Cancelled/invalid observations, conflicting completed bodies, unresolved id-only updates and reset views missing the block conservatively suppress that identity; filenames and import order do not settle conflicts. Conflicting completion timestamps remain unknown. Raw response files share `agentId/turnId/blockId` identity (the wire body has no Repl ID); global unambiguous snapshot ownership lets update-only files conservatively veto a stale successful copy. Deleting an import file rebuilds the set, so removed duplicates or conflicts cannot leave stale cached evidence. Its Replit doctor row replaces the old generic Replit gap row in the central caller; keep the other `cloud.rs` rows. Its note names the raw-response/recording shapes, absolute-path requirement, unknown cwd/model and disabled public recording switch. No shared registry or UI file is modified by this subtask.

The activity bundle's declared source map resolves to a valid 71-byte map with empty `sources` and `sourcesContent`; the schema evidence is in the shipping JavaScript and embedded descriptor itself.

## Bolt.new: exact remaining boundary

The current [API client](https://bolt.new/assets/api-client-DOdYlOlO.js) implements real history persistence:

- `updateChatHistory(projectId, messages, organization)` posts `{messages,description}` to `/api/chats/{projectId}`.
- `getChatHistory` reads `{agent,model,isLatestStateForge,effortLevel,designSystemId,editedAt,messages,hasMore}`. It preserves message members and `parts`, and then rewrites most reloaded statuses to `complete`. Thus message `status:"complete"` alone is not a successful editing receipt.
- The same client's `updateChat` changes project metadata. The [Chat client](https://bolt.new/assets/Chat.client-DkyWj5Jm.js) posts `/api/chat/model` to change the selected model. The response's top-level `model` is therefore mutable project selection, not a per-write historical model.

The current Chat client handles modern `Read`, `Write`, `Edit`, `MultiEdit`, `NotebookEdit`, etc. A part is `{type:"tool-invocation",toolInvocation:{toolCallId,toolName,args,state,result}}`; it also handles `message.toolInvocations`. Its debug renderer considers `state=="result"` finished and specifically checks `result?.type=="error"`; `stringifyToolInvocation` otherwise accepts arbitrary strings, arrays, `content` objects, or arbitrary JSON. This is an untyped display path. It does not establish a machine-checkable successful `Write`/`Edit` receipt schema, a guaranteed resulting edit body, or a per-tool execution timestamp. Absence of that one `type:"error"` tag is not enough to fabricate success.

`file_event` annotations expose `filePath` in the inspected client; its analyzer fetches that path's **current** contents from the live workbench file store. That is not a returned before/after body for the original AI write. The [message serializer](https://bolt.new/assets/serialize-messages-uu8F8s9A.js) additionally has a request-history slimming path that converts tool invocations to calls and retains only `file_path`, `command`, `description` arguments. This is request serialization, **not proof that all stored history discards results**.

The [workbench bundle](https://bolt.new/assets/workbench-CqEfvfNn.js) serializes compressed snapshots as `{timestamp,snapshot}`; snapshot metadata includes `messageId`, `isFinalized`, `commitSha` and `repoId`. These may contain manual editing and are not imported as per-AI tool writes. `/api/chat/debug` displays arbitrary returned JSON through `safeStringify` and supplies no response validator in the public client; it was not called.

Both current JavaScript files explicitly advertise `.js.map` URLs; those public URLs returned HTTP 404 on this check. No new Bolt line importer is added: the specific missing evidence is the typed successful write-result/body/time contract behind the generic tool display, not a blanket assertion that cloud histories or source code do not exist.

## Lovable: persisted execution results verified, inner diff schema still missing

The normal public `/login` page loaded the current application resource graph. The root homepage returned HTTP 403 to the plain downloader; that restriction was not bypassed. The publicly referenced lazy modules were fetched directly, including the actual project route body, ChatThread and its trajectory reducer dependencies.

The [current trajectory client](https://lovable.dev/assets/app-lazy-shared-ClOWhgKT.js) explicitly reads `/{resource}/{scopeId}/trajectory?filter=frontend&inclusive=true&chain=view&head_id=...` (or `before=...`, optional `limit`). It expects `{events:[...]}` and passes each event to protobuf `fromJson(EventWrapperSchema, event, {ignoreUnknownFields:true})`. Its websocket `type:"trajectory"` frames carry `head`, `event` and `id`, and use the same parser. These are persistent history and live execution event contracts; no authenticated endpoint was contacted here.

The [current trajectory descriptor and reducer](https://lovable.dev/assets/dashboard-shared-2rv9s-Mi.js) embed `trajectory/events.proto` and prove:

- `EventWrapper`: `{id:{value},timestamp,payload,parent,copiedFrom?}`. `EventPayload` uses protobuf-JSON named oneofs such as `agentOutputToolCall`, `toolParametersValidated`, `toolExecutionStart`, `toolExecutionEnd`, `agentOutputEnd`, `gitCommit`, `revert`.
- `agentOutputToolCall`: `toolName`, `toolUseId`, `params` (string), `toolId`, `integrationName`, etc.
- `toolParametersValidated`: `toolCall:{value}`, `params` (JSON string), `innerToolCalls`, metadata.
- `toolExecutionEnd`: `toolCall:{value}` and exactly one of `hardError` (string), `softError`, or `output:ToolOutput`. The reducer only maps the output branch to `type:"tool_output"`; approval, rejection, start, and completion are separate events.
- `ToolOutput`: `content:Prompt`, `changes:Change[]`, `writableFiles`, `isFree`, `diffs:FileDiff[]`, `metadata`, `outcomeId`.
- `FileDiff`: `filePath`, `content:InlineOrRef`, `tooLarge`. The inline bytes are decoded with TextDecoder then **JSON.parse**, and passed through as `{filePath,hunks:parsedJSON}`. A blob reference is placed in a separate `blobRefs` collection. No unified-diff string contract is declared here.
- `agentOutputEnd.model:ModelDescriptor` provides actual `modelName`, `provider`, `region`, `endpoint`, effort and related fields. The UI aggregates model names within a reduced AI message. This is a per-inference field, **not** a mutable final project model; binding it to tool calls still requires the event/iteration chain, rather than backfilling all session lines.

`Change` identifies addition/deletion/rename/copy/edit paths, not line content. The inspected current project route body, ChatThread, ChatTimeline, ChatItemWorkspace, tool registry and shared rendering modules do not declare or consume the inner JSON hunk row structure. The reducer's `JSON.parse` result is unvalidated and typed as data passed through. The current shared editor's legacy `tool_diffs` is also forwarded without a hunk serializer. Neither the trajectory bundle nor the current related modules advertise a public source-map URL; probing the conventional trajectory `.js.map` path returned 404.

Consequently a real successful result envelope and its body location are now established, but the content is an opaque inline JSON hunk representation or a blob reference. No guessed `lines`, `newLines`, `added` flags, complete-file content, or unified-diff encoding is accepted. A real saved successful `ToolExecutionEnd.output.diffs[].content.inlineData` value (or the declared serializer for that JSON) is the specific remaining input needed to safely add a Lovable line reader. Git commits, editable snapshots, validated parameters, and approval events do not substitute for that value. No new Lovable line importer is added.

## Reproducible resource identities

The current sites use hashed filenames; retain these identities when rechecking. Bytes and SHA-256 below are from the actual downloaded bytes, not decoded-text character counts.

| Official resource | Bytes | SHA-256 |
|---|---:|---|
| Replit `0lww5pc-3bpz9.js` | 492300 | `236bb28714131f5cf79665d378532686ce60c316784c71454af34f2342486923` |
| Replit `25mnqzltzupus.js` | 85015 | `25898f7b9d5b8ab8fad9fb5efd265b7c90ee622ca39b09ab0c87f55bb844f46f` |
| Bolt `api-client-DOdYlOlO.js` | 97311 | `bab9e6cfcc171b8495db25906dfba7c35276450799bf177751b1f733f01cda01` |
| Bolt `Chat.client-DkyWj5Jm.js` | 1354845 | `989050d7d663a485e84ab2adaf5c50c35ab9fef8de1006cc12fc3a68d191c242` |
| Lovable `dashboard-shared-2rv9s-Mi.js` | 453178 | `35ae3e12efca6c8759bbc480be1bc2fe9097cc95a23df8301f9dcb99e0a24a25` |
| Lovable `app-lazy-shared-ClOWhgKT.js` | 936692 | `7c1edfa1d99f30919d0fa3ad5f89b42e2cde0ab3ebc63ab794dc1c57135e23b6` |

Validation: `CARGO_TARGET_DIR=/private/tmp/creditia-agent-target cargo test -p creditia-adapters cloud_native --offline` passes 9 tests. Fixtures are constructed from these verified contracts; they are not claimed as authenticated real-account captures. Tests cover original response identity, recording scope, successful create and edit additions, incremental result updates, unknown times/models, cancellation, pending/read blocks, relative/out-of-project paths, resets, invalid patches and generic fake receipts. Filesystem regressions also cover repeated feed and raw-response exports, same-text distinct blocks, changed/cancelled copies, orphan updates, compatible incremental completion, and removal/rebuild behavior. No real-account acceptance test was possible or attempted in this scope.
