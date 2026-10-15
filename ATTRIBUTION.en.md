# How creditia attributes code

[中文](ATTRIBUTION.md)

This document explains how creditia decides who wrote a line. The principles: **no language model is called; evidence and inference are recorded and shown separately; every conclusion can be checked.**

To be clear up front: the default level (standard) **makes two kinds of inference**: extending through the same change block, and branch prefixes. Both are recorded as "inferred" and never mixed into line-level evidence or declarations. If you only want conclusions backed by evidence, use `strict`.

## 1. Where evidence comes from

| Source | What it holds | Evidence level |
|---|---|---|
| Agent logs | Every edit that was **successfully** written to a file (Write, Edit, FileChange, write_file, replace, patch…), with its session, model, time and provider. Readable for Claude Code, Codex, Cursor, Gemini CLI, Qwen Code, OpenCode, Cline, Roo Code, Kilo Code (extension and CLI), Grok, Kimi, GitHub Copilot (VS Code and CLI), Aider, Amp, TRAE CLI, Crush, Goose, Continue, Zed, Amazon Q CLI and Antigravity CLI. Edits that errored, were rejected or were never applied do not count | Line-level evidence (verified) |
| Agent shell commands | The text of commands that ran successfully (heredocs, `printf`, new text inside a python script), as candidate evidence for the repo files the command names (Claude Code's Bash, Codex, Amp, TRAE CLI, Goose, Continue, Zed, Amazon Q CLI and Antigravity CLI shells) | Line-level evidence |
| Cursor's line store | Cursor's own per-line hashes. creditia reproduces its normalization and hashing byte for byte | Line-level evidence |
| Commit trailers | `Creditia-Harness` / `Creditia-Model` (written by `creditia stamp` or the hook) | Declared |
| Commit metadata | An agent's commit email (Cursor cloud, Copilot, Devin, Jules…), `Co-authored-by`, `Assisted-by: tool:model` (Crush writes it by default, and it's the Linux kernel's format for AI credit; values naming a person or an email don't count), bot accounts, the author name aider rewrites | Declared |
| Other people's commits | Not your commit identity → credited to that person | Declared |
| Branch prefix | Branches a tool creates by itself: `codex/`, `cursor/`, `copilot/`… | Inferred |
| Time window | A session of some tool happened to be running at commit time | Inferred (loose level only) |

The evidence store keeps only "relative path, line hash, tool, model, session, time, provider". **It stores no code and no prompts.**

## 2. How a line is matched

1. Read `git log -p`. Every added line of every commit is normalized (whitespace removed, quotes unified and so on) and hashed; the key is `relative path + normalized line`.
2. Look up evidence with the same path and hash, timed from 14 days before the commit to 1 hour after it; with several hits, take the closest in time.
3. A match is **line-level evidence**. Lines without a match go down the fallback chain below.

Evidence records only the lines an agent **newly wrote in that edit**, and each line of one edit can be claimed by only one commit:

