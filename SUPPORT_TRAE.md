# TRAE desktop native reader

TRAE desktop (`trae-ide`, `Src::TraeIde`) is separate from TRAE CLI (`trae`, `Src::Trae`). The native reader uses desktop history tables and original successful tool results. It does not read code-contribution telemetry or infer changes from the current working tree.

## Evidence and version boundary

The implementation was traced in the official Linux ARM64 **3.6.2 / build 2.3.91093** distribution, obtained from the [official version endpoint](https://icube-normal.trae.ai/icube/api/v1/native/version/trae/latest) and its [release archive](https://lf-cdn.trae.ai/obj/trae-ai-sg/pkg/app/releases/stable/2.3.91093/linux/TRAE-linux-arm64.tar.gz). `libai_agent.so` SHA-256: `750360db5dac81e74cd8b4d38ff35e3c8b6a336aea77701ba3aa49857b4c5692`.

The official endpoint returned **3.6.4 / build 2.3.93439** on 2026-10-19. Its Linux ARM64 Debian package was also inspected read-only (436066532 bytes; SHA-256 `f5cdeb0fa78980261e5b81fcfaedaf1c7e6f116de00622f726290e06da889915`). Current `libai_agent.so` SHA-256: `ed7f416ecf480a74208001132ffc497deb51f4c97af7fc1f607b8281de9acef5`.

The current native config dispatch, cyclic XOR loop and KDF call were independently checked: the config and salt are unchanged; the SHA-256 descriptor retains digest length 32, block length 64 and all eight initialization words; KDF still uses 100000 rounds, 16 salt bytes and 32 output bytes. Native project/turn/session/message/task records, TaskContent, task-message, plan-item, timing, changes and status field sequences match the older artifact. The relevant frontend history and result transforms also retain the same contracts. This verifies current static codec/schema parity, not an authenticated native-account run.

The archive's `resources/app/node_modules/@byted-icube/ai-modules-chat/dist/index.mjs` and Rust libraries establish:

- `chat_message` stores the session/message index, role, type, archive/delete markers, reply relationship and historical user context. `chat_message_task` stores task content separately. Plain assistant chat contains prose/reasoning and is not tool-write evidence.
- Task history contains `messages[].{type: "plan_item", plan_item: {tool_call_info: {id, name, params, result}, timing}}`. The backend task entity retains `message_type` as its discriminant; the wire API uses `type`. Both are read explicitly. The backend `ChatMessageContent` task-variant serializer has `content`, `task_id`, `summary`, while the assembled `ChatAssistantTaskContent` has `task_id`, `user_message_id`, `messages`; its task-message array and the assembled `{messages}` representation are supported. These remain static serialization observations, not real-account samples.
- The original result is `{status, error_message, data, render, is_truncated, interrupt, ...}`. The reader requires raw `status == "success"`. The UI can display an old `running` item as success; its computed display status is explicitly excluded.
- Actual file changes are in `result.data.changes[]` with `file_path`, `rename_file_path`, `file_action`, `old_content`, `new_content` and `diff_info`. The UI's save-last-snapshot path visits `added`/`modified` changes. Preview tools such as `show_diff` are excluded even if they contain similar text.
- `timing.tool_call_finished_at_ms` is per-call completion time. Its absence remains unknown; message creation time is not substituted. Historical context and smart-selection model configuration do not prove each tool's execution model, so per-edit models remain unknown.

## Read-only storage and codec

Default roots are the platform application-support directories for `Trae` and `Trae CN`. The reader checks `ModularData/ai-agent/database.db`, follows an absolute `.last-db-dir` in that directory, and checks the legacy `ModularData/ai-chat/database.db` when the current database is absent. A configured directory is authoritative; an unreadable custom database does not silently fall back to stale default history. `CREDITIA_TRAE_IDE_DIR` can point to an application data directory or a specific `.db`.

The database codec is a release-wide constant in the public executable, not an account token. The fixed `mics_sj10gy` configuration is cyclically XOR-decoded with `rust`, `cpp`, `electron`, then PBKDF2-HMAC-SHA256 (100000 rounds, fixed 16-byte salt) produces a 32-byte raw key. Startup converts that to lower-case hexadecimal for SQLCipher's raw-key notation. The reader does not inspect login storage, tokens, cookies or keychains. Fixed codec values and derived keys are not written to diagnostics.

The strict [SQLCipher reader](SUPPORT_SQLCIPHER.md) authenticates database/WAL pages, checks snapshot consistency, and opens a read-only in-memory SQLite connection. Unknown keys, corrupt pages and incomplete schemas produce visible errors. No history database, WAL, settings, or real project is modified.

## Counting and lifecycle behavior

Only native write/edit tools with a successful original result and `added`/`modified` actions are considered. Existing-file overwrites require both actual before and after strings. A missing/null old string is allowed only when the native result explicitly says the file was added. Only added lines after subtracting before text become evidence; requested `params.content`, insert-line counters and preview diffs do not substitute for this result.

The reader skips archived/deleted assistant messages and responses whose initiating user message was archived/deleted. Deleted sessions are rebuilt as empty when their identity was previously observed, allowing the library to retract stale evidence. A missing or unreadable database/body cannot authorize retraction and reports a warning instead.

All successfully read native databases are reconciled together in one `trae-native:all` group. Repeated copies with the same native tool-call ID are counted once across Trae and Trae CN; contradictory observations of that ID suppress its evidence. Session metadata is counted once per native session ID. A clearly deleted session copy, conflicting project/creation metadata, or archived/deleted message copy vetoes stale successes from another database. Old per-path cache groups are removed after a successful global rebuild. Missing/unreadable databases do not create negative observations; when no database can be read, existing cache is left untouched and read errors remain visible. All files in a multi-file call are preserved. The implementation does not claim that fork sessions which rewrite all native call IDs have been validated. Real fork/revert account histories remain an acceptance gap.

Renamed successful changes use the retained `rename_file_path` destination; original content remains the before state. Project paths are bound through native project records with `project_id` and `absolute_path`; conflicting bindings stay unknown. Absolute receipt paths can still be matched against the scanned repository. Relative receipt paths require a known project root.

## Validation and remaining gaps

Eleven focused adapter tests cover the independently derived release codec vector, actual added text with per-call time and explicitly unknown execution model, failed/running/preview/unknown-before exclusions, missing execution timestamps and text deletion dates, archive/delete retraction, multi-file duplicate calls and conflicts, and historical path/project boundaries, native task discriminants and saved rename destinations. Cross-database regressions cover duplicate successes, success/failure conflicts, deleted/conflicting session copies, archived/deleted message copies, metadata deduplication and missing/unreadable database preservation. A constructed task-contract database was encrypted by an independently built official SQLCipher **4.5.7** shell for end-to-end testing. It contains fictitious data, not a recording produced by the TRAE App.

The real-binary CLI regression `trae_native_encrypted_history_reaches_attribution_and_retracts_failed_results` starts from those encrypted files and runs `init`, `doctor` and `analyze` against an isolated Git repository. It verifies exactly one actual added line, the native completion timestamp, an unknown execution model, persistent-library idempotence, success-to-failure retraction and restoration, and duplicate/conflicting copies across both default Trae data roots. The original database bytes remain unchanged and no sidecars are created. The fixtures and their reproducible reference-shell generator are in `crates/cli/tests/fixtures/trae-native`.

No `ModularData` native history database was present under either local TRAE application-support root during this work. Thus real-account native reading, fork-ID rewrite behavior and full editor interaction remain independently unverified. The nearby `FileEventCacheStore` `.events.jsonl`/`.manifest.json` backend is not used: its desktop activation/root has not been established. Missing editor logs are not attributed to cleanup.
