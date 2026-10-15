# creditia 怎么归因

[English](ATTRIBUTION.en.md)

这份文档说明 creditia 如何判断「这一行是谁写的」。原则：**不调用大模型；证据和推断分开记、分开显示；每个结论都能复查**。

要说清楚的是：默认口径（standard）**会做两种推断**——同一改动块内的延伸、分支前缀；它们都记为「推断」，不会混进逐行证据或明确声明。只想要有证据的结论，用 `strict`。

## 1. 证据从哪里来

| 来源 | 内容 | 证据等级 |
|---|---|---|
| agent 日志 | 每次**成功**写进文件的内容（Write、Edit、FileChange、write_file、replace、patch……），连同会话、模型、时间、渠道。读得到的有 Claude Code、Codex、Cursor、Gemini CLI、Qwen Code、OpenCode、Cline、Roo Code、Kilo Code（扩展与命令行）、Grok、Kimi、GitHub Copilot（VS Code 与命令行）、Aider、Amp、TRAE 命令行、Crush、Goose、Continue、Zed、Amazon Q 命令行、Antigravity 命令行；报错、被拒绝、没应用的编辑都不算 | 逐行证据（verified） |
| agent 的 shell 命令 | 成功执行的命令原文（heredoc、`printf`、python 脚本里的新文本），作为命令里提到的仓库文件的候选证据（Claude Code 的 Bash、Codex、Amp、TRAE 命令行、Goose、Continue、Zed、Amazon Q 命令行、Antigravity 命令行的 shell） | 逐行证据 |
| Cursor 行级库 | Cursor 自己记下的逐行哈希。creditia 复现了它的规范化与哈希算法，逐字节一致 | 逐行证据 |
| 提交 trailer | `Creditia-Harness` / `Creditia-Model`（`creditia stamp` 或 hook 写入） | 明确声明（declared） |
| 提交元数据 | agent 的提交邮箱（Cursor 云端、Copilot、Devin、Jules…）、`Co-authored-by`、`Assisted-by: 工具:模型`（Crush 默认写它，Linux 内核的 AI 署名也是这个格式；写人名、邮箱的不算）、bot 账号、aider 改写的作者名 | 明确声明 |
| 其他人的提交 | 不是你的提交身份 → 算那个人 | 明确声明 |
| 分支前缀 | `codex/`、`cursor/`、`copilot/`… 由工具自动创建的分支 | 推断（inferred） |
| 时间窗口 | 提交时恰好有某个工具的会话在进行 | 推断（只在 loose 口径） |

证据库只存「相对路径、行哈希、工具、模型、会话、时间、渠道」，**不存代码、不存 prompt**。

## 2. 一行是怎么对上的

1. 读 `git log -p`，对每个提交的每一个新增行做规范化（去空白、统一引号等）再哈希，键是 `相对路径 + 规范化后的行`。
2. 在证据库里找同路径、同哈希、时间在提交前 14 天到提交后 1 小时之内的证据；多条命中取时间最接近的。
3. 对上了就是**逐行证据**。没对上的行按下面的降级链处理。

证据只记 agent **这次新写的行**，一次写入的一行只能被一个提交认领：

- 编辑时模型会带上几行没改的上下文，覆盖写入时会把整个文件重写一遍。Claude Code 的工具结果里有 `structuredPatch`，只取其中新增的行；没有工具结果的旧日志和别的工具，从新文本里减掉旧文本里已有的行；
- 提交按时间先后认领证据。Codex 写过三次 `return Ok(());`，你后来又亲手敲了一次，这一次不会再算给 Codex（Cursor 的行哈希表一行只存一次，不按次数算）。

过短的行（`}`、空行）哈希起来谁都一样，不能单独当证据。证据库给它们记的是位置：一次写入里，这个 `}` 跟在哪一行后面第几个、在哪一行前面第几个。提交里同样位置上的短行对上了，就跟着那次写入走；对不上的，跟随同一改动块里它上面最近的长行。

存活代码用 `git blame` 的原行号，对回引入它的那个提交里的同一行。