- When editing, a model sends a few unchanged lines of context, and a full overwrite rewrites the whole file. Claude Code's tool results include a `structuredPatch`, and only its added lines are taken; for older logs without tool results, and for other tools, lines already in the old text are subtracted from the new text.
- Commits claim evidence in time order. If Codex wrote `return Ok(());` three times and you later typed it once yourself, that fourth one does not go to Codex. (Cursor's line-hash table stores each line once, so it is not counted per occurrence.)

Very short lines (`}`, blank lines) hash the same for everyone and cannot be evidence on their own. For them the store records a position: within one edit, which line this `}` follows and by how many lines, and which line it precedes. A short line in the same position in a commit follows that edit; one that doesn't match follows the nearest long line above it in the same change block.

Surviving code uses `git blame`'s original line numbers to map each line back to the same line in the commit that introduced it.

Because the last step requires that "the line's content appears in a real commit":

- Edits that were rejected or errored in the logs are not evidence: Claude Code checks `is_error` in the tool result, Cline / Roo / Kilo need a "write succeeded" result in the conversation, Gemini CLI checks `status`, OpenCode checks `completed`;
- Edits that were later deleted never turn into credit;
- Path matching can be loose (several worktrees, temporary clones), because content matching guards against false matches;
- Rewriting history (rebase, squash, cherry-pick) doesn't change the result: the line contents and author times are still there.

## 3. Unmatched lines: the fallback chain and levels

| Step | strict | standard (default) | loose |
|---|:-:|:-:|:-:|
| Stamp trailer | ✓ | ✓ | ✓ |
| Agent commit email, Co-authored-by, Assisted-by, bot, aider author name | ✓ | ✓ | ✓ |
| Other people's commits → that person | ✓ | ✓ | ✓ |
| Most of the same change block has line-level evidence from one agent → the rest of the block follows it (inferred) | | ✓ | ✓ |
| A session was running at commit time (inferred) | | | ✓ |
| Branch prefix (inferred) | | ✓ | ✓ |
| None of the above → **unattributed** | ✓ | ✓ | ✓ |

A **change block** is a contiguous run of added lines in a diff. The rest of a block follows a tool only when at least half of its lines have line-level evidence and at least 80% of those belong to that one tool; inferred lines are never used as evidence to carry other lines. One kind of line is an exception: lines sitting between two lines of line-level evidence that were adjacent in the agent's single edit. Those were inserted later, not written by that edit, so they don't follow the inference. This way, lines in an agent's code that a formatter split up, or that you changed by a few characters, still count as the agent's, while a separate file or a new paragraph you wrote in the same commit isn't carried off by a single line of evidence. Models work the same way: a model is recorded only when at least 80% of the block's evidence comes from one model; otherwise only the tool is. When a commit carries a declaration (for example the `Co-authored-by` Claude Code adds by default), the block is inferred only when its tool matches the declaration, to fill in the model the declaration lacks.

Set the level with `level` in `.creditia/config.json`, or `creditia analyze --level strict`. In `credits.json` every contributor's lines are split by evidence level, and pieces also show "xx% line-level evidence".

## 4. Project identity: one project in several places

A repository often lives in several places at once: your main directory, several worktrees, temporary clones agents create (`/tmp/repo-task-123`), another computer, an old directory from before a move, inside Docker or WSL. creditia handles this as follows:

- **Identity**: root commit + remote URL. Clones, worktrees and moved directories of one repository count as one project; a fork with the same root commit but an entirely different remote is kept separate.
- **Path registry**: every directory you run in (including a worktree where an agent runs it for you) is registered in the library. If the directory is later deleted, paths left in the logs are still recognized.
- **Manual links**: `creditia link <path>` links a path on another computer or a directory that no longer exists (it checks that a directory that still exists is the same repository); `creditia paths` lists them all. `paths` in the config works the same way.
- **Path mapping**: for an absolute path in a log, the longest suffix found in the repo's history becomes the in-repo relative path, so different computers, different user names and Windows backslashes all match.
- **Several computers**: `creditia evidence export` / `import` merges evidence stores (hashes and metadata only); a different root commit is refused.
- **Gemini CLI / Qwen Code** name their directories with the SHA-256 of the project path, with no plain path: every registered path above is hashed and tried in turn.

## 5. Models and providers (including relays)

- **Name normalization**: `anthropic/claude-opus-5.5`, `openrouter/anthropic/claude-opus-5.5`, `us.anthropic.claude-…-v1:0`, `claude-…@20250514` and `[逆向]claude-…-thinking` all map to the same official id; Claude ids lose their date suffix, GPT ids keep the official dotted form.
- **Vendor**: decided by name rules (OpenAI, Anthropic, Google, xAI, Moonshot, DeepSeek, Alibaba, Zhipu, MiniMax, ByteDance, Mistral, Meta, Cursor).
- **Provider** (who served the model) is recorded only when there is evidence:
  - Claude Code: message ids returned by Bedrock / Vertex have fixed prefixes; an id not starting with `msg_` means a relay rewrote the response (recorded as `relay`). A relay that passes official responses through can't be told apart, so nothing is recorded.
  - Codex: `model_provider` in the rollout.
  - OpenCode: each message's `providerID` (including your own relay names).
  - Cline / Roo / Kilo: `model_provider_id`.
  - Prefixes in the model string itself (`openrouter/`, `bedrock/`, `vertex_ai/`).

## 6. People

Pieces answer one question only: which AI tools and models wrote how much. People are not ranked against AI tools and appear in no piece: percentages are taken only over AI-written and unattributed lines, and no piece shows names or how much people wrote. The `human` section of your local `credits.json` still records the following, for you alone:

- **Messages to AI**: how many messages people sent in sessions (user messages in each tool's logs);
- **Reviews**: your own reviews on GitHub PRs (`--github`); bot reviews are counted separately;
- **Lines by people**: other people's commits go to them, and Tab-completed lines count as people's too; lines in your own commits with no AI evidence are recorded as "unattributed", because there's no telling whether you wrote them or an AI that left no log did;
- **Where the AI's code went**: lines AI wrote in sessions → lines committed → lines still here;
- **Tab completion**: Cursor's Tab completion counts as assisted typing by a person, not as an agent.

`display.named` and `display.sign` in the config are kept in the record only; pieces never write names because of them.

## 7. What isn't counted

- Lockfiles (npm, yarn, pnpm, bun, Cargo, go.sum, poetry, uv, Gemfile, composer, Podfile, pubspec, flake), `dist/`, `build/`, `node_modules/`, `vendor/`, `third_party/`, `.min.`, protobuf output;
- Paths marked `linguist-generated`, `linguist-vendored`, `-diff` or `binary` in `.gitattributes`;
- Very short lines (under 4 characters after normalization) are not evidence by content alone: they match an edit by position (see section 2), and otherwise follow the nearest long line above them in the same change block;
- Binary files are counted, not their lines.

## 8. Known limits

- An agent's line that you changed by a few characters, or that a formatter split or merged, hashes differently and is not line-level evidence. When most of what surrounds it is the agent's line-level evidence, the change-block inference credits it to the agent; when the whole passage was rewritten, it's recorded as unattributed.
- A few lines you insert into the middle of an agent's code are recorded as unattributed if they sit exactly between two lines that were adjacent in one of the agent's edits; if you also changed the agent's lines next to them, they can't be told apart and follow the passage's inference to the agent.
- When an agent writes a file through a shell command, content written in the command itself (`cat > file <<EOF`, `printf`, new text inside a python script) counts as line-level evidence: a line of the command text counts only if it really appears among that file's added lines in a commit, so commands that only read files (`cat`, `grep`) can't cause false matches. Content produced by a program (scaffolding, codegen scripts, `curl > file`) is recorded as unattributed: a model didn't write it, and like generated files it isn't credited to AI.
- Once a tool cleans up its own logs, the evidence store still has them as long as `analyze` / `collect` ran once before; if they were never collected, only the commit metadata is left.
- For a relay that passes official responses through, the provider can't be determined and isn't recorded.
- Some tools' local records don't reveal edits: Windsurf's Cascade records are encrypted; TRAE IDE's format isn't public; Amp since March 2026 keeps threads on its servers. They can only be credited through commit metadata or stamps, and `creditia doctor` marks them "not supported". TRAE CLI is read from its published rollout format and hasn't been checked against many real logs yet. OpenCode's newer SQLite store (`opencode.db`) and the Kilo CLI's `kilo.db` are read from their source code and haven't been checked against real logs either. Crush is read from its source code (file contents stored version by version in the `files` table; the intermediate version Crush stores after you edit a file by hand between two agent edits doesn't count) and Goose from its source code (`sessions.db`, only calls whose tool result succeeded); neither has been checked against real logs yet. Continue is also read from its source code (only tool calls whose status is done); its records carry no per-message time, so evidence is dated to the session's creation, and models are the titles the user gave them in their config. Zed is also read from its source code (threads in the 0.3.0 format in `threads.db`; edit results carry full before / after contents, errors don't count); its threads carry no per-message time either, and the model is recorded per thread. Amazon Q CLI is also read from its source code (the conversations table in `data.sqlite3`); it keeps only the latest conversation per directory, so earlier ones survive only if they were analyzed before. Antigravity CLI (`agy`, Google's successor to Gemini CLI) is closed source with no published format, so it is read from what third-party tools have observed in real logs (`brain/<conversation>/.system_generated/logs/transcript_full.jsonl`) and hasn't been checked against real logs yet. Its tool calls carry no id, so they are paired with their results in order, and calls whose result isn't DONE don't count. The logs carry no model id: the model is the display name from "Model Selection" setting changes in the conversation, so conversations that never switched models have none. The Antigravity editor stores conversations as `.pb` and can't be read.
- Copilot's agent records in VS Code don't always carry a model name: it's used when the same session's chatSessions or debug log has it, otherwise only the tool is recorded.
- Aider's history file has no time per edit: entries with a "Commit" line use that commit's time; the rest use the time zone of the repo's last commit.
- Tools change their log formats: `creditia doctor` lists how much each source yielded and its earliest and latest day; formats it can't read raise a warning instead of being silently miscounted.

## 9. Accuracy benchmark

`crates/cli/tests/bench.rs` simulates a project's development with the author of every line known: several agents each writing their own parts, edits with context, full-file overwrites, mixed commits, formatter-split lines, people changing a few characters, stamps, cloud branches, one tool with two models, Chinese docs, lost logs, renames, common lines repeated within a file, several agents taking turns on one file, agents writing files through the shell (heredoc, python script), output of a generator script, an agent only reading a file a person wrote, a person inserting lines into agent code, Aider and Copilot edits, and a Copilot edit that errored. It analyzes with the real binary and checks every line against the answer (by convention, generator output isn't credited to AI and is counted together with people's lines as "lines that shouldn't go to AI"):

| | Right tool | Right model | Credited to another AI | Missed | People's or generated lines credited to AI | Total share error |
|---|--:|--:|--:|--:|--:|--:|
| Current | 96.4% | 96.3% | 0.1% | 3.6% | 0% | 2.5 points |

Almost all of "missed" comes from the lost-logs scenario, which can't be recovered; the remaining error is a few `}` lines and the model when one tool alternates between two models. CI runs it every time, and the test fails if results get worse. `cargo test -p creditia-cli --test bench -- --nocapture` prints the per-scenario table.

## 10. When an agent runs it for you

- Every command is non-interactive and takes `--json`; exit codes: 0 success, 1 failure, 2 warnings;
- `creditia whoami` detects which agent it is running inside; `creditia stamp --harness <tool> --model <model>` stamps the next commit; after `creditia hook install`, commits made in an agent are stamped automatically;
- Run in a worktree or temporary clone, results are written back to the main repo's record, and the path is registered under the same project;
- Before publishing anything (writing CREDITS.md, changing the README, committing), the skill requires the agent to ask you first.
