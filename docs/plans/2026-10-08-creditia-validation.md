# creditia 技术验证方案（阶段 0）

状态：待确认 · 2026-10-08
主方案：[`2026-10-08-creditia-plan.md`](2026-10-08-creditia-plan.md)
已授权的数据：魏碑仓库、本机所有 agent 日志（只在本机运行，不外传）

---

## 1. 要回答的问题

1. **覆盖率**：魏碑的新增代码里，有多少能找到逐行对得上的日志证据？
2. **准确率**：归因结果和真实情况一致吗？
3. **Cursor**：逐行、带模型的归因能不能在真实 commit 上跑通？
4. **和现有面板比**：结果与魏碑官网现在的数字（Codex 86.6% / Cursor 6.6% / Grok 4.0% / Claude Code 0.4% / DeepSeek 0.4% / 未确认 1.6%）差在哪、为什么？
5. **性能**：全量分析和增量更新各要多久？
6. **隐私**：输出里确实没有代码和 prompt 原文吗？

不在本阶段验证：Windows / Linux（只整理路径，阶段 1 实测）、Grok / Gemini / Kimi（只调研字段）、视觉和 App。

## 2. 数据与覆盖情况

### 2.1 魏碑仓库

- 时间：2026-07-03 → 2026-10-06
- commit：2,155 个
- 远程：`github.com/WroughtMind/weibei`
- PR 信息：merge commit 的提交信息（"Merge pull request #519 from WroughtMind/codeberg/codex/connection-cards"）带有 PR 号和分支名；`Docs/migrations/codeberg-pr-archive-2026-09-30.json` 里有 Codeberg 时期的 PR 存档

### 2.2 各数据源对魏碑的覆盖（2026-10-08 实测）

| 数据源 | 魏碑相关数据 | 覆盖时间 |
|---|---|---|
| Codex 线程索引 | 480 个线程，每个都有模型、token、`git_sha`、`git_branch` | 2026-08-01 起 |
| Codex 会话内容 | 255 个线程的内容文件还在（41 个 `.jsonl` + 214 个 `.jsonl.zst`），225 个找不到（可能是之前手动清理过） | 2026-08 起 |
| Codex 线程模型分布 | `codex-auto-review` 240、`gpt-5.6-sol` 209、`gpt-6-astra` 18、`gpt-6.1-sol` 4、`gpt-6-sol` 3 | |
| Claude Code | 16 个会话（中文路径被编码成 `-Users-<user>-projects---`，要靠记录里的 `cwd` 识别） | |
| Cursor 行级记录 | 8,732 条行哈希 | 2026-09-18 起（只留 30 天） |
| Cursor 对话库 | 每个对话的模型名 | 2026-05-26 起 |
| Cursor 会话记录 | 2 个项目目录 | |
| git trailer | `Cursor <cursoragent@cursor.com>` 82 条，`Claude` 20 条 | 全程 |

按证据强弱，魏碑的时间线分成三段：

- **A 段（07-03 → 07-31）**：本机没有这段时间的 agent 日志，只有 git 信号。预期这一段大部分只能做到"标准"或"宽松"口径。这一段正好用来验证"日志缺失时的降级处理"，这是很多用户都会遇到的情况
- **B 段（08-01 → 09-17）**：有 Codex 线程和部分会话内容、Claude Code、Cursor 对话库
- **C 段（09-18 → 10-06）**：在 B 段基础上多了 Cursor 行级记录，证据最全

结果按段分别报告，不混成一个总数掩盖差异。

## 3. 各数据源的归因方法

### 3.1 Cursor（重点）

Cursor 在本机有三层数据，组合起来可以做到逐行、带模型。

**第一层：行级记录** `~/.cursor/ai-tracking/ai-code-tracking.db`

Cursor 每次由 agent 或 Tab 补全写入代码时，对每一行计算一个哈希并存下来，附带模型、对话 ID 和来源。

`ai_code_hashes` 表的字段：`hash`、`source`（`composer` = agent，`tab` = Tab 补全）、`fileName`、`model`、`conversationId`、`requestId`、`timestamp`。

哈希算法从 Cursor 安装包（`Cursor.app/Contents/Resources/app/out/main.js`）中确认：

```
hash = murmurhash3_32(`${仓库内相对路径}:+${规范化后的行}`, seed = 0).toString(16)
```

