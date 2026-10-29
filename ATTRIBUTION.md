# creditia 怎么归因

[English](ATTRIBUTION.en.md)

这份文档说明 creditia 如何判断「这一行是谁写的」。原则：**不调用大模型；证据和推断分开记、分开显示；每个结论都能复查**。

要说清楚的是：默认口径（standard）**会做两种推断**——同一改动块内的延伸、分支前缀；它们都记为「推断」，不会混进逐行证据或明确声明。只想要有证据的结论，用 `strict`。creditia **不推断谁手写**：日志只能证明 AI 写了什么，证明不了人写了什么。

## 1. 证据从哪里来

| 来源 | 内容 | 证据等级 |
|---|---|---|
| agent 日志 | 每次**成功**写进文件的内容（Write、Edit、FileChange、write_file、replace、patch……），连同会话、模型、时间、渠道。读得到的有 Claude Code、Codex、Cursor、Gemini CLI、Qwen Code、OpenCode、Cline、Roo Code、Kilo Code（扩展与命令行）、Grok Build、Kimi Code CLI、GitHub Copilot（VS Code 与命令行）、Aider、Amp、TRAE 命令行、Crush、Goose、Continue、Zed、Amazon Q 命令行、Antigravity 命令行；报错、被拒绝、没应用的编辑都不算 | 逐行证据（verified） |
| agent 的 shell 命令 | 成功执行的命令原文（heredoc、`printf`、python 脚本里的新文本），作为命令里提到的仓库文件的候选证据（Claude Code 的 Bash、Codex、Amp、TRAE 命令行、Goose、Continue、Zed、Amazon Q 命令行、Antigravity 命令行的 shell） | 逐行证据 |
| Cursor 行级库 | Cursor 自己记下的逐行哈希。creditia 复现了它的规范化与哈希算法，逐字节一致 | 逐行证据 |
| 提交 trailer | `Creditia-Harness` / `Creditia-Model`（`creditia stamp` 或 hook 写入） | 明确声明（declared） |
| 提交 trailer `Creditia-Lines` | 提交盖章 hook 在提交那一刻按本机日志算出的各工具行数（`claude-code/claude-opus-5-5 30 / 120`；没有 AI 行写成 `/ 120`） | 明确声明（按行数） |
| 提交元数据 | agent 的提交邮箱（Cursor 云端、Copilot、Devin、Jules…）、`Co-authored-by`、`Assisted-by: 工具:模型`（Crush 默认写它，Linux 内核的 AI 署名也是这个格式；写人名、邮箱的不算）、bot 账号、aider 改写的作者名 | 明确声明 |
| 其他人的提交 | 不是你的提交身份 → 算那个人 | 明确声明 |
| 分支前缀 | `codex/`、`cursor/`、`copilot/`… 由工具自动创建的分支 | 推断（inferred） |
| 时间窗口 | 提交时恰好有某个工具的会话在进行 | 推断（只在 loose 口径） |
| 网页合并提交的正文 | 在 GitHub 网页上 squash 合并时，原分支各提交的盖章、`Co-authored-by` 被并进正文中间；读整个正文找回来，`Creditia-Lines` 加起来算 | 明确声明 |

证据库只存「相对路径、行哈希、工具、模型、会话、时间、渠道」，**不存代码、不存 prompt**。

## 2. 一行是怎么对上的

1. 读 `git log -p`，对每个提交的每一个新增行做规范化（去空白、统一引号等）再哈希，键是 `相对路径 + 规范化后的行`。
2. 在证据库里找同路径、同哈希、时间在提交前 14 天到提交后 1 小时之内的证据；多条命中取时间最接近的。写完很久才提交（分支放了几个星期）：更早的、还没被别的提交认领的证据也算，但要连着的两行在那次写入里也是紧挨着的，单独一行常见代码对上了可能是巧合，不算。
3. 对上了就是**逐行证据**。没对上的行按下面的降级链处理。

证据只记 agent **这次新写的行**，一次写入的一行只能被一个提交认领：

