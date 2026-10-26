# Constructed TRAE native-contract databases

These are fictitious native-contract rows, encrypted by the unmodified official
SQLCipher 4.5.7 shell. They are **not databases recorded by the TRAE App** and
contain no account, token or personal project data.

The serializer fields were traced in official TRAE 3.6.2 and checked against
3.6.4: `chat_message_task.content` contains a TaskContent with `task_id`,
`user_message_id`, and `messages`; each native task item has
`message_type: "plan_item"`, and the original `tool_call_info.result` contains
the status and actual file changes. See the repository's `SUPPORT_TRAE.md`.

Both fixtures contain one native session (`native-s`) and one edit of
`/private/tmp/creditia-trae-native-project/a.rs`. The actual result removes
`removed line`, retains `unchanged line`, and adds exactly
`actual native fixture line`. Its requested parameter text is deliberately
different. The per-call completion time is 1720000005; a historical model
configuration is present but is not a proved execution model.

| File | Original status | SHA-256 |
| --- | --- | --- |
| `success.db` | `success` | `e8298e0373fd7c8b2ae930ddb1083170396a67bca4460cec2dcaf5e31acba91c` |
| `failed.db` | `failed` | `113ed7fcb07ff5c7b2bdfaf6bdbdb02d66d632fef92b36c85af86fc559030d9e` |

Both passed the official shell's `PRAGMA integrity_check`. Random IVs mean
regenerating equivalent fixtures produces different hashes.

To reproduce into an empty directory, use the independently built official
SQLCipher shell described in the adapters' SQLCipher fixture README, and the
pinned official 3.6.2 `libai_agent.so`:

```sh
python3 generate.py --sqlcipher /path/to/sqlcipher \
  --native-lib /path/to/libai_agent.so --out /path/to/empty/fixtures
```

The generator verifies the release library's SHA-256 and derives its public
release codec in memory without printing it. It refuses to replace existing
files. The CLI regression copies these fixtures into an isolated test home and
checks doctor, final `credits.json`, stored hashes, unknown execution model,
the call's own timestamp, idempotence, read-only input and failure retraction.