因为最后一步要求「行的内容出现在真实提交里」，所以：

- 日志里被拒绝、报错的编辑不算证据：Claude Code 看工具结果的 `is_error`，Cline / Roo / Kilo 要在对话记录里找到「写入成功」的结果，Gemini CLI 看 `status`，OpenCode 看 `completed`；
- 后来被删掉的编辑，不会凭空变成署名；
- 路径匹配可以宽松一些（多个 worktree、临时克隆），误认的风险由内容匹配兜底；
- 改写历史（rebase、squash、cherry-pick）不影响结果：行的内容和作者时间都还在。

## 3. 没对上的行：降级链与口径

| 步骤 | strict | standard（默认） | loose |
|---|:-:|:-:|:-:|
| 盖章 trailer | ✓ | ✓ | ✓ |
| agent 提交邮箱、Co-authored-by、Assisted-by、bot、aider 作者名 | ✓ | ✓ | ✓ |
| 其他人的提交 → 那个人 | ✓ | ✓ | ✓ |
| 同一改动块里大半有某个 agent 的逐行证据 → 块里余下的行跟随它（推断） | | ✓ | ✓ |
| 提交时有会话在进行（推断） | | | ✓ |
| 分支前缀（推断） | | ✓ | ✓ |
| 以上都没有 → **未确认** | ✓ | ✓ | ✓ |

**改动块**是 diff 里一段连续的新增行。块里有逐行证据的行至少占一半、其中至少八成是同一个工具，余下的行才跟着它；推断出来的行不再当证据去带别的行。有一种行例外：夹在两行逐行证据之间，而那两行在 agent 的同一次写入里本来是紧挨着的。这几行是后来插进去的，不是那次写入写的，不跟着推断。所以 agent 写的一段代码里被格式化拆开、被你改了几个字的行，仍然算它的；同一个提交里你另写的一个文件、另起的一段，不会被一行证据带走。模型也一样：块里的证据八成以上是同一个模型才写模型，否则只写工具。提交带声明（比如 Claude Code 默认加的 `Co-authored-by`）时，只在块的工具和声明一致时推断，用来补上声明里没有的模型。

口径写在 `.creditia/config.json` 的 `level`，或者 `creditia analyze --level strict`。每个贡献方在 `credits.json` 里都按证据等级拆分了行数，展示里也会标出「逐行证据 xx%」。

## 4. 项目身份：一个项目在好几处

一个仓库常常同时出现在：主目录、多个 worktree、agent 自动建的临时克隆（`/tmp/repo-task-123`）、另一台电脑、搬家前的旧目录、Docker / WSL 里。creditia 这样处理：

- **身份**：根提交 + 远程地址。同一仓库的克隆、worktree、搬过的目录认作同一个项目；根提交相同但远程完全不同的（fork）分开记。
- **路径登记**：每次在某个目录里运行（包括 agent 在 worktree 里代你运行），这个目录就登记进作品库。目录以后被删了，日志里留下的路径仍然认得出。
- **手动关联**：`creditia link <路径>` 关联别的电脑上的路径或已经不存在的目录（会检查：目录还在的话必须是同一个仓库）；`creditia paths` 列出全部。配置里的 `paths` 也一样。
- **路径映射**：日志里的绝对路径取「能在仓库历史里找到的最长后缀」作为仓库内相对路径，所以不同电脑、不同用户名、Windows 反斜杠都能对上。
- **多台电脑**：`creditia evidence export` / `import` 合并证据库（只含哈希与元数据），根提交不一致会拒绝。
- **Gemini CLI / Qwen Code** 旧版的目录名是项目路径的 SHA-256，没有明文路径：用上面登记过的全部路径逐个算哈希去对；新版 Gemini CLI 的目录是项目名，里面的 `.project_root` 写着项目路径，直接读。

## 5. 模型与渠道（含中转站）