- 编辑时模型可能带上未改的上下文或覆盖整个文件。Claude Code 必须有配对的成功工具结果；有 `structuredPatch` 时只取新增行，成功回执没有差分时，Edit/MultiEdit 从新文本中减去旧文本。只有调用请求、缺少成功回执的记录不算；
- 提交按时间先后认领证据。Codex 写过三次 `return Ok(());`，你后来又亲手敲了一次，这一次不会再算给 Codex（Cursor 的行哈希表一行只存一次，不按次数算）。

过短的行（`}`、空行）哈希起来谁都一样，不能单独当证据。证据库给它们记的是位置：一次写入里，这个 `}` 跟在哪一行后面第几个、在哪一行前面第几个。提交里同样位置上的短行对上了，就是那次写入的逐行证据；对不上的，跟随同一改动块里它上面最近的长行，记为推断。

被格式化工具拆开的行：提交里连续 2～4 行（同一改动块内）规范化后拼起来，正好是 agent 一次写入里的一行，这几行都算那次写入的逐行证据。规范化本来就去掉了空白、逗号和分号，所以 prettier、rustfmt 把一个长调用折成几行、补上尾逗号，都拼得回去。

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
| `Creditia-Lines` 配额：工具现在算到的行数比提交时记下的少（日志后来被清理了），从上面不是 agent 的行里按顺序补足 | ✓ | ✓ | ✓ |
| 以上都没有 → **未确认** | ✓ | ✓ | ✓ |

**没有 AI 证据不等于人写的**：你自己提交里没对上 AI 证据的行记「未确认」，不推断为手写——代码可能是另一台电脑、云端会话里的 agent 写的，日志不在这台电脑上，也可能是 agent 写完被格式化工具并过、被人改过。要让未确认变少，办法是把证据补全：每台写代码的电脑都装上提交盖章 hook，另一台电脑的证据用 `evidence import` 导进来。

**盖章按行数算**：提交带 `Creditia-Lines` 时，盖章 trailer、`Co-authored-by`、`Assisted-by` 不再代表整个提交——提交那一刻已经逐行核对过日志，章只认它记下的行数，其余的行记未确认。所以在 agent 里提交一批自己手写的改动，不会再被整个算给 agent；日志后来被清理了，记下的行数也还在。没有 `Creditia-Lines` 的旧提交照旧按章算整个提交。网页 squash 合并的提交里，几条 `Creditia-Lines` 加起来比这个提交实际新增的多时（后面的提交改写了前面的行），按比例缩。

**改动块**是 diff 里一段连续的新增行。块里有逐行证据的行至少占一半、其中至少八成是同一个工具，余下的行才跟着它；推断出来的行不再当证据去带别的行。有一种行例外：夹在两行逐行证据之间，而那两行在 agent 的同一次写入里本来是紧挨着的。这几行是后来插进去的，不是那次写入写的，不跟着推断。所以 agent 写的一段代码里被格式化拆开、被你改了几个字的行，仍然算它的；同一个提交里你另写的一个文件、另起的一段，不会被一行证据带走。模型也一样：块里的证据八成以上是同一个模型才写模型，否则只写工具。提交带声明（比如 Claude Code 默认加的 `Co-authored-by`）时，只在块的工具和声明一致时推断，用来补上声明里没有的模型。

口径写在 `.creditia/config.json` 的 `level`，或者 `creditia analyze --level strict`。每个贡献方在 `credits.json` 里都按证据等级拆分了行数，展示里也会标出「逐行证据 xx%」。

## 4. 项目身份：一个项目在好几处

一个仓库常常同时出现在：主目录、多个 worktree、agent 自动建的临时克隆（`/tmp/repo-task-123`）、另一台电脑、搬家前的旧目录、Docker / WSL 里。creditia 这样处理：