- 删除的行用 `:-` 前缀
- 空行不记
- 规范化：去掉首尾空白，再去掉所有空白；单引号换成双引号；去掉分号和逗号；JS / TS / Python / Ruby 另有少量语言规则（箭头函数参数括号、模板字符串、Python 引号和赋值空格等）
- 字符串按 UTF-16 码元取低 8 位参与计算，不是 UTF-8 字节

**已经验证**：用这个算法对本次对话里 Cursor 写的方案文件逐行计算，283 个哈希全部与 Cursor 数据库里的记录对上（283/283）。

所以对于 git diff 里的任意一行新增代码，按同样方式算出哈希，去表里查，就能知道它是不是 Cursor 写的、哪个模型、哪次对话。

限制：

- **只保留 30 天**：Cursor 启动时删除 30 天前的记录（代码里写死的 `720*60*60*1e3`）
- 模型名可能是 `default`（Auto 模式）。本机记录中 `default` 占 2,654 / 13,578
- 同表里还有 `scored_commits`，是 Cursor 自己对 commit 的打分（agent 行、Tab 行、人写行、AI 占比），本机只有 21 条，作为交叉核对用

**第二层：对话库** `~/Library/Application Support/Cursor/User/globalStorage/state.vscdb`

- `cursorDiskKV` 表里 `composerData:<对话ID>` 的 `modelConfig.modelName` 是对话选用的模型。本机有 241 个对话，从 2026-05-26 起
- `bubbleId:<对话ID>:<消息ID>` 是每条消息，`modelInfo.modelName` 是实际回答的模型，另有 `tokenCount` 和代码块、diff 相关字段
- 文件 2.9GB，只读查询，不复制

**第三层：会话记录** `~/.cursor/projects/*/agent-transcripts/<对话ID>.jsonl`

- 记录了编辑工具调用（StrReplace、Write）的文件路径和内容，没有模型字段
- 文件名就是对话 ID，可以和第二层关联。本机 125 个会话记录中有 89 个能在对话库里找到

**组合策略**：

- 30 天以内：直接用第一层，逐行、带模型，记为 `verified`
- 30 天以前：用第三层的编辑内容和 diff 逐行匹配（`verified`），模型取第二层的消息级模型，查不到再用对话级模型
- 模型是 `default` 时：先查消息级模型；仍然查不到就显示为"Cursor Auto"

### 3.2 Codex

- 用 `~/.codex/state_5.sqlite` 的 `threads` 表找出魏碑的所有线程（按 `cwd` 和 `git_origin_url`），拿到模型、token、分支、起始 commit
- 读线程对应的内容文件（`.jsonl` 或 `.jsonl.zst`），解析 `apply_patch` 里新增的行，建立"路径 + 规范化行"索引
- 内容文件丢失的线程，只能在"同分支 + 时间窗口"层面推断
- `codex-auto-review` 线程记为审阅，不计入代码行

### 3.3 Claude Code

- 在 `~/.claude/projects/` 里按 `cwd` 找出魏碑的会话
- 解析 Edit 的 `new_string`、Write 的内容、MultiEdit，建立同样的索引
- 模型取每条消息的 `message.model`，跳过 `<synthetic>`

### 3.4 git 信号

- trailer：`Co-authored-by`、提交者邮箱
- 分支：从 merge commit 信息里取 PR 号和分支名（`codex/…`、`cursor/…`、`claude/…`、`grok/…`）
- PR：merge commit 信息 + Codeberg 存档

### 3.5 统一匹配

所有日志来源都转换成同一种索引：`(仓库内相对路径, 规范化后的行) → [工具, 模型, 会话, 时间]`。规范化统一采用 Cursor 的规则，这样三家的结果可以直接比较。

对每个非 merge commit 的每一行新增代码：

1. 查索引，候选会话的时间要在 commit 之前 14 天以内、之后 1 小时以内
2. 规范化后少于 4 个字符的行（`}`、`)` 这类）不作为证据，按该 commit 的主要贡献者分配
3. 多个来源同时命中时，取时间最接近 commit 的那个，并记录冲突，在报告里统计冲突比例
4. 没有命中的行依次降级：trailer → 同分支同时间窗口有会话 → 分支前缀 → 未确认
5. 默认排除 lockfile、构建产物、二进制文件

## 4. 标准答案

自动结果需要人工核对。

