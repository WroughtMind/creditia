<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset=".github/readme/logo-dark.svg">
  <img src=".github/readme/logo-light.svg" width="52" alt="">
</picture>

# creditia

**Which AI wrote your code.**

Reads your local git history and agent logs, matches every line to the tool and model that wrote it,<br>
and turns the result into badges, CREDITS.md, end credits and share cards. Runs locally, calls no model.

[![built with](https://wroughtmind.github.io/creditia/assets/usage/en-self-badge.svg)](https://wroughtmind.github.io/creditia/self/en/)

[Website](https://wroughtmind.github.io/creditia/) · [Install](#install) · [Attribution rules](ATTRIBUTION.en.md) · [中文](README.md)

</div>

<br>

<picture>
  <source media="(prefers-color-scheme: dark)" srcset=".github/readme/map-dark-en.svg">
  <img src=".github/readme/map-light-en.svg" alt="Code map of the sample project lumen: one block per file, one stroke per line, coloured by the agent that wrote it">
</picture>

## One command

```text
$ creditia analyze && creditia fetch
lumen  2026-06-02 → 2026-09-21
───────────────────────────────
Claude Code     ███████████████░░░░░░░░░░  59.3%  Claude Opus 5.5
Codex           ███████░░░░░░░░░░░░░░░░░░  27.6%  GPT-5.6 Sol
Cursor          ██░░░░░░░░░░░░░░░░░░░░░░░   6.7%  Composer 2
Kimi            █░░░░░░░░░░░░░░░░░░░░░░░░   3.1%  Kimi K3
Grok            █░░░░░░░░░░░░░░░░░░░░░░░░   1.3%  Grok 5 Code
Unattributed    █░░░░░░░░░░░░░░░░░░░░░░░░   2.0%

commits 612  lines 49,360  tokens 491M
```

Once analyzed, put the result to use:

```bash
creditia doc                                  # write CREDITS.md (only its marked block)
creditia export --piece roll --out roll.png   # end credits
creditia export --list                        # every piece, layout, style and format
creditia hook install                         # stamp future commits, so they count even after logs are gone
creditia release install                      # release credits: credits in release notes and PR comments
creditia blame src/core/analyze.ts:12         # which tool and model wrote this line
```

One project in several places (worktrees, scratch clones, another machine): `creditia paths` lists the registered paths, `creditia link <path>` adds one.

Pieces list one row per tool by default. When one tool wrote nearly everything and switched models along the way, they switch to one row per model on their own. Pick with `--group tool` / `--group model`, or flip it in the App's top bar and go back to automatic any time. When one tool used several models, its model rows sit next to each other in shades of the same brand color. Each kind of piece also has a layout that draws every tool with its models underneath: "split" and "grouped" bars, the "nested" donut, the "troupe" credits roll, the "itemized" receipt, "blocks" waffle, "split" badge and the "with models" byline (marked with a small dot in the App).

Commit stamps and release credits both change your repository, so they are your call, per project: `init` / `analyze` ask once in a terminal (the answer goes into `.creditia/config.json` and isn't asked again), and the App shows both switches at the top of each project's wall, ready to flip back any time. `creditia integrations` shows where they stand.

## Every line has a source

- **Matched in logs** — The tool's own log recorded writing this line: Claude Code Write / Edit, Codex FileChange, Cursor line hashes…
- **Declared in commits** — Commit email, `Co-authored-by`, `Assisted-by`, creditia's commit stamp
- **Inferred** — When most of a contiguous change has line-level evidence from one tool, the rest of that change follows it; branches tools create themselves, like `codex/`. Kept separate; turn off with `--level strict`
- **Unattributed** — Anything that matches none of the above. Counted toward no tool

creditia answers one question — which AI tools and models wrote how much. No names appear in any piece, and percentages are taken over AI and unattributed lines only. Full rules and known limits: [ATTRIBUTION.en.md](ATTRIBUTION.en.md).

## Twenty-six looks

<img src=".github/readme/styles-en.png" alt="The same credits in different styles: cinema end credits, ukiyo-e share card, woodblock vertical text, manga stat panels, cyber HUD, neon donut, broadsheet version bars, grimoire ticket, terminal fetch, baroque podium">

One record, more than twenty pieces (leaderboard, share card, end credits, code map, evidence, areas, receipt, badge, timeline ribbon, activity clock…) × twenty-six styles, out as PNG, SVG, PDF, GIF or HTML. Each style is a full visual language, not a palette swap, and some pieces get their own layouts in some styles: vertical woodblock print, manga stat panels, a cyber HUD, a dozen ways to roll end credits.

> Minimal · Minimal Dark · Glass · Film · Cinema · Editorial · Broadsheet · Playbill · Gala · Mono · Poster · Terminal · CRT · Neon · Arcade<br>
> Ink Wash · Ukiyo-e · Woodblock Book · Baroque · Grimoire · Manuscript · Symmetry · Cut-Paper Titles · Summer Meadow · Manga · Cyber

See them all side by side on the [website's style wall](https://wroughtmind.github.io/creditia/#looks).

## Four ways to use it

**Command line** — everything above. Every command takes `--json`.

**Your agent** — install [skills/creditia](skills/creditia/SKILL.md) into your agent and ask it which AIs wrote this repo. It analyzes and renders step by step, and asks you before publishing or committing anything.

**Cross-project summary** — with several projects in the library, the App's project menu gets an "All projects" entry at the top: their credits merged into one, and the whole wall draws as usual. On the command line it's `creditia summary` (prints a card; `--out all.json` writes a credits file), or add `--all-projects` to any command, e.g. `creditia export --all-projects --piece tier`. Line counts add up; versions belong to each project and aren't merged; files and PRs carry their project's name.

**Privacy check before publishing** — before `creditia publish` writes credits.json, it lists every kind of thing that becomes public (PR / commit titles, file paths, people's names, working hours, repository URLs) with a few examples, and flags anything that looks private (secret-like strings, emails, local paths, internal hosts, URLs with credentials). In a terminal it asks per category, and a category with something suspicious defaults to private. `--check` only checks, `--hide titles,paths` and the like hide directly; in scripts it refuses to write when something looks private unless you add `--yes`. Hiding only changes the published copy, never your local record.

**GitHub Action** — CI can't see anyone's agent logs, so analysis runs locally: `creditia publish` writes the record to `credits.json` at the repo root; commit it. On every push the Action redraws the credits page from it, and can also write a badge (`badge:`) and CREDITS.md (`doc:`). It can put each release's credits into its release notes (`release-notes: true`) and comment a PR's credits on it, updated on every push (`pr-comment: true`); `creditia release install` writes that workflow for you. Those two don't need credits.json: with commit stamps on, every commit carries a `Creditia-Lines` trailer (lines per tool, computed locally at commit time, no code), and that's all CI reads.

**Dynamic badge** — the credits site (`site:`) always includes a `badge.json`. Publish it to GitHub Pages and put `![credits](https://img.shields.io/endpoint?url=https://<user>.github.io/<repo>/badge.json)` in your README; the badge follows every push. On its own: `creditia badge-json --out badge.json`, or `badge-json:` in the Action. Without credits.json it sums the stamps on all commits.

```yaml
# .github/workflows/credits.yml
on: { push: { branches: [main] } }
permissions: { contents: read, pages: write, id-token: write }
jobs:
  credits:
    runs-on: ubuntu-latest
    environment: github-pages
    steps:
      - uses: actions/checkout@v4
      - uses: WroughtMind/creditia@main
        with: { site: _site, style: ink, lang: en }
      - uses: actions/upload-pages-artifact@v3
        with: { path: _site }
      - uses: actions/deploy-pages@v4
```

**App** — one wall: switch projects at the top, styles with the dots at the bottom left, and every piece is laid out at once. Hover a piece to change its layout or export it, hover any part of it for details, or drag it onto the export dock. [Try it on the website](https://wroughtmind.github.io/creditia/#use).

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="site/app-dark.webp">
  <img src="site/app-light.webp" alt="The creditia app: every piece for the sample project laid out on one wall">
</picture>

## Supported tools

| | Tool | What it reads | Lines | Model |
|:-:|---|---|:-:|:-:|
| <img src=".github/readme/logo/claude-code.svg" width="18"> | Claude Code | Write / Edit / MultiEdit in `~/.claude/projects/*/*.jsonl`; providers incl. Bedrock, Vertex, relays | ✓ | ✓ |
| <img src=".github/readme/logo/codex.svg" width="18"> | Codex | `~/.codex` thread store + rollout FileChange; provider from `model_provider` | ✓ | ✓ |
| <img src=".github/readme/logo/cursor.svg" width="18"> | Cursor | `ai-code-tracking.db` line hashes, byte-for-byte the same as Cursor's own | ✓ | ✓ |
| <img src=".github/readme/logo/gemini.svg" width="18"> <img src=".github/readme/logo/qwen.svg" width="18"> | Gemini CLI · Qwen Code | Successful write / replace in `~/.gemini/tmp/<project>/chats` (`.json` and the newer `.jsonl`); newer Qwen Code's `~/.qwen/projects/*/chats/*.jsonl` | ✓ | ✓ |
| <img src=".github/readme/logo/opencode.svg" width="18"> | OpenCode | Completed edit / write / patch in `opencode.db` (the SQLite store of newer versions, both its v1 and v2 tables) and the older `storage/*.json`; provider from `providerID` | ✓ | ✓ |
| <img src=".github/readme/logo/cline.svg" width="18"> <img src=".github/readme/logo/roo.svg" width="18"> <img src=".github/readme/logo/kilo.svg" width="18"> | Cline · Roo Code · Kilo Code | `tasks/*/ui_messages.json` in each VS Code–family editor; the Kilo CLI's `kilo.db` (a fork of OpenCode) | ✓ | ✓ |
| <img src=".github/readme/logo/grok.svg" width="18"> <img src=".github/readme/logo/kimi.svg" width="18"> | Grok · Kimi | File-write calls in their session directories | ✓ | ✓ |
| <img src=".github/readme/logo/copilot.svg" width="18"> | GitHub Copilot | Successful edits in VS Code `chatSessions` and agent transcripts; Copilot CLI `~/.copilot/session-state` | ✓ | ✓ |
| <img src=".github/readme/logo/aider.svg" width="18"> | Aider | The repo's `.aider.chat.history.md`: only files confirmed by "Applied edit", timed by its commit | ✓ | ✓ |
| | Amp · TRAE CLI | Completed edits in Amp's local `threads/*.json`; TRAE CLI session rollouts | ✓ | ✓ |
| | Goose | `~/.local/share/goose/sessions/sessions.db`: successful write / edit / text_editor calls (paired with their tool results) and shell commands; model from the per-call usage ledger | ✓ | ✓ |
| | Amazon Q CLI | The conversations table in `amazon-q/data.sqlite3`: fs_write calls and commands whose result is Success; only the latest conversation per directory is kept | ✓ | ✓ |
| | Antigravity CLI | `agy`'s `~/.gemini/antigravity-cli/brain/*/…/transcript_full.jsonl`: write_to_file / replace_file_content calls and commands whose result is DONE (closed source; read from third-party observation) | ✓ | ✓ |
| | Zed | The agent panel's `threads/threads.db`: successful edit_file / write_file results carry full before / after contents, and their difference counts; terminal commands | ✓ | ✓ |
| | Continue | `~/.continue/sessions/*.json` (shared by the editor extensions and the `cn` CLI): edits and commands with status done; rejected diffs don't count | ✓ | ✓ |
| | Crush | The project's `.crush/crush.db`: lines added between consecutive file versions in the `files` table; `Assisted-by: Crush:<model>` in commits | ✓ | ✓ |
| <img src=".github/readme/logo/devin.svg" width="18"> <img src=".github/readme/logo/jules.svg" width="18"> | Copilot cloud · Devin · Jules · Cursor cloud · bots | Commit email, `Co-authored-by`, `Assisted-by`, author name | declared | — |
| | Any tool | `creditia stamp` / commit-hook stamp (`Creditia-Harness` trailer) | declared | ✓ |

## Install

Needs Node 22+. npm, Homebrew, one-line scripts and app installers are coming soon; for now, install from source:

```bash
git clone https://github.com/WroughtMind/creditia && cd creditia/packages/cli && npm i && npm link
# or just the Rust analysis core
cargo install --git https://github.com/WroughtMind/creditia creditia-cli
```

Agent skill (Claude Code):

```bash
git clone --depth 1 https://github.com/WroughtMind/creditia /tmp/creditia && cp -r /tmp/creditia/skills/creditia ~/.claude/skills/
```

The GitHub Action needs no install: use `WroughtMind/creditia@main`.

The command line speaks Chinese by default; add `--lang en` or set `CREDITIA_LANG=en` for English.

The first analysis of a large repository traces every current line back to its commit, which takes a while: a repo with 12,000 commits and 9,000 files takes tens of minutes on a single core, much less on more. The terminal and the app show which step it is on; cancel midway and the next run picks up where it stopped. After that, only new commits are analyzed.

<details>
<summary><b>Repository layout</b></summary>

```
crates/
  core/       attribution engine, line hashing, model catalog (name normalization, vendors, providers), config
  adapters/   readers for git and each agent's logs
  store/      library: projects, paths, evidence store, snapshots (SQLite)
  cli/        the creditia-core binary
packages/
  schema/     JSON Schema and types for credits.json v0
  render/     rendering: credits.json → SVG scene (style × piece) → PNG / SVG / PDF / GIF / HTML
  cli/        the creditia npm package: records, export, terminal card
  embed/      <creditia-card> web component
apps/desktop/ Tauri 2 app (wall and export)
site/         website (build.ts) and the images in this README (readme.ts)
skills/       skill for agents
spike/        phase-0 validation project (kept for the record)
```

</details>

<details>
<summary><b>Development</b></summary>

```bash
cargo build --release && cargo test           # Rust core (end-to-end tests use fake logs in a temp HOME)
cd packages/render && npm i && npm test       # rendering (every piece × style × layout)
npm run gallery                               # proof sheet: gallery/index.html
cd apps/desktop && npm i && npm run fonts && npm run dev   # app in the browser (uses the fixture record)
npm run core && npm run tauri dev             # native app (core puts the analysis binary where the app bundles it)
node site/build.ts                            # website → _site/
node site/readme.ts                           # README images → .github/readme/
```

The macOS app isn't signed yet: the first time you open a build, right-click → Open, or run `xattr -cr /Applications/creditia.app`.

</details>

## License

MIT, see [LICENSE](LICENSE). Logos come from simple-icons (CC0) and are used only to identify each tool.