- **名称归一**：`anthropic/claude-opus-5.5`、`openrouter/anthropic/claude-opus-5.5`、`us.anthropic.claude-…-v1:0`、`claude-…@20250514`、`[逆向]claude-…-thinking` 都归到同一个官方 id；Claude 去掉日期后缀，GPT 保留官方的点号写法。
- **厂商**：按名字规则判断（OpenAI、Anthropic、Google、xAI、Moonshot、DeepSeek、阿里、智谱、MiniMax、字节、Mistral、Meta、Cursor）。
- **渠道**（谁提供的模型）只在有证据时才写：
  - Claude Code：Bedrock / Vertex 返回的消息 id 有固定前缀；不是 `msg_` 开头说明经过了改写响应的中转站（记为 `relay`）。透传官方响应的中转站看不出来，就不写。
  - Codex：rollout 里的 `model_provider`。
  - OpenCode：每条消息的 `providerID`（包括你自定义的中转站名）。
  - Cline / Roo / Kilo：`model_provider_id`。
  - 模型串本身的前缀（`openrouter/`、`bedrock/`、`vertex_ai/`）。

## 6. 人

作品只回答一个问题：哪些 AI 工具和模型写了多少。人不和 AI 工具排同一个百分比，也不出现在任何作品里：百分比只在 AI 写的行和「未确认」的行之间算，作品里不写人名、不写人写了多少。本机档案 credits.json 的 `human` 里仍然记着下面这些，只供你自己查看：

- **发给 AI 的消息**：会话里人发出的消息数（各工具日志里的 user 消息）；
- **review**：GitHub PR 上你本人的 review（`--github`），bot 的 review 单独计；
- **人写的行**：别人的提交算那个人，Tab 补全的行也算人；你自己的提交里没有任何 AI 证据的行记为「未确认」，因为没法确定那是你写的还是没留下日志的 AI 写的；
- **AI 写的代码去了哪**：AI 在会话里写的行 → 提交了的行 → 现在还在的行；
- **Tab 补全**：Cursor 的 Tab 补全算人的辅助输入，不算 agent。

配置里的 `display.named`、`display.sign` 只保留在档案里，作品不会因此写出人名。

## 7. 不计入的内容

- lockfile（npm、yarn、pnpm、bun、Cargo、go.sum、poetry、uv、Gemfile、composer、Podfile、pubspec、flake）、`dist/`、`build/`、`node_modules/`、`vendor/`、`third_party/`、`.min.`、protobuf 生成物；
- `.gitattributes` 里标了 `linguist-generated`、`linguist-vendored`、`-diff`、`binary` 的路径；
- 过短的行（规范化后不到 4 个字符）不单独按内容作证据：按位置对上一次写入（见第 2 节），对不上的跟随同一改动块里它上面最近的长行；
- 二进制文件只计数，不计行。

## 8. 已知的边界