- **身份**：根提交 + 远程地址。同一仓库的克隆、worktree、搬过的目录认作同一个项目；根提交相同但远程完全不同的（fork）分开记。
- **路径登记**：每次在某个目录里运行（包括 agent 在 worktree 里代你运行），这个目录就登记进本地项目库。目录以后被删了，日志里留下的路径仍然认得出。
- **手动关联**：`creditia link <路径>` 关联别的电脑上的路径或已经不存在的目录（会检查：目录还在的话必须是同一个仓库）；`creditia paths` 列出全部。配置里的 `paths` 也一样。
- **路径映射**：日志里的绝对路径取「能在仓库里找到的最长后缀」作为仓库内相对路径（仓库历史、当前树，以及暂存区、工作区里还没提交的文件），所以不同电脑、不同用户名、Windows 反斜杠都能对上；都找不到时，路径在某个已知根目录（主仓库、worktree、登记过的克隆）下面，就按根目录切。agent 新建的文件在提交之前就被读到（提交 hook 每次都这样）也认得出，提交以后不会丢。
- **新登记的路径**：登记了新的 worktree、克隆或手动关联的路径以后，之前因为认不出而没读出证据的日志会重新读，不会因为日志文件没变就一直用空结果。
- **多台电脑**：`creditia evidence export` / `import` 合并证据库（只含哈希与元数据），根提交不一致会拒绝。
- **Gemini CLI / Qwen Code** 旧版的目录名是项目路径的 SHA-256，没有明文路径：用上面登记过的全部路径逐个算哈希去对；新版 Gemini CLI 的目录是项目名，里面的 `.project_root` 写着项目路径，直接读。新版 Qwen Code 改存 `projects/<目录>/chats/<会话>.jsonl`，每条记录都带 cwd，直接对。

## 5. 模型与渠道（含中转站）

- **名称归一**：按「是不是同一个模型」归，不按字符串像不像。
  - 渠道：`anthropic/`、`openrouter/anthropic/`、`models/`、`us.anthropic.…-v1:0`（Bedrock）、`…@20250514`（Vertex）、`:free`、`:thinking`（OpenRouter）去掉；
  - 快照与预览：日期（`-20250929`、`-2024-08-06`、`-0709`、`-2507`）、`-latest`、`-preview`、`-exp` 去掉；没有稳定系列名的 `gemini-exp-1206` 保留完整实验标识，不能与 `gemini-exp-1121` 一起变成 `gemini`；
  - 界面写法：`Claude Sonnet 4.5`、`Gemini 3 Pro (High)`、`CLAUDE_SONNET_4_20250514_V1_0`、`[1m]`、`(1M context)` 这类空格、下划线、括号标签；
  - 同一模型的调用档位不是另一个模型：OpenAI 系的推理强度（`-high`、`-xhigh`，Cursor 的 `-fast`）、Claude 的 `-thinking`、Cursor MAX 模式的 `-max`、`-instruct`；
  - Claude 用官方词序：4 代起 `claude-<系列>-<版本>`（Cursor、Amazon Q 写的 `claude-4.5-sonnet` → `claude-sonnet-4-5`），3 代及以前 `claude-<版本>-<系列>`；GPT 保留官方的点号写法（`gpt-5-1-codex` → `gpt-5.1-codex`）；
  - 真不同的模型分开：`kimi-k2-thinking` 和 `kimi-k2`、`gpt-5.1-codex-max` 和 `gpt-5.1-codex`、本地模型的尺寸（`gpt-oss:20b` → `gpt-oss-20b`，和 `gpt-oss-120b` 分开）。
  `crates/core/src/model.rs` 的 `same_model_everywhere` 测试列着各工具、渠道的上百种写法和必须分开的模型对；见到新写法就往里加。
- **Continue 请求模型**：优先读 `promptLogs[].completionOptions.model`，缺失时才用同次请求的 `modelTitle`；都没有就保持未知，不用 `chatModelTitle` 当前选择或前次请求补空。自定义标题不能覆盖已记录的请求模型标识；请求标识不等于服务端最终执行模型的回执。升级会重新解析日志，并修正已读事件旧的标题归属；已经丢失、无法重读的日志不能据此猜回真实模型。
- **工具名归一**：盖章、`Assisted-by`、`creditia stamp --harness` 里的 `Claude Code`、`claude`、`Codex CLI`、`GitHub Copilot`、`Amazon Q`、`agy`……都归到 creditia 的工具 id。
- **厂商**：按名字规则判断（OpenAI、Anthropic、Google、xAI、Moonshot、DeepSeek、阿里、智谱、MiniMax、字节、Mistral、Meta、Cursor）。
- **渠道**（谁提供的模型）只在有证据时才写：
  - Claude Code：Bedrock / Vertex 返回的消息 id 有固定前缀；不是 `msg_` 开头说明经过了改写响应的中转站（记为 `relay`）。透传官方响应的中转站看不出来，就不写。
  - Codex：rollout 里的 `model_provider`。
  - OpenCode：每条消息的 `providerID`（包括你自定义的中转站名）。
  - Cline / Roo / Kilo：`model_provider_id`。
  - 模型串本身的前缀（`openrouter/`、`bedrock/`、`vertex_ai/`）。

