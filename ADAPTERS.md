# Agent 读取范围（开发版本）

本页保留 2026-10-19 核查的 73 个工具与读取入口及其证据。当前 78 项公共目录的口径为 **72 个历史入口、5 个执行宿主、1 个未确认格式**，见 [完整覆盖表](AGENT_SUPPORT.md)。两张表不能相加作为产品数量。最新的 Eve 与 inference.sh 真实执行验收、MCPJam 原生执行验收分别见 [SUPPORT_NATIVE_LIVE.md](SUPPORT_NATIVE_LIVE.md) 与 [SUPPORT_MCPJAM_LIVE.md](SUPPORT_MCPJAM_LIVE.md)。灵码真实库验收及此前各轮来源保留在 [SUPPORT_AGENT3.md](SUPPORT_AGENT3.md) 与 `SUPPORT_AGENT*.md`。

下表区分原生编辑、官方导入、有限 hook、仅会话和 ACP 新会话采集。数量不是独立产品数、市场覆盖率或逐版本验收数。同名产品的不同入口分别核查；表中范围以实际已确认格式为准。

新增读取统一进入 CLI 与 App 共用的项目发现、数据体检、正式分析。doctor 的 `scope` 区分 `edits`、`sessions`、`capture`、`diagnostic`；可读取会话不表示能计算代码贡献。Grok Build、Kimi Code CLI 是工具名，模型另记。

采集入口的数据源 ID 带 `-acp`，与同工具的原生历史入口分别显示，避免把开启采集误当成读到了旧历史。