- 人改过几个字的 agent 行、被格式化工具拆开或合并的行，哈希就变了：不算逐行证据。周围大半是 agent 的逐行证据时，按改动块推断算它的；整段都被改过就记「未确认」。
- 你在 agent 写的一段代码中间插进去的几行，如果正好插在 agent 一次写入里相邻的两行之间，记「未确认」；如果你同时也改了旁边 agent 的行，就分不出来，会跟着那一段推断给 agent。
- agent 用 shell 命令写文件时，内容写在命令里的（`cat > file <<EOF`、`printf`、python 脚本里的新文本）按逐行证据算：命令原文的每一行只有真出现在那个文件的提交新增行里才算数，只读文件的命令（`cat`、`grep`）不会带来误认。内容是程序生成的（脚手架、代码生成脚本、`curl > file`）记「未确认」：那不是模型写的，和生成物一样不署给 AI。
- 日志被工具自动清理后，只要之前跑过一次 `analyze` / `collect`，证据库里还留着；从没采集过的就只剩提交元数据。
- 透传官方响应的中转站，渠道无法确定，不写。
- 有些工具的本地记录读不出编辑：Windsurf 的 Cascade 记录是加密的；TRAE IDE 的格式没有公开；2026 年 3 月以后的 Amp 把线程存在服务器上。它们只能靠提交元数据或盖章，`creditia doctor` 会标「不支持」。TRAE 命令行按它公开的 rollout 格式读，还没在大量真实日志上验证过；OpenCode 新版的 SQLite（`opencode.db`）和 Kilo 命令行的 `kilo.db` 按源码读，也还没用真实日志核对过；Crush 按它的源码读（`files` 表逐版存的文件内容，用户在两次编辑之间手改后存的中间版本不算），也还没用真实日志核对过；Goose 按它的源码读 `sessions.db`（只认工具结果成功的调用），同样还没用真实日志核对过。Continue 也按源码读（只认状态为 done 的工具调用）；它的记录没有逐条时间，证据时间取会话的创建时间，模型是用户在配置里给模型起的标题。Zed 也按源码读（`threads.db` 里 0.3.0 格式的线程，编辑结果带改前改后的完整内容，报错的不算）；线程里同样没有逐条时间，模型按线程记。Amazon Q 命令行也按源码读（`data.sqlite3` 的 conversations 表），它每个目录只存最近一次对话，之前的只有分析过才留在证据库里。Antigravity 命令行（`agy`，Google 用来接替 Gemini CLI）闭源，格式没有公开，只能按第三方工具对真实日志的观察来读（`brain/<对话>/.system_generated/logs/transcript_full.jsonl`），还没用真实日志核对过；它的工具调用没有 id，按先后和结果对上，结果不是 DONE 的不算；日志里不记模型 id，模型取对话里「Model Selection」设置变更写的界面名字，没换过模型的对话不知道模型。Antigravity 编辑器的对话存成 `.pb`，读不了。
- Copilot 在 VS Code 里的 agent 记录不一定带模型名：同一会话的 chatSessions 或调试日志里有就用，都没有就只记工具。
- Aider 的历史文件没有逐次编辑的时间：有「Commit」行的用那次提交的时间，没有的按仓库最后一次提交的时区估。
- 各工具的日志格式会变：`creditia doctor` 会列出每个数据源读到了多少、最早最晚是哪天；读不懂的格式会报警告，不会默默算错。

## 9. 准确率基准

`crates/cli/tests/bench.rs` 模拟一个项目的开发过程，每一行是谁写的都已知：几个 agent 各写各的、编辑带上下文、整文件覆盖写入、混合提交、格式化拆行、人改几个字、盖章、云端分支、同一工具两个模型、中文文档、日志丢失、重命名、同文件重复的常见行、多个 agent 交替改同一文件、agent 用 shell 写文件（heredoc、python 脚本）、生成脚本的输出、agent 只读人写的文件、人在 agent 代码中间插行、Aider 与 Copilot 的编辑、Copilot 报错的编辑。用真实二进制分析，逐行对答案（生成脚本的输出按约定不算给 AI，和人写的行一起算在「不该算给 AI 的行」里）：

| | 工具对 | 模型对 | 错给别的 AI | 漏掉 | 人写或生成的行被算给 AI | 份额总偏差 |
|---|--:|--:|--:|--:|--:|--:|
| 当前 | 96.4% | 96.3% | 0.1% | 3.6% | 0% | 2.5 个百分点 |

「漏掉」几乎都来自日志丢失这个找不回来的场景；剩下的偏差是几个 `}` 和同一工具两个模型交替写时的模型。CI 每次都跑，结果变差测试就失败。`cargo test -p creditia-cli --test bench -- --nocapture` 看分场景的表格。

## 10. agent 代你使用

- 所有命令非交互、支持 `--json`，退出码：0 成功、1 失败、2 有警告；
- `creditia whoami` 识别当前运行在哪个 agent 里；`creditia stamp --harness <工具> --model <模型>` 给下一次提交盖章；`creditia hook install` 之后在 agent 里提交会自动盖章；
- 在 worktree、临时克隆里运行，结果写回主仓库的档案，路径登记进同一个项目；
- 公开任何内容（写 CREDITS.md、改 README、提交）之前，skill 要求 agent 先让你确认。
