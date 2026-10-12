# creditia 怎么归因

这份文档说明 creditia 如何判断「这一行是谁写的」。原则：**不调用大模型；证据和推断分开记、分开显示；每个结论都能复查**。

要说清楚的是：默认口径（standard）**会做两种推断**——同一提交内的延伸、分支前缀；它们都记为「推断」，不会混进逐行证据或明确声明。只想要有证据的结论，用 `strict`。

## 1. 证据从哪里来

| 来源 | 内容 | 证据等级 |
|---|---|---|
| agent 日志 | 每次**成功**写进文件的内容（Write、Edit、FileChange、write_file、replace、patch……），连同会话、模型、时间、渠道 | 逐行证据（verified） |
| Cursor 行级库 | Cursor 自己记下的逐行哈希。creditia 复现了它的规范化与哈希算法，逐字节一致 | 逐行证据 |
| 提交 trailer | `Creditia-Harness` / `Creditia-Model`（`creditia stamp` 或 hook 写入） | 明确声明（declared） |
| 提交元数据 | agent 的提交邮箱（Cursor 云端、Copilot、Devin、Jules…）、`Co-authored-by`、bot 账号、aider 改写的作者名 | 明确声明 |
| 其他人的提交 | 不是你的提交身份 → 算那个人 | 明确声明 |
| 分支前缀 | `codex/`、`cursor/`、`copilot/`… 由工具自动创建的分支 | 推断（inferred） |
| 时间窗口 | 提交时恰好有某个工具的会话在进行 | 推断（只在 loose 口径） |

证据库只存「相对路径、行哈希、工具、模型、会话、时间、渠道」，**不存代码、不存 prompt**。

## 2. 一行是怎么对上的

1. 读 `git log -p`，对每个提交的每一个新增行做规范化（去空白、统一引号等）再哈希，键是 `相对路径 + 规范化后的行`。
2. 在证据库里找同路径、同哈希、时间在提交前 14 天到提交后 1 小时之内的证据；多条命中取时间最接近的。
3. 对上了就是**逐行证据**。没对上的行按下面的降级链处理。

因为最后一步要求「行的内容出现在真实提交里」，所以：

- 日志里被拒绝、报错的编辑不算证据：Claude Code 看工具结果的 `is_error`，Cline / Roo / Kilo 要在对话记录里找到「写入成功」的结果，Gemini CLI 看 `status`，OpenCode 看 `completed`；
- 后来被删掉的编辑，不会凭空变成署名；
- 路径匹配可以宽松一些（多个 worktree、临时克隆），误认的风险由内容匹配兜底；
- 改写历史（rebase、squash、cherry-pick）不影响结果：行的内容和作者时间都还在。

## 3. 没对上的行：降级链与口径

| 步骤 | strict | standard（默认） | loose |
|---|:-:|:-:|:-:|
| 同一提交里有某个 agent 的逐行证据 → 余下的行跟随它（推断） | | ✓ | ✓ |
| 盖章 trailer | ✓ | ✓ | ✓ |
| agent 提交邮箱、Co-authored-by、bot、aider 作者名 | ✓ | ✓ | ✓ |
| 其他人的提交 → 那个人 | ✓ | ✓ | ✓ |
| 提交时有会话在进行（推断） | | | ✓ |
| 分支前缀（推断） | | ✓ | ✓ |
| 以上都没有 → **未确认** | ✓ | ✓ | ✓ |

口径写在 `.creditia/config.json` 的 `level`，或者 `creditia analyze --level strict`。每个贡献方在 `credits.json` 里都按证据等级拆分了行数，展示里也会标出「逐行证据 xx%」。

## 4. 项目身份：一个项目在好几处

一个仓库常常同时出现在：主目录、多个 worktree、agent 自动建的临时克隆（`/tmp/repo-task-123`）、另一台电脑、搬家前的旧目录、Docker / WSL 里。creditia 这样处理：

- **身份**：根提交 + 远程地址。同一仓库的克隆、worktree、搬过的目录认作同一个项目；根提交相同但远程完全不同的（fork）分开记。
- **路径登记**：每次在某个目录里运行（包括 agent 在 worktree 里代你运行），这个目录就登记进作品库。目录以后被删了，日志里留下的路径仍然认得出。
- **手动关联**：`creditia link <路径>` 关联别的电脑上的路径或已经不存在的目录（会检查：目录还在的话必须是同一个仓库）；`creditia paths` 列出全部。配置里的 `paths` 也一样。
- **路径映射**：日志里的绝对路径取「能在仓库历史里找到的最长后缀」作为仓库内相对路径，所以不同电脑、不同用户名、Windows 反斜杠都能对上。
- **多台电脑**：`creditia evidence export` / `import` 合并证据库（只含哈希与元数据），根提交不一致会拒绝。
- **Gemini CLI / Qwen Code** 的目录名是项目路径的 SHA-256，没有明文路径：用上面登记过的全部路径逐个算哈希去对。

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

人不和 AI 工具排同一个百分比。credits.json 的 `human` 和展示里「人」这一块记录：

- **发给 AI 的消息**：会话里人发出的消息数（各工具日志里的 user 消息）；
- **review**：GitHub PR 上你本人的 review（`--github`），bot 的 review 单独计；
- **人写的行**：别人的提交算那个人，Tab 补全的行也算人；你自己的提交里没有任何 AI 证据的行记为「未确认」，因为没法确定那是你写的还是没留下日志的 AI 写的；
- **AI 写的代码去了哪**：AI 在会话里写的行 → 提交了的行 → 现在还在的行；
- **Tab 补全**：Cursor 的 Tab 补全算人的辅助输入，不算 agent。

人名默认不出现在任何展示里；`display.named` 打开才写，`display.sign` 是你主动写的落款。

## 7. 不计入的内容

- lockfile（npm、yarn、pnpm、bun、Cargo、go.sum、poetry、uv、Gemfile、composer、Podfile、pubspec、flake）、`dist/`、`build/`、`node_modules/`、`vendor/`、`third_party/`、`.min.`、protobuf 生成物；
- `.gitattributes` 里标了 `linguist-generated`、`linguist-vendored`、`-diff`、`binary` 的路径；
- 过短的行（规范化后不到 4 个字符）不作证据，跟随同一提交的主要贡献方；
- 二进制文件只计数，不计行。

## 8. 已知的边界

- 人改过一个字的 agent 行，哈希就变了：算作那个提交的降级结果，而不是 agent 的逐行证据。
- 格式化工具（prettier、rustfmt）大幅改动 agent 输出的换行时，也会让逐行证据对不上。
- 日志被工具自动清理后，只要之前跑过一次 `analyze` / `collect`，证据库里还留着；从没采集过的就只剩提交元数据。
- 透传官方响应的中转站，渠道无法确定，不写。
- 各工具的日志格式会变：`creditia doctor` 会列出每个数据源读到了多少、最早最晚是哪天；读不懂的格式会报警告，不会默默算错。

## 9. agent 代你使用

- 所有命令非交互、支持 `--json`，退出码：0 成功、1 失败、2 有警告；
- `creditia whoami` 识别当前运行在哪个 agent 里；`creditia stamp --harness <工具> --model <模型>` 给下一次提交盖章；`creditia hook install` 之后在 agent 里提交会自动盖章；
- 在 worktree、临时克隆里运行，结果写回主仓库的档案，路径登记进同一个项目；
- 公开任何内容（写 CREDITS.md、改 README、提交）之前，skill 要求 agent 先让你确认。