| 工具 / 入口 | harness | 读取范围 | 边界 |
|---|---|---|---|
| Claude Code | `claude-code` | 原生编辑及 MCPJam 执行历史 | 自动发现 MCPJam 隔离 home 的原生 JSONL；成功回执、路径链接、跨目录去重与子 agent 分离均经过验收，见 [记录](SUPPORT_MCPJAM_LIVE.md)。 |
| Codex | `codex` | 原生编辑及执行宿主历史 | SQLite 索引可选；读取原生 rollout、成功 patch 和严格的命令写入回执，自动发现 MCPJam 独立 Codex home；inference.sh 官方驱动/导出已真实验收，见 [记录](SUPPORT_NATIVE_LIVE.md)。 |
| Cursor 编辑器 | `cursor` | 已有读取 | 现有范围见 [归因说明](ATTRIBUTION.md)，本次未逐版本重新验收。 |
| Grok Build | `grok` | 已有读取 | 现有范围见 [归因说明](ATTRIBUTION.md)，本次未逐版本重新验收。 |
| Kimi Code CLI | `kimi` | 已有读取 | 现有范围见 [归因说明](ATTRIBUTION.md)，本次未逐版本重新验收。 |
| Gemini CLI | `gemini` | 已有读取 | 现有范围见 [归因说明](ATTRIBUTION.md)，本次未逐版本重新验收。 |
| Qwen Code | `qwen` | 已有读取 | 现有范围见 [归因说明](ATTRIBUTION.md)，本次未逐版本重新验收。 |
| OpenCode | `opencode` | 已有读取 | 现有范围见 [归因说明](ATTRIBUTION.md)，本次未逐版本重新验收。 |
| Cline 扩展 | `cline` | 已有读取 | 现有范围见 [归因说明](ATTRIBUTION.md)，本次未逐版本重新验收。 |
| Roo Code | `roo` | 已有读取 | 现有范围见 [归因说明](ATTRIBUTION.md)，本次未逐版本重新验收。 |
| Kilo Code | `kilo` | 已有读取 | 现有范围见 [归因说明](ATTRIBUTION.md)，本次未逐版本重新验收。 |
| Aider | `aider` | 已有读取 | 现有范围见 [归因说明](ATTRIBUTION.md)，本次未逐版本重新验收。 |
| GitHub Copilot | `copilot` | 已有读取 | 现有范围见 [归因说明](ATTRIBUTION.md)，本次未逐版本重新验收。 |
| Amp | `amp` | 已有读取 | 现有范围见 [归因说明](ATTRIBUTION.md)，本次未逐版本重新验收。 |
| TRAE CLI | `trae` | 已有读取 | 现有范围见 [归因说明](ATTRIBUTION.md)，本次未逐版本重新验收。 |
| Crush | `crush` | 已有读取 | 现有范围见 [归因说明](ATTRIBUTION.md)，本次未逐版本重新验收。 |
| Goose | `goose` | 已有读取 | 现有范围见 [归因说明](ATTRIBUTION.md)，本次未逐版本重新验收。 |
| Continue | `continue` | 已有读取 | 现有范围见 [归因说明](ATTRIBUTION.md)，本次未逐版本重新验收。 |
| Zed | `zed` | 已有读取 | 现有范围见 [归因说明](ATTRIBUTION.md)，本次未逐版本重新验收。 |
| Amazon Q CLI | `amazon-q` | 已有读取 | 现有范围见 [归因说明](ATTRIBUTION.md)，本次未逐版本重新验收。 |
| Antigravity CLI | `antigravity` | 已有读取 | 现有范围见 [归因说明](ATTRIBUTION.md)，本次未逐版本重新验收。 |
| ZCode | `zcode` | 原生编辑 | 本机 SQLite；逐生成消息模型；Write 需实际差分，缺执行/文件结果的 shell 排除。 |
| Pi | `pi` | 原生编辑 | 逐消息模型；配对成功结果。 |
| Oh My Pi（OMP） | `omp` | 原生编辑 | hashline 使用实际结果；逐消息模型。 |
| Command Code | `command-code` | 原生编辑 | 只认实际成功回执；逐消息模型。 |
| Muse Code | `muse` | 有限原生编辑 | 保留参数且 task completed 的审计链；未知历史模型不猜。 |
| T3 Code | `t3` | 宿主编辑 | v2 completed 文件变更；执行者/模型取 driver/run；v1 仅会话。 |
| Mistral Vibe | `mistral-vibe` | 原生编辑 | 消息不保留逐调用模型/时间，编辑模型未知。 |
| MiniMax Code | `minimax-code` | 原生编辑 | canonical v2 历史优先；逐消息模型。 |
| Factory Droid | `droid` | 原生编辑 | 官方 SDK JSONL；调用与成功结果配对。 |
| Auggie CLI | `auggie` | 原生编辑 | 跨 exchange 配对结果；逐 exchange 模型。 |
| Qoder CLI | `qoder` | 原生编辑 | 明确成功 tool_result；生成消息模型。 |
| CodeBuddy Code | `codebuddy` | 原生编辑 | function_call/result；业务错误优先排除。 |
| Junie | `junie` | 有限原生编辑 | 888.12 CLI A2UX completed 前后差分；旧协议/IDE 另验。 |
| Kiro CLI | `kiro` | 有限原生编辑 | classic SQLite 与 KAS V3 原生会话/快照已读；Rust V2 codec 仍待核实。 |
| iFlow CLI | `iflow` | 原生编辑 | 明确 toolUseResult.status=success；历史可读不表示服务仍运行。 |
| Neovate Code | `neovate` | 原生编辑 | 官方项目索引必须唯一；逐消息模型。 |
| CoStrict | `costrict` | 有限原生编辑 | CLI SQLite 与扩展 structured receipt 分别读取；旧 XML 协议未覆盖。 |
| CodeFlicker | `codeflicker` | 原生编辑 | 本地 SQLite KV；按工具 ID 配对实际成功回执，排除拒绝与复制历史。 |
| Tabnine Agent | `tabnine` | 有限原生编辑 | legacy CLI 与 3.366.0 IDE Agent 已读；底层 OpenCode 模型/时间优先，UI 记录补缺。 |
| Comate / Zulu | `comate` | 原生编辑 | JSON 与 V2 blob；已执行、已接受且保存后的实际 before/after。 |
| Devin | `devin` | 有限 hook 编辑 + 云端 ACP 导入 | CLI 成功 exec apply_patch；Cloud v3 仅会话元数据，另读原生 ACP 更新（completed edit diff，模型取同回合唯一值）；Desktop Cascade 走 Cascade 读取器。 |
| Jules | `jules` | 官方导入编辑 | Sources 仓库映射 + activities 产物补丁；模型未知。 |
| OpenHands | `openhands` | 官方导入编辑 | 0.60 serialized events 成功 write/edit；现代 SDK 另验。 |
| SWE-agent | `swe-agent` | 官方导入编辑 | submitted 实际 patch；无时间只匹配历史唯一新增行。 |
| mini-SWE-agent | `mini-swe-agent` | 官方导入编辑 | 1.1 actions/result returncode=0；限定可证明的 shell 写法。 |
| Replit Agent | `replit` | 官方导入编辑 | 原始 Timeline SyncBlocksResponse / feed recording 的已完成 CreateFile/FileEdit；需绝对路径，模型未知。 |
| Bolt.new | `bolt` | 官方 HAR 导入编辑 | 原始 HAR 中互证的回合起始提交、bolt-merged、apply-changes 与同 baseSha 原文；模型与执行时间未知，缺基线的新文件跳过。 |
| Lovable | `lovable` | 官方 HAR 导入编辑 | trajectory 成功输出按官方 Hunk[] 消费链读取；相对路径须同项目 gitsync.repo_url 绑定。 |
| v0 | `v0` | 仅会话 | v2 usage/时间与可定位项目；file-edit 无正文，不冒充逐行证据。 |
| Warp Agent | `warp` | 原生编辑 | SQLite task protobuf / ConversationData；配对成功差分，排除人工修改与撤销；Oz 元数据另读。 |
| Windsurf Cascade | `windsurf` | 官方导入编辑 | 已有 post_write_code hook / done transcript；原生 pb 未解码。 |
| TRAE 编辑器 | `trae-ide` | 原生编辑 | 只读解密 SQLCipher，读取 task 原始 success 回执的实际前后差分；归档/删除/失败不计，执行模型未知。3.6.2 与 3.6.4 协议已静态对照，加密测试库已通过 CLI 归因；真实账号另验。见 [TRAE 说明](SUPPORT_TRAE.md)。 |
| Antigravity 编辑器 | `antigravity-ide` | 原生编辑 | 完整 trajectory SQLite/protobuf 成功 ActionResultEdit；逐生成模型，摘要仅补缺。 |
| DeepSeek Harness（DSH） | `dsh` | 原生编辑 | v0–v4、zstd、canonical generation；成功回执与实际前后差分。 |
| Cortex Code | `cortex-code` | 原生编辑 + ACP | 快照/增量日志；嵌套调用与明确成功回执；编辑模型未知。 |
| IBM Bob | `bob` | 原生编辑 + ACP | 原生 messages 中实际保存后的 `_meta.changes`；审批预览、失败不算。 |
| Poolside | `poolside` | 原生编辑 + ACP | NDJSON 实际 old_contents/new_contents；同 step 推理模型。 |
| Autohand Code | `autohand` | 有限原生编辑 + ACP | 当前确定支持 git_apply_patch；普通文件工具的 preview 状态未持久化，排除。 |
| VT Code | `vtcode` | 原生编辑 + ACP | SessionSnapshot 配对 write 的真实输出 diff/bytes；未存逐调用模型。 |
| Kimchi | `kimchi` | 原生编辑 + ACP | 独立 Pi harness 路径；逐消息模型、多段 edit。 |
| Nova | `nova` | 原生编辑 + ACP | archive/CLI/已有 ACP 快照；只读 completed 实际 changes，排除 dry-run。 |
| GLM Agent | `glm-agent` | 有限原生编辑 + ACP | 成功 edit_file；缺 before 的整文覆盖排除，历史最后模型不回填编辑。 |
| Dirac | `dirac` | 原生编辑 + ACP | baseline 与增量操作；真实保存及单文件实际 diff，排除审批展示与人工改写。 |
| Corust Agent | `corust` | ACP 新会话 | 当前 0.6.0 明确 load_session 未实现；未伪造旧历史读取。 |
| crow-cli | `crow-cli` | 原生编辑 + ACP | completed actual diff；可变模型字段仅作会话记录。 |
| DimCode | `dimcode` | 原生编辑 + ACP | SQLite 实际 edit diff；run 账本只有唯一模型时关联，歧义留空。 |
| Minion Code | `minion-code` | 原生编辑 + ACP | 已保留 agent_history 的成功编辑；覆盖需完整 read-before，只有 UI 对话无法还原。 |
| siGit Code | `sigit` | 原生编辑 + ACP | 创建/编辑/多编辑明确回执；逐调用模型未知。 |
| Stakpak | `stakpak` | 原生编辑 + ACP | 跨 checkpoint 配对本地成功回执与实际 diff；未存的模型不猜。 |
| Cursor CLI | `cursor` | 原生编辑 | 独立 store.db 成功 before/after；encrypted_model 不猜。 |
| Cline CLI | `cline` | 原生编辑 | 独立 SDK sessions.db/messages；逐消息 modelInfo。 |
| Amazon Q IDE | `amazon-q` | 原生编辑 | LokiJS fsWrite/fsReplace 成功结果；逐调用模型未知。 |