模型字段按它在来源中的含义使用；选择、请求和执行回执不是同一层证据。模型规则通过官方源码、数据格式和构造测试核对，不要求每个工具、每个型号另跑一次真实会话。不确定的别名保持独立。

| 来源与依据 | 读取与边界 |
|---|---|
| [Cursor 3.27.22 官方发行包](https://downloads.cursor.com/production/b1aa50cb6db84fd3e8c808259cb172ea73425c1f/darwin/arm64/Cursor-darwin-arm64.dmg) 中 `storeHashes`、`trackSingleEdit`、`setModelConfigForComposer` | `ai_code_hashes.model` 是该行记录的模型快照；`default` 为 Auto，空字段为未知。不用可变的 `composerData.modelConfig` 回填历史。只接受 `composer` / `cli` / `tab`，排除 `human` 和未知来源；Tab 仍记辅助输入。 |
| [Amazon Q 请求元数据](https://github.com/aws/amazon-q-developer-cli/blob/15cc8f3cd18c4272925ce1c7053268eedff1ea0a/crates/chat-cli/src/cli/chat/parser.rs#L216) | `history[].request_metadata.model_id` 是历史请求标识；对话顶层 `model` / `model_info` 是当前选择，不用于补旧编辑。 |
| [Continue 已保存的模型字段](https://github.com/continuedev/continue/issues/2085#issuecomment-2687146546) | `completionOptions.model` 是请求标识，优先于可自定义的 `modelTitle`；没有请求标识才保留同次请求中的标题；两者都缺失则未知，不继承前次请求或 `chatModelTitle` 当前选择。标题相似不能证明是同一模型。 |
| [Copilot SDK 事件定义](https://github.com/github/copilot-sdk/blob/e22b95e6ead5e1de7e9a68d3087b8d28dc5f364a/nodejs/src/generated/session-events.ts) 与 [VS Code 请求记录](https://github.com/microsoft/vscode/blob/e1eafb38b7179f09c61af3482d63dd2aac021784/src/vs/workbench/contrib/chat/common/model/chatSessionOperationLog.ts#L140) | CLI 优先工具事件自己的 `model`，否则保留调用时的 `selectedModel` / `newModel` 选择；VS Code 读该请求的 `modelId`。缺失的历史模型不由后续选择、会话汇总或调试日志回填。 |
| [Kimi wire 类型](https://github.com/MoonshotAI/kimi-cli/blob/9ab1286b8fe4e6bcd116949a27ce5e0ac3389c82/src/kimi_cli/wire/types.py) | 官方嵌套 `message.type/payload` 格式的 `StatusUpdate` 没有模型字段，不能从启动时模型选择补历史。现有读取器支持的顶层 `llm.request` / `usage.record` 是另一种记录形状；仅使用其中明确写出的标识，缺字段或换 wire 时保持未知，不宣称已支持官方 wire 的逐请求模型。 |

重新解析可读日志时会修正对应旧模型归属。Cursor 等滚动历史按实际读到的事件修正，已被清理、无法重读的记录不根据当前选择猜测。构造回归覆盖自定义标题、Auto/未知、切换模型、跨记录缺字段、不可合并的型号，以及日志清理后的归档结果。

## 6. 贡献者

团队仓库里有别人的提交。口径：

- **分母是全部新增行**：AI 工具、其他贡献者、未确认合起来是 100%。不另开一个「只算 AI」的百分比。
- **别人的提交算那个人**：提交里有线索（盖章、agent 邮箱、`Co-authored-by`、`Assisted-by`、bot）时照样认成那个工具和模型，没有线索的行算那位贡献者。贡献者那一行说的是「这位贡献者的提交里没有 AI 线索」，不说明他没用 AI。
- **你自己的提交**：没有任何 AI 证据的行记为「未确认」，不推断为手写，因为没法确定那是你写的还是没留下日志的 AI 写的（见第 3 节）。你不会作为一行出现在工具排名里。
- **名字**：默认写 git 里的名字（`privacy.aliases` 可以把某个名字换成别的写法）。不想写名字时在配置里设 `display.hideNames: true`，档案和全部作品里只写「其他贡献者」；只是这次不写用 `--no-names`，App 设置里的「写其他贡献者的名字」管 App 里的作品和导出。
- **两种看法**：默认按工具 / 模型看（`--group tool|model`）；`--by contributor`（CREDITS.md、终端名片、`summary`）和 App 里的「贡献者」卡片按提交者看——每位提交者一行，后面是他的提交里各工具、没有 AI 记录、未确认各占多少。按提交者看只说是谁提交的，不说是谁手写的。档案里对应的是 `authors`。
- **网页合并的提交**：提交者是 GitHub（`noreply@github.com`）的提交是在 GitHub 网页上合并的，作者只是开 PR、点合并的人，用的是那个账号的邮箱、不是写代码时的邮箱，所以不算他写的：原分支的盖章照常算，其余未确认；
- **同一个 GitHub 账号**：`123+login@users.noreply.github.com` 和 `login@users.noreply.github.com` 认作同一个人；
- **Tab 补全**：Cursor 的 Tab 补全算提交者的辅助输入，不算 agent。

本机档案 credits.json 的 `human` 里还记着开发者这一侧，只供你自己查看，不进作品：发给 AI 的消息数（各工具日志里的 user 消息）、GitHub PR 上你本人的 review（`--github`，bot 的 review 单独计）、AI 写的代码去了哪（会话里写的行 → 提交了的行 → 现在还在的行）。落款 `display.sign` 是你自己填的署名。

## 7. 不计入的内容

- lockfile（npm、yarn、pnpm、bun、Cargo、go.sum、poetry、uv、Gemfile、composer、Podfile、pubspec、flake）、`dist/`、`build/`、`node_modules/`、`vendor/`、`third_party/`、`.min.`、protobuf 生成物；
- `.gitattributes` 里标了 `linguist-generated`、`linguist-vendored`、`-diff`、`binary` 的路径；
- 过短的行（规范化后不到 4 个字符）不单独按内容作证据：按位置对上一次写入（见第 2 节），对不上的跟随同一改动块里它上面最近的长行；
- 二进制文件只计数，不计行。

## 8. 已知的边界

- 人改过几个字的 agent 行、被格式化工具拆开或合并的行，哈希就变了：不算逐行证据。周围大半是 agent 的逐行证据时，按改动块推断算它的；整段都被改过就记「未确认」。
- 你在 agent 写的一段代码中间插进去的几行，如果正好插在 agent 一次写入里相邻的两行之间，记「未确认」；如果你同时也改了旁边 agent 的行，就分不出来，会跟着那一段推断给 agent。
- agent 用 shell 命令写文件时，内容写在命令里的（`cat > file <<EOF`、`printf`、python 脚本里的新文本）按逐行证据算：命令原文的每一行只有真出现在那个文件的提交新增行里才算数，只读文件的命令（`cat`、`grep`）不会带来误认。内容是程序生成的（脚手架、代码生成脚本、`curl > file`）记「未确认」：那不是模型写的，和生成物一样不署给 AI。
- 日志被工具自动清理后，只要之前跑过一次 `analyze` / `collect`（或提交 hook 跑过），证据库里还留着（`creditia schedule install` 每小时自动收一次，不用记着手动跑），每一次写入的每一行都单独记，重建缓存以后结果和日志在的时候一样；从没采集过的就只剩提交元数据。
- 被格式化工具合并的行（agent 的两行被并成一行）还对不上逐行证据，靠改动块推断。
- 透传官方响应的中转站，渠道无法确定，不写。
- 数据源按已确认的格式分别读取。Windsurf Cascade 已接已有 hook/transcript；Antigravity 编辑器可读摘要，摘要不代表编辑证据。原生历史、官方导入、ACP 新会话采集、仅会话和仍缺数据契约的入口分别列在 [完整适配表](ADAPTERS.md)，不根据文件扩展名推断加密或断言只能盖章。没有逐调用模型时保持未知，不把可变的最终模型回填旧编辑。
- 缺少真实时间的成功编辑保留时间未知。只在完整、非浅克隆的已分析历史中，路径与行键只新增过一次且不存在竞争工具/模型时使用该证据；有真实时间的证据优先。重复添加和仅暂存区不走这个兜底。
- Copilot 在 VS Code 里的 agent 记录不一定带模型名：同一会话的 chatSessions 或调试日志里有就用，都没有就只记工具。
- Aider 的历史文件没有逐次编辑的时间：有「Commit」行的用那次提交的时间，没有的按仓库最后一次提交的时区估。
- 各工具的日志格式会变：`creditia doctor` 会列出每个数据源读到了多少、最早最晚是哪天；读不懂的格式会报警告，不会默默算错。

## 9. 准确率基准

`crates/cli/tests/bench.rs` 模拟一个项目的开发过程，每一行是谁写的都已知：几个 agent 各写各的、编辑带上下文、整文件覆盖写入、混合提交、格式化拆行、人改几个字、盖章、云端分支、同一工具两个模型、中文文档、日志丢失、重命名、同文件重复的常见行、多个 agent 交替改同一文件、agent 用 shell 写文件（heredoc、python 脚本）、生成脚本的输出、agent 只读人写的文件、人在 agent 代码中间插行、Aider 与 Copilot 的编辑、Copilot 报错的编辑、在 agent 里提交的手写、格式化合并的行、GitHub 网页 squash 合并。用真实二进制分析，逐行对答案（生成脚本的输出按约定不算给 AI，和人写的行一起算在「不该算给 AI 的行」里）：

| | 工具对 | 模型对 | 错给别的 AI | 漏掉 | 人写或生成的行被算给 AI | 份额总偏差 |
|---|--:|--:|--:|--:|--:|--:|
| 当前 | 95.7% | 95.6% | 0.1% | 4.2% | 0% | 2.9 个百分点 |

最后三个场景：「在 agent 里提交的手写」（盖了章、带 `Creditia-Lines`）里人写的行一行都没算给 AI；「格式化合并的行」里 agent 的行被两两并成一行、逐行证据全对不上，记「未确认」；「GitHub 网页 squash 合并」在云端写、网页上合并，本机没有日志，提交作者是 GitHub 账号的邮箱，原分支的盖章在正文中间——AI 写的行 100% 按盖章找回（不认网页合并时是 0%，全部漏掉）。基准断言 AI 写的行算给人的比例是 0。

「漏掉」几乎都来自日志丢失这个找不回来的场景和格式化合并的行（见第 8 节的边界）；剩下的偏差是几个 `}` 和同一工具两个模型交替写时的模型。CI 每次都跑，结果变差测试就失败。`cargo test -p creditia-cli --test bench -- --nocapture` 看分场景的表格。

## 10. agent 代你使用

- 所有命令非交互、支持 `--json`，退出码：0 成功、1 失败、2 有警告；
- `creditia whoami` 识别当前运行在哪个 agent 里；`creditia stamp --harness <工具> --model <模型>` 给下一次提交盖章；`creditia hook install` 之后在 agent 里提交会自动盖章；
- Cursor 自动识别只接受 `CURSOR_AGENT`；`CURSOR_TRACE_ID` 和 `TERM_PROGRAM` 不足以证明命令由 Agent 执行。显式 `CREDITIA_HARNESS` 仍优先。
- 在 worktree、临时克隆里运行，结果写回主仓库的档案，路径登记进同一个项目；
- 公开任何内容（写 CREDITS.md、改 README、提交）之前，skill 要求 agent 先让你确认。