1. **抽样**：抽 40 个 commit，A、B、C 三段各 10 个，另外 10 个从改动最大的 commit 里随机抽
2. **准备材料**：我为每个 commit 准备一页摘要，包括标题、分支、trailer、改动文件、各数据源的命中情况、系统给出的归因
3. **你来确认**：逐条标"对 / 错 / 不确定"，错的写上实际是谁。预计 20–30 分钟
4. **受控实验（可选）**：新建一个临时仓库，分别用 Cursor、Claude Code、Codex 和纯手写各做一个 commit，真实情况完全已知，检验结果是否 100% 正确

## 5. 指标与通过标准

| 指标 | 通过标准 |
|---|---|
| Cursor 哈希复现率（C 段内 Cursor 写的行能否在表里查到） | ≥ 99% |
| 覆盖率：严格口径下能归因的新增行占比 | C 段 ≥ 70%，B 段 ≥ 50%，A 段只报告 |
| 准确率：抽样 commit 的主要贡献者判断正确 | ≥ 90% |
| 占比误差：抽样 commit 中各贡献者占比与你确认的结果之差 | 平均 ≤ 15 个百分点 |
| 全量分析耗时（魏碑） | ≤ 2 分钟 |
| 增量更新耗时（新增 20 个 commit） | ≤ 10 秒 |
| 隐私：输出文件里出现代码或 prompt 原文 | 0 处（自动检查） |

**判定**：

- 全部通过：进入阶段 1
- 覆盖率不达标但准确率达标：说明证据会丢失，证据库和盖章要提到 MVP 最前面，方案基本不变
- 准确率不达标：先分析错在哪类 commit，调整匹配算法后重跑一次；仍不达标就回头改方案

## 6. 步骤

- [ ] 1. 在 `spike/` 下搭最小 Rust 工程（cargo workspace），验证结束后拆进 `crates/`（0.5 天）
- [ ] 2. 实现规范化和 murmurhash3（注意 Cursor 按 UTF-16 码元取低 8 位，要逐字节复现），用 Cursor 表里的真实数据做单元测试（0.5 天）
- [ ] 3. 实现 Cursor 三层、Codex、Claude Code、git 四个 adapter，输出统一索引；顺带处理 worktree 映射、中文路径、`.jsonl.zst`（2 天）
- [ ] 4. 实现匹配和降级逻辑，产出魏碑的 `credits.json` 草稿（1 天）
- [ ] 5. 生成验证报告 `docs/validation/2026-10-xx-weibei-report.md`：分段覆盖率、各数据源贡献、冲突统计、与现有面板的对比、性能（0.5 天）
- [ ] 6. 准备 40 个抽样 commit 的摘要，交给你确认（0.5 天准备 + 你 20–30 分钟）
- [ ] 7. 根据确认结果算准确率，写结论，决定是否进入阶段 1（0.5 天）
- [ ] 8. （可选）受控实验
- [ ] 9. 调研 Grok、Gemini、Kimi 日志字段，写进报告附录（0.5 天）
- [ ] 10. 逐项验证情况处理方案末尾"待验证"列表里的五种情况
- [ ] 11. 实测 Tauri 2 空工程在本机的首次编译时间和缓存体积，确认磁盘占用可控

合计约 5–6 个工作日。

## 7. 隐私

- 所有 agent 数据只读打开，SQLite 用只读模式，不在 Cursor / Codex 运行时加锁
- `spike/out/` 加入 `.gitignore`，中间索引和输出都不提交
- 验证报告只含聚合数字、commit hash、PR 标题；抽样摘要里的文件名和改动行数可以出现，代码内容不出现
- 自动检查：扫描所有输出文件，确认没有超过一定长度的代码片段或 prompt 文本

## 8. 风险

| 风险 | 应对 |
|---|---|
| 魏碑 A 段没有日志，总体覆盖率偏低 | 按段报告，A 段只作参考，不计入通过标准 |
| Codex 有 225 个线程的内容文件已丢失 | 这些线程只能做到"同分支 + 时间窗口"层面，在报告中单独统计 |
| 同一行被多个 agent 命中（例如通用的 import 语句） | 取时间最近的那个，并统计冲突比例；比例过高再调整规则 |
| Cursor 读取的是未公开的内部格式 | 本阶段只验证可行性；阶段 1 加格式版本检测 |
| 抽样标注时你也记不清是谁做的 | 允许标"不确定"，不确定的不计入准确率，但会报告数量 |