## ACP 新会话采集

在支持 ACP 的客户端中，把原来的 agent 启动命令包在 Creditia 后面。例如原来启动 `cortex acp serve`，改为：

```sh
creditia capture --harness cortex-code -- cortex acp serve
```

客户端仍按原来的流程登录、发消息、确认权限。Creditia 不安装 agent、不增加权限、不替用户批准操作；标准输入和输出逐字节透传，退出码传回客户端。不是在普通终端里直接发聊天的命令。

默认记录在 XDG 数据目录的 `creditia/captures/*.jsonl`，随后正常 `creditia discover`、`creditia doctor --json`、`creditia analyze --repo /path/to/repo` 自动读取。`CREDITIA_CAPTURE_DIR` 可指定另一个采集目录；使用 `--out /path/to/file.jsonl` 时，应把其所在目录设为该变量。已有文件不会覆盖。

仅记录已完成工具的明确 diff，或有完整 before 快照及成功回执的 `fs/write_text_file` 增量；权限获批、pending、失败、缺少 before 的整文覆盖都不作为写入证据。没有模型字段就保持未知。加载旧会话重放的记录不伪装成当前时间。

采集文件是本机私有原始编辑记录，可能含代码正文；Unix 新文件权限为 0600。认证交换、MCP 环境变量、提示词正文、权限 payload、任意工具原始输出不保存。正式证据库仍只保存行哈希和元数据。不要把原始采集文件当作公开 credits.json。

