# Support for 78 agent entries

[中文](AGENT_SUPPORT.md) | English

Support status updated 2026-10-26. Catalogue checked on 2026-10-23 against the [Vercel Skills agent registry](https://github.com/vercel-labs/skills/blob/main/src/agents.ts), source blob `c4af0e54d6f0c87d7d6863315be5c7acb4ed62fe`. Universal is a generic installation target, not a separate agent; the remaining 78 entries match the reference list. The CLI and display names share the machine-readable catalogue in [`packages/schema/agents.json`](packages/schema/agents.json).

All 78 entries have tool identities, explicit launch identity, commit attribution and skill installation instructions. **72 have history readers, 5 attribute through the executing host, and 1 remains unconfirmed.** Eve local source sandbox and inference.sh Codex driver/native export passed real-write → CLI → durable evidence → final credits acceptance. MCPJam local Codex/Claude histories, default workspace links and the official launch chain passed scripted-model checks covering successful, rejected and background commands plus repeated collection. The official CodeMaker CLI ran but requires an unavailable account key. See [native acceptance](SUPPORT_NATIVE_LIVE.md) and [MCPJam acceptance](SUPPORT_MCPJAM_LIVE.md). These counts include limited edits, sessions, imports and conditional readers; they do not claim every product mode is verified.

Use `creditia agents --json` for per-entry status and installation commands, or `creditia agents --agent <ID>` for one entry. The IDs below are accepted by `--agent`; the internal harness is the stable tool identity stored in the credits archive. Antigravity's editor and CLI remain separate. TRAE and TRAE CN share an editor identity; Qoder and Qoder CN share a tool identity while reading separate directories. Skill installation, identity, commit declarations, session discovery and verified edits are separate capabilities.

## All entries

Every row supports `creditia stamp --agent <ID>`. Commit stamping requires the project's hook to have been installed; an unknown model is left blank. Skill paths are project-level directories managed by the Skills installer.

| Agent | --agent ID | History scope | Project skill directory |
|---|---|---|---|
| AiderDesk | `aider-desk` | Native context v2: successful Power edits and committed Aider diffs | `.aider-desk/skills` |
| Amp | `amp` | Existing reader | `.agents/skills` |
| Antigravity | `antigravity` | Native edits | `.agents/skills` |
| Antigravity CLI | `antigravity-cli` | Existing reader | `.agents/skills` |
| AstrBot | `astrbot` | Native SQLite file-tool receipts; mapped absolute paths only, model unknown | `data/skills` |
| Autohand Code CLI | `autohand-code` | Limited native edits + ACP | `.autohand/skills` |
| Augment | `augment` | Native edits | `.augment/skills` |
| IBM Bob | `bob` | Native edits + ACP | `.bob/skills` |
| Claude Code | `claude-code` | Existing reader | `.claude/skills` |
| OpenClaw | `openclaw` | Successful native edit diffs; older records without diffs provide sessions only | `skills` |
| Cline | `cline` | Existing reader | `.agents/skills` |
| CodeArts Agent | `codearts-agent` | Conditional: known OpenCode v1 schema and saved successful diffs; vendor logs unverified | `.codeartsdoer/skills` |
| CodeBuddy | `codebuddy` | Native edits | `.codebuddy/skills` |
| Codemaker | `codemaker` | Official CodeMaker AI CLI 1.6.0 executed but requires an unavailable API key; Y3 port identity and durable edit receipts remain unverified | `.codemaker/skills` |
| Code Studio | `codestudio` | Native accepted editing timelines and text snapshots; per-request model | `.codestudio/skills` |
| Codex | `codex` | Existing reader | `.agents/skills` |
| Command Code | `command-code` | Native edits | `.commandcode/skills` |
| Continue | `continue` | Existing reader | `.continue/skills` |
| Cortex Code | `cortex` | Native edits + ACP | `.cortex/skills` |
| Crush | `crush` | Existing reader | `.crush/skills` |
| Cursor | `cursor` | Existing reader | `.agents/skills` |
| Deep Agents | `deepagents` | Native LangGraph SQLite: data-only JSON/MessagePack and CLI local-file receipts | `.agents/skills` |
| Devin for Terminal | `devin` | Limited hook edits + cloud ACP import | `.devin/skills` |
| Dexto | `dexto` | Native SQLite kv/list: paired successful writes/edits and workspace path mapping | `.agents/skills` |
| Droid | `droid` | Native edits | `.agents/skills` |
| Eve | `eve` | Eve 0.83.0 official local source sandbox verified end to end: write receipts, pinned mount and repository attribution; GitHub/eve-code paths remain conditional; overwrites and shell edits excluded, model unknown | `agent/skills` |
| Firebender | `firebender` | Native project SQLite: completed Write receipts and subagents; unknown model, IDE rewrites excluded | `.agents/skills` |
| ForgeCode | `forgecode` | Native .forge.db: message-level model and successful fs tool diffs | `.forge/skills` |
| fx | `fx` | Native v2: CRC32C journal, SHA-256 pieces and committed file presentations | `.agents/skills` |
| Gemini CLI | `gemini-cli` | Existing reader | `.agents/skills` |
| GitHub Copilot | `github-copilot` | Existing reader | `.agents/skills` |
| Goose | `goose` | Existing reader | `.goose/skills` |
| Grok Build | `grok` | Existing reader | `.grok/skills` |
| Hermes Agent | `hermes-agent` | Successful native patch receipts; model unknown | `.hermes/skills` |
| inference.sh | `inference-sh` | Codex mode verified with the official agentprotocol driver and native export, including index-free histories; credit belongs to the executor; standalone cloud tools remain unverified | `.inferencesh/skills` |
| Jazz | `jazz` | Native JSONL/gzip: complete successful diffs; historical model unknown | `.jazz/skills` |
| Junie | `junie` | Limited native edits | `.junie/skills` |
| iFlow CLI | `iflow-cli` | Native edits | `.iflow/skills` |
| Kilo Code | `kilo` | Existing reader | `.agents/skills` |
| Kimchi | `kimchi` | Native edits + ACP | `.kimchi/skills` |
| Kimi Code CLI | `kimi-code-cli` | Existing reader | `.agents/skills` |
| Kiro CLI | `kiro-cli` | Limited native edits | `.kiro/skills` |
| Kode | `kode` | Native projects JSONL: successful Write/Edit structuredPatch, separate subagents | `.kode/skills` |
| Lingma | `lingma` | Native local.db + workingSpace: accepted snapshots with matching MD5; public real database validated, relocated Windows snapshots supported, model unknown | `.lingma/skills` |
| Loaf | `loaf` | Native work journal inspected; code credit belongs to the executing host agent | `.agents/skills` |
| MCPJam | `mcpjam` | Reads local Codex and Claude executor histories, resolves workspace links and deduplicates native sessions; official bridges and real executors tested with scripted models; credit belongs to the executor; account model proxy, full managed launch and cloud RPC remain unverified | `.mcpjam/skills` |
| MiniMax Code | `minimax-code` | Native edits | `.minimax/skills` |
| Mistral Vibe | `mistral-vibe` | Native edits | `.vibe/skills` |
| Moxby | `moxby` | Desktop 3.2.0 native SQLite: terminal successful file_edit receipts, absolute paths and per-turn model | `.moxby/skills` |
| Mux | `mux` | Native active/archive JSONL: successful diffs, inherited fork prefix excluded | `.mux/skills` |
| OpenCode | `opencode` | Existing reader | `.agents/skills` |
| OpenHands | `openhands` | Edits imported from official exports | `.openhands/skills` |
| Ona | `ona` | Current official Codex Agent uses the Codex harness; read its native logs in the execution environment | `.ona/skills` |
| Pi | `pi` | Native edits | `.agents/skills` |
| Posit Assistant | `posit-assistant` | Native V2/V3 active branches, completed edit receipts and per-message models | `.posit/assistant/skills` |
| Qoder | `qoder` | Native CLI edits; separate international and CN directories | `.qoder/skills` |
| Qoder CN | `qoder-cn` | Native CLI edits; separate international and CN directories | `.qoder/skills` |
| Qwen Code | `qwen-code` | Existing reader | `.qwen/skills` |
| Replit | `replit` | Edits imported from official exports | `.agents/skills` |
| Reasonix | `reasonix` | Native flat/append/replace/schema-2 DAG: selected branch and successful post-write receipts | `.reasonix/skills` |
| Rovo Dev | `rovodev` | Native session_context: successful creation and actual edit diffs, per-response model; preserves pruned evidence | `.rovodev/skills` |
| Roo Code | `roo` | Existing reader | `.roo/skills` |
| Sarvam Code | `sarvam-code` | Native rollouts; successful patch_apply_end and per-turn model | `.agents/skills` |
| Tabnine CLI | `tabnine-cli` | Limited native edits | `.tabnine/agent/skills` |
| Terramind | `terramind` | Nucleus native local execution journal; completed Write/Edit and per-step models | `.terramind/skills` |
| Tinycloud | `tinycloud` | Native SessionStore JSONL; absolute paths, actual written text and model | `.tinycloud/skills` |
| Trae | `trae` | Native edits | `.trae/skills` |
| Trae CN | `trae-cn` | Native edits | `.trae/skills` |
| Warp | `warp` | Native edits | `.agents/skills` |
| Windsurf | `windsurf` | Edits imported from official exports | `.windsurf/skills` |
| Zed | `zed` | Existing reader | `.agents/skills` |
| ZCode | `zcode` | Native edits | `.zcode/skills` |
| Zencoder | `zencoder` | Native CLI history and IDE post-execution diffs; per-message CLI model | `.zencoder/skills` |
| Zenflow | `zenflow` | Native normalized-log database; final successful edits, model unknown | `.zencoder/skills` |
| Neovate | `neovate` | Native edits | `.neovate/skills` |
| Pochi | `pochi` | Native LiveStore SQLite: successful edit results; memory/OPFS require persistent export | `.pochi/skills` |
| PromptScript | `promptscript` | Instruction compiler/telemetry inspected: no code-edit receipts; attribute executing host | `.agents/skills` |
| AdaL | `adal` | Official JSON/NDJSON import: successful replace_by_string; HTML history not decoded | `.adal/skills` |

## Integration commands

```sh
# Inspect an entry; this does not install an agent
creditia agents --agent pi --json

# Preserve arguments, input, output and exit code; the child inherits the identity
creditia run --agent pi -- pi

# Declare identity for the next commit; stamp does not install a hook
creditia stamp --agent pi

# Install the skill from a local development checkout
# Replace /path/to/creditia with this repository's path
npx skills add /path/to/creditia --skill creditia -a pi

# For tools with a confirmed ACP command, wrap that command in its ACP client
creditia capture --agent <ID> -- <original-acp-command>
```

`run` does not capture logs, produce edit evidence or install hooks. An explicit `--model` declares a known execution model; Creditia does not infer it from the brand, default settings or a parent agent's environment. Arguments after `--` go to the original command.

ACP is the communication protocol between a client and an agent. The generic collector accepts every registered identity, but line-level evidence requires the tool to support ACP and provide successful edit diffs or file-write results. An unverified ACP launch command does not establish product support. Confirmed commands and format limits are recorded in [ADAPTERS.md](ADAPTERS.md).

## Native history audits

Current real-history validation is documented in [SUPPORT_NATIVE_LIVE.md](SUPPORT_NATIVE_LIVE.md) and [SUPPORT_MCPJAM_LIVE.md](SUPPORT_MCPJAM_LIVE.md). Remaining execution modes and account checks are tracked in [Issue #110](https://github.com/WroughtMind/creditia/issues/110). Earlier research and Lingma validation are in [SUPPORT_AGENT3.md](SUPPORT_AGENT3.md). The initial Eve implementation is in [SUPPORT_AGENT4.md](SUPPORT_AGENT4.md). The earlier Rovo Dev/Lingma readers are documented in [SUPPORT_AGENT6.md](SUPPORT_AGENT6.md). The earlier Firebender/Moxby readers are in [SUPPORT_AGENT8.md](SUPPORT_AGENT8.md). The previous seven readers, paths, receipts, deduplication and model boundaries are documented in [SUPPORT_AGENT18.md](SUPPORT_AGENT18.md). The earlier 13 paths remain in [SUPPORT_AGENT31.md](SUPPORT_AGENT31.md). The following readers were implemented in an earlier pass.

- **Hermes Agent:** reads `HERMES_HOME/state.db` (default `~/.hermes/state.db`; Creditia override `CREDITIA_HERMES_AGENT_DIR`). It pairs `messages.tool_calls` with tool receipts and extracts added lines from actual successful `patch` diffs and confirmed targets. Inactive sessions, inherited messages, unchanged files, failures and unpaired receipts are excluded. The mutable session-end model is not used for line attribution. Official sources: [state schema](https://github.com/NousResearch/hermes-agent/blob/main/hermes_state_common.py), [messages](https://github.com/NousResearch/hermes-agent/blob/main/hermes_state_messages.py), [file tools](https://github.com/NousResearch/hermes-agent/blob/main/tools/file_tools.py).
- **OpenClaw:** reads `OPENCLAW_STATE_DIR/agents/*/sessions/openclaw-agent.sqlite` and older session JSONL under the default `~/.openclaw` directory (Creditia override `CREDITIA_OPENCLAW_DIR`). JSON and zstd payloads in `transcript_events` are supported. Only paired, successful `edit` receipts containing actual unified patches produce lines; the model comes from the corresponding assistant message. Older logs without diffs provide sessions only. Current databases take precedence over the same agent's older JSONL. ACP-owned sessions, inherited branch records, superseded records and conflicting copies do not produce contributions. Format source: [pinned OpenClaw revision](https://github.com/openclaw/openclaw/tree/1197ee98bb5eab98f61befcbeafba382aa382e8b), including `openclaw-agent-schema.sql`, `transcript-payload-read.ts` and `sessions/tools/{edit,file-diff}.ts`.
- **Qoder CN:** adds the separate `~/.qoder-cn/projects/**/*.jsonl` location, respects the official `QODERCN_CONFIG_DIR` and offers `CREDITIA_QODER_CN_DIR`. International `~/.qoder` records remain separate. Only paired successful edits confirmed by the existing reader count. [Official session documentation](https://docs.qoder.cn/en/cli/sessions) confirms the location and environment variable; real-account acceptance remains unverified.

SQLite databases are opened read-only and read within a transaction for a consistent snapshot. Complete rereads of Hermes and OpenClaw sessions can revoke invalidated stored edits; read failures are not treated as successful empty reads. Other local readers, official cloud imports and ACP collection were integrated from an earlier worktree. Their sources, format limits and original verification dates remain in [ADAPTERS.md](ADAPTERS.md) and the `SUPPORT_*.md` files.

## Verification boundary

Integration checks on 2026-10-26: **362 Rust workspace tests passed, 3 ignored by default**, with strict Clippy passing; 38 renderer, 38 desktop frontend and 17 CLI/packaging/download tests passed. The bilingual site, embedded App and desktop frontend built successfully. An actual npm tarball installed outside the repository generated a credits document and PNG. Isolated Chromium checks covered both languages, desktop/mobile and light/dark in eight combinations: 77 monochrome Hero marks, no page overflow or console errors. The installed native App was not reinstalled or accepted in this pass.

The official MCPJam launch chain with Codex 0.164.0 was rerun in attended and unattended modes after the final fixes: each produced 5 verified lines, unchanged on rescan, with native source digests unchanged. The model endpoint was scripted. Earlier real Eve and inference.sh execution acceptance remains in [SUPPORT_NATIVE_LIVE.md](SUPPORT_NATIVE_LIVE.md); MCPJam details are in [SUPPORT_MCPJAM_LIVE.md](SUPPORT_MCPJAM_LIVE.md).

- Isolated repositories and records in official formats exercised the real CLI `run`, `whoami` and `stamp` paths for all 78 entries. Identity, blank unknown models and the absence of implicit hook installation were checked. Unknown IDs and conflicting arguments are rejected.
- Eve 0.83.0 official local source sandbox passed real model execution, durable write/readback and repository attribution: 3 verified lines, idempotent rescans and unchanged native files. Virtual-only writes are excluded; GitHub/eve-code coverage remains separately conditional.
- A pinned, unmodified Lingma SQLite database and 214-line native snapshot passed through the CLI, durable evidence and final credits: 119 verified and 95 declared lines, unchanged on repeat analysis. The comparison Git repository was reconstructed; the Windows client was not run. Previous Rovo validation remains in [the earlier pass](SUPPORT_AGENT6.md#hero-与验证).
- Firebender/Moxby native databases were checked through the CLI, durable evidence and final credits, including subagents, forks, failures/deletions, corrupt data and stale/truncated/foreign results. See [the previous audit](SUPPORT_AGENT8.md#验证与-hero).
- `run` checks cover argument preservation, standard input/output, standard error, exit codes and isolation from parent-agent identity.
- Integration review added real CLI/persistent-library regressions: separate Claude main/subagent identities and old-library migration; preserved durable evidence for incomplete Codex and shared native histories; native-progress snapshot ordering with warnings for equal-progress conflicts. Shared path boundaries exclude traversal and resolvable external symlinks without borrowing the process cwd for unbound paths.
- Hermes, OpenClaw and Qoder CN were checked through native-format records, readers, persistent storage and the final credits archive. Repeated analysis does not increase evidence. Failed or inactive records revoke stored evidence, and older OpenClaw JSONL does not revive revoked contributions.
- Both renderer families were checked for catalogue names, proportions, badges and classic credit rolls. New brand SVG sources match the generated data. Missing logos do not remove authors: names and initials remain visible.
- These include source, constructed records and selected original public histories, **not acceptance tests across 78 real product accounts, Windows/Linux or the installed App**. No release was published and no real agent logs were modified.

## Remaining work

CodeMaker remains unconfirmed and needs an account for positive execution acceptance. Eve local source sandbox is verified; inference.sh / MCPJam Codex modes now have implemented reading paths. Standalone cloud tools, other harnesses and a full MCPJam account run remain unverified. Ona, Loaf and PromptScript still attribute through their executing host. See [SUPPORT_NATIVE_LIVE.md](SUPPORT_NATIVE_LIVE.md).


The existing hero strip now contains **77 sourced brand marks**, including official Kode, Lingma, Rovo Dev, Eve, inference.sh and MCPJam marks, all painted with the page foreground. Editor, CLI and regional variants of the same brand share a mark. The mobile strip scrolls horizontally. A logo does not imply complete integration. See [asset provenance](site/assets/agents/SOURCES.md).