下列启动参数来自核查时的[官方 ACP 注册表](https://agentclientprotocol.com/get-started/registry)，实际账号、安装及平台要求仍由各工具决定。以已安装的命令代替示例中的命令：

| harness | 官方启动入口 |
|---|---|
| cortex-code | `cortex acp serve` |
| bob | `npx @ibm/bob@2.0.8 acp` |
| poolside | `pool acp` |
| autohand | `npx @autohandai/autohand-acp@0.2.1` |
| vtcode | `vtcode acp`；客户端环境设置 `VT_ACP_ENABLED=1`、`VT_ACP_ZED_ENABLED=1` |
| kimchi | `kimchi --mode acp` |
| nova | `npx @compass-ai/nova@1.1.54 acp` |
| glm-agent | `npx glm-acp-agent@1.15.0` |
| dirac | `npx dirac-cli@0.5.21 --acp` |
| corust | `corust-agent-acp` |
| crow-cli | `crow-cli acp` |
| dimcode | `npx dimcode@0.5.19 acp` |
| minion-code | `uvx minion-code@0.1.44 acp` |
| sigit | `sigit`（注册表入口本身即 ACP） |
| stakpak | `stakpak acp` |

这是已实现的通用协议路径及官方启动映射，尚未逐家启动模型做真实交互验收。开启后才有采集记录；它不补造旧历史。原生历史、导出与采集的具体证据分别见 [本地 CLI 与编辑器](SUPPORT_LOCAL.md)、[Pi / Muse / T3 等会话](SUPPORT_SESSIONS.md)、[长尾原生格式](SUPPORT_LONGTAIL.md)、[最后五家发行物核实](SUPPORT_REMAINING.md)、[云端 / 轨迹 / DSH](SUPPORT_CLOUD.md)、[当前云端客户端契约](SUPPORT_CLOUD_NATIVE.md)、[Bolt 保存链](SUPPORT_BOLT.md)、[Lovable 链路](SUPPORT_LOVABLE.md)、[Devin ACP 协议](SUPPORT_DEVIN.md)、[SQLCipher 解密器](SUPPORT_SQLCIPHER.md)。

## 时间与去重

已知时间的行证据优先按现有时间窗口匹配。缺失真实时间的成功编辑保持时间未知；只有完整、非浅克隆的已分析 Git 历史中，该路径与行键只新增过一次，且工具/模型没有竞争归属时才匹配。重复添加、存在冲突和仅暂存区的场景不会使用此兜底，也不借文件修改时间补猜执行时刻。

T3 按真实执行者和 native thread ID 避免重复会话；上次 ACP 缓存不会压制新读到的原生记录。ACP 根据调用身份去除重放/拷贝，保留不同调用、不同模型和一次写入中的重复行。与原生同会话的路径、行及模型按发生次数保守重叠，优先原生时间和模型；捕获接收时间不作为独立写入证明。缺模型可以重叠，不同已知模型分别保留；未知模型最后匹配，避免占用已知模型名额。由于没有所有来源共享的调用 ID，这种合并可能少计无法证明独立的补充记录，不将它们重复记功。

工具执行记录按一次写入消费，包括 Cursor CLI/宿主/ACP 的 Cursor 调用；只有 Cursor 编辑器的静态行哈希集合保留其原有集合语义。证据库保存不同模型、不同时间及相同事件内的重复行次数；旧库原子迁移，不删除既有行哈希。

Tabnine 原生会话映射只改变已确认的执行者归属，底层 OpenCode 历史模型、时间及 token 保留；UI fallback 不覆盖原生元数据。新确认的映射会事务迁移作品库旧 OpenCode 记录，已确认归属在映射清理或缓存重建后仍从作品库恢复。Replit 以原生 block 身份跨导出合并，保留不同 block 的同文写入；冲突或取消记录不复活为成功。

## 已验证与未验证

新增解析测试覆盖成功、失败、未完成、上下文扣除、模型切换、工作目录、跨入口去重与增量缓存；真实 CLI 回归覆盖 ZCode/ACP、Bolt/Lovable/Devin 云端原生导入进入最终归因（含跨导出重复、冲突副本抑制与恢复、被拒/失败编辑、保留的人类上下文、未知模型/时间、项目边界与增量幂等），以及无时间 SWE patch 与中文路径。采集代理另验证原始标准流、子命令参数、退出码、敏感字段过滤和不覆盖文件。

本次完整验证：Rust 250 项通过、1 项保留既有忽略状态；展示/命令行/App 前端 46 项通过。工作区严格 Clippy 检查、CLI 优化构建及包装构建、桌面前端构建、桌面原生编译检查均通过。Tabnine 的实际 CLI 回归覆盖新增映射、清空映射、删除缓存后的作品库恢复，始终保留单份原生模型与 token。Bolt/Lovable/Devin 的测试为契约推导的构造 fixture，真实账户捕获与全平台验收未做。

本机只读验证：ZCode 388 个会话、18,884 条行哈希、0 警告；DSH 37 份压缩会话、4,038 条行哈希；Cursor CLI 33 个可读库中匹配 1 会话、2,342 条行哈希，3 个库的警告保留；Antigravity 5 个原生会话、221 条行哈希，全部保留逐生成模型，无警告。计数是数据读取证据，不是准确率，也没有把用户原文复制进测试。[Antigravity 原生格式与验证](SUPPORT_ANTIGRAVITY.md)。

未完成各产品当前版本的全平台运行验收；没有相应账户的云端入口未作现场验证。本次不发布、不替换已安装 App。
