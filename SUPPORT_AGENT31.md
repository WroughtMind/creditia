# 剩余 31 个 Agent 的逐项核查

> 本页保留前一轮的版本与结论。随后已继续核查下述 18 项并新增 7 个原生读取器；当前统计为 67 个历史入口、3 个宿主、8 个待验证格式，Hero 已补到 55 个标志。请以 [SUPPORT_AGENT18.md](SUPPORT_AGENT18.md) 和 [当前覆盖表](AGENT_SUPPORT.md) 为准。

核查日期：2026-10-23。范围是上一版目录中 `history=unconfirmed` 的 31 项，不是新找的 31 个产品。机器可读结果在 [agents.json](packages/schema/agents.json)，`creditia agents` 也会显示逐项发现。

本轮增加 **13 条读取路径**：11 个原生历史读取器、AdaL 官方命令行导出、CodeArts 已知数据库结构的条件读取。连同原有 47 项，目录现有 60 个历史入口；另外 18 项尚无独立逐行读取器。**这不是 60 个产品完整适配或真实账号验收。**

所有新路径进入 CLI、App 共用的 `scan_all` / `discover_all` / `doctor_all`。没有项目路径的记录不会制造自动发现结果；已识别历史并不意味着其中每种工具、版本或远程文件系统都已覆盖。读取只访问历史，不执行厂商代码，不运行历史中的命令，不修改原日志。

## 已实现的 13 项

源码链接固定到本轮查阅的上游提交；它们定义测试记录的结构。CodeArts 和 AdaL 另以官方文档合同为准。

| Agent | 原生位置与已实现格式 | 成功编辑与模型边界 | 一手依据 |
|---|---|---|---|
| AiderDesk | `<repo>/.aider-desk/tasks/<id>/{context,settings}.json`；context v2 | 配对 Power `file_write/file_edit` 成功回执，或 Aider 带 commitHash 的实际 diff。模型取消息 usageReport；不取当前 settings。正则/反斜杠替换不猜 | [持久化与消息](https://github.com/hotovo/aider-desk/blob/1958b399a2133d6a735a782a2a9676a7de568150/src/main/task/task.ts) |
| AstrBot | `ASTRBOT_ROOT/data/data_v4.db`，桌面默认 `~/.astrbot`；`conversations.content` | 配对内置文件写入/字面替换回执；只认能映射回仓库的绝对路径。会话无逐消息模型/cwd，时间仅会话更新时间；远程相对路径不猜 | [数据库](https://github.com/AstrBotDevs/AstrBot/blob/0cd42bd4dd216d8810da4e3f5fb2e381d3c6817e/astrbot/core/db/po.py)、[文件工具](https://github.com/AstrBotDevs/AstrBot/blob/0cd42bd4dd216d8810da4e3f5fb2e381d3c6817e/astrbot/core/tools/computer_tools/fs.py) |
| CodeArts Agent | `~/.codeartsdoer/{codearts-data,cli-data}/opencode.db` | **条件读取**已知 v1 `session/message/part` 列结构，且工具 completed、保留实际 unified diff。未知结构报错；没有厂商实录，旧 IDE chat_sessions 与无 diff 工具未覆盖 | [IDE 位置](https://support.huaweicloud.com/intl/zh-cn/usermanual-codeartsagent/codeartsagent_ug_0033.html)、[CLI 位置](https://support.huaweicloud.com/usermanual-cli/codeartsagent_cli_0014.html) |
| Deep Agents | `DEEPAGENTS_SESSIONS_DB` 或 `DEEPAGENTS_HOME/.state/sessions.db`；默认 `~/.deepagents` | LangGraph 最新根 checkpoint 加 pending writes；JSON/MessagePack 数据解码、Overwrite/RemoveMessage。配对 CLI 本地 write/edit 成功回执，模型取 response_metadata。时间是 checkpoint 更新时间；任意框架后端和子图命名空间不冒充本地文件 | [官方历史重建脚本](https://github.com/langchain-ai/deepagents/blob/708efd251cf4ac5e0138bd34b79edcd357af14fc/libs/code/deepagents_code/built_in_skills/deepagents-thread-inspector/scripts/inspect_sessions.py)、[文件中间件](https://github.com/langchain-ai/deepagents/blob/708efd251cf4ac5e0138bd34b79edcd357af14fc/libs/deepagents/deepagents/middleware/filesystem.py) |
| Dexto | `~/.dexto/database/*.db` 与项目 `.dexto/database`；SQLite `kv_store/list_store` | session/workspace 项解析 cwd，配对 toolCalls 和成功 tool 行；写入与字面替换，拒绝/失败排除。模型取消息；PostgreSQL、内存库不在本地 SQLite 范围 | [原生存储源码](https://github.com/truffle-ai/dexto/tree/dbb401dabbf1b41a0ec6f7b39e1fb3266bffd701/packages/core/src/storage)、[写入回执](https://github.com/truffle-ai/dexto/blob/dbb401dabbf1b41a0ec6f7b39e1fb3266bffd701/packages/tools-filesystem/src/write-file-tool.ts) |
| ForgeCode | `FORGE_CONFIG/.forge.db`；默认 `~/.forge/.forge.db`，存在旧 `~/forge` 时遵从上游优先级 | `conversations.context` 中 text/tool 配对，fs_write/patch/multi_patch 成功的实际 FileDiff 或带行号的 XML diff。模型取消息；时间仅会话创建时间，工作区 hash 不反推路径 | [目录规则](https://github.com/tailcallhq/forgecode/blob/cd9e94d1ec5498ff545eb45a3aaed6eef521d5b6/crates/forge_config/src/reader.rs)、[消息合同](https://github.com/tailcallhq/forgecode/blob/cd9e94d1ec5498ff545eb45a3aaed6eef521d5b6/crates/forge_domain/src/context.rs) |
| fx | Vercel fx：`~/.fx/sessions/v2/*/log.jsonl` 与 blobs | 校验 CRC32C、连续 seq、SHA-256 数据块；取成功 tool_result 的 committed_file_presentation。完整保存内容优先，截断预览排除；按事件当时 prefs 记录模型，fork 复制前缀排除。旧二进制 v1 未解码 | [日志协议](https://github.com/vercel-labs/fx/blob/d824f609cfbb6262e6bb9272f1c2f89d6cd24bec/src/core/session_manager/schema.zig)、[事件适配](https://github.com/vercel-labs/fx/blob/d824f609cfbb6262e6bb9272f1c2f89d6cd24bec/src/core/session/session_adapter.zig) |
| Jazz | `JAZZ_HOME/history/**/*.jsonl` 及 `.jsonl.gz`；默认 `~/.jazz` | conversation v1–4，配对成功文件结果；去 ANSI 后完整 diff，或明确新建回执。截断/脱敏内容排除；消息无 cwd/模型，绝对路径才能归项目 | [原生日志与归档](https://github.com/lvndry/jazz/blob/579c2f6ef06005a6c1578ba7769afbf67b7547b3/packages/adapters/src/history/conversation-log.ts) |
| Kode | `KODE_CONFIG_DIR/projects/**/*.jsonl`；默认 `~/.kode` | Write/Edit 成功 `toolUseResult` 的 create/structuredPatch；模型取生成消息，子 agent 独立身份。文件快照不当作编辑；旧 `messages/*.json` 重写了全部时间和会话身份，本轮不解码为历史编辑 | [序列化与子 agent 路径](https://github.com/shareAI-lab/Kode-CLI/blob/400bbf0449707846465895ef64cfbbd18bea4763/packages/protocol/src/utils/kodeAgentSessionLog.ts)、[旧快照重写](https://github.com/shareAI-lab/Kode-CLI/blob/400bbf0449707846465895ef64cfbbd18bea4763/packages/core/src/logging/log/messages.ts) |
| Mux | 当前上游已改名 Xum；`XUM_ROOT/MUX_ROOT` 或 `~/.xum/.mux/.cmux`，config.json 与 sessions | active/archive 合并，后写的同 ID 消息优先；成功 file_edit diff，包括 code_execution 内嵌工具。session-usage.json 的 forkedAt 排除复制前缀；模型取消息 | [历史服务](https://github.com/coder/xum/blob/a3cc8ea39492072d266ae0913d6b854f6ec188e7/src/node/services/historyService.ts)、[fork 边界](https://github.com/coder/xum/blob/a3cc8ea39492072d266ae0913d6b854f6ec188e7/src/node/services/workspaceService.ts) |
| Pochi | 持久化的 LiveStore **materialized SQLite** `tasks/messages` | 仅 output-available 且 success 的 writeToFile/applyDiff/multiApplyDiff；实际 `_meta.edit` 优先。无格式化时才用可验证字面输入。当前 task.modelId 会变，历史模型留空。CLI 内存模式没有落盘库；VS Code OPFS 需导出物化数据库 | [工具合同](https://github.com/TabbyML/pochi/tree/81ca233451a223e7897676a9e28ed4c9e20d92e4/packages/tools/src)、[保存/格式化实现](https://github.com/TabbyML/pochi/blob/81ca233451a223e7897676a9e28ed4c9e20d92e4/packages/vscode/src/tools/write-to-file.ts) |
| Reasonix | `REASONIX_STATE_HOME/REASONIX_HOME` 的 sessions；macOS/Linux 默认 `~/.reasonix`，Windows 为 `%APPDATA%/reasonix` | flat JSONL、v1/v3/v4/v5 append/replace、schema-2 DAG；应用 patch/redact/rewind/select/retire 后读取当前分支。写文件须字节数与路径回执吻合；编辑须真实写后 replacement receipt。模型取 modelRef；预执行 Diff 不算成功 | [旧事件协议](https://github.com/esengine/DeepSeek-Reasonix/blob/edfc6b12b6ea68e883eebbc057c061f65efba05c/internal/state/sessionstore/session_events.go)、[DAG 重建](https://github.com/esengine/DeepSeek-Reasonix/blob/edfc6b12b6ea68e883eebbc057c061f65efba05c/internal/state/sessionstore/session_dag_read.go) |
| AdaL | 官方 `-o json` / `-o stream-json` 原始导出 | 仅配对 completed `replace_by_string` 与精确 `Edited <path>` 回执；拒绝/失败排除，要求完整结束 envelope。最终 model 不是逐调用模型，不使用；时间未知。原生 HTML 历史与其他工具仍未解码 | [完整 JSON](https://docs.sylph.ai/integration/headless/)、[NDJSON 字段与真实示例](https://docs.sylph.ai/integration/event-stream/) |

### 指定已有记录

默认目录可直接发现的记录无需配置。另有覆盖路径用于历史备份或非默认位置：

| Agent | 路径覆盖 |
|---|---|
| AiderDesk | `CREDITIA_AIDER_DESK_DIR` 指向含原始 tasks 布局的目录；相邻 settings 必须保留 |
| AstrBot | `CREDITIA_ASTRBOT_DIR` 指向原根目录或 data_v4.db |
| CodeArts | `CREDITIA_CODEARTS_AGENT_DIR` 指向 .codeartsdoer 或数据库文件 |
| Deep Agents | `CREDITIA_DEEPAGENTS_DIR` 指向 profile 根或 sessions.db；原生 `DEEPAGENTS_SESSIONS_DB` 也支持 |
| Dexto | `CREDITIA_DEXTO_DIR` 指向含 database 的根或一个 SQLite 文件 |
| ForgeCode | `CREDITIA_FORGECODE_DIR` 指向原根目录或 .forge.db |
| fx / Jazz / Kode / Mux / Reasonix | 各自的 `CREDITIA_FX_DIR` / `CREDITIA_JAZZ_DIR` / `CREDITIA_KODE_DIR` / `CREDITIA_MUX_DIR` / `CREDITIA_REASONIX_DIR` 指向原根目录 |
| Pochi | `CREDITIA_POCHI_DIR` 指向物化数据库或其目录；不是 LiveStore event log 数据库 |
| AdaL | 把**未改写的原始导出**放在目标仓库 `.creditia/imports/adal/*.{json,jsonl,ndjson}`，目录所在仓库提供相对路径含义。`CREDITIA_ADAL_DIR` 可指向外部导出，但外部记录必须自带绝对文件路径 |

## 其余 18 项：查到了什么，还缺什么

“没有确认格式”不表示产品没有保存记录，更不表示记录被工具清理了。以下明确区分官方已经确认的入口和本次尚未得到的证据。

| Agent | 已查明的原生形态 | 本轮决定与尚缺的证据 | 一手依据 |
|---|---|---|---|
| Codemaker | 公开 Go CLI 调 API 后直接 os.WriteFile，公开实现未见会话持久化 | 不制造历史日志；IDE 产品可能不同，需其真实导出/序列化实现 | [官方 CLI](https://github.com/codemakerai/codemaker-cli/blob/dev/cli/cli.go) |
| Code Studio | Syncfusion v2 会话界面按工作区显示，旧版本历史不能迁移至 v2 | 官方页面未给磁盘文件结构；需要 v2 原始会话和成功编辑记录，不能套用 VS Code/Copilot schema | [官方会话说明](https://help.syncfusion.com/code-studio/troubleshoot/access-session-history-files) |
| Eve | Workflow 保存 durable session，history 独立传递；NDJSON 有 actions.requested/action.result | 原生事件外壳已知，工具由应用自行定义；缺具体文件工具成功回执和沙箱到仓库的映射 | [durable state](https://github.com/vercel/eve/blob/main/packages/eve/src/execution/durable-session-store.ts)、[事件协议](https://github.com/vercel/eve/blob/main/docs/concepts/sessions-runs-and-streaming.md) |
| Firebender | JetBrains agent 有检查点；Local Mode 用自定义模型运行 | 确认功能并不足以确认保存 codec；需要原始会话/检查点及文件操作状态 | [检查点](https://docs.firebender.com/input/checkpointing)、[Local Mode](https://docs.firebender.com/configurations/local-mode) |
| inference.sh | CLI 发起云端 app run，结果是各应用自己的输出 | 成功 run 不等于成功代码编辑；需具体 coding app 的文件结果合同与仓库映射 | [官方 CLI](https://inference.sh/docs/extend/cli-setup)、[官方技能](https://github.com/inference-sh/skills/tree/main/tools/infsh-cli) |
| Lingma | 官方已迁移到 Qoder CN；Cloud Agents 事件 API 有 tool_use/tool_result | 云事件 envelope 可确认，但内置文件工具字段/本地仓库映射不足；迁移公告不证明旧 Lingma IDE 与 Qoder CLI 日志相同 | [产品更新](https://help.aliyun.com/zh/lingma/qoder-cn-update-log)、[事件历史](https://help.aliyun.com/zh/lingma/cloud-agents-cn/query-event-history) |
| Loaf | 正确项目为 levifig/loaf：工作日志/协调工具，实际代码操作交给宿主 agent | **由宿主历史归因**，不把工作日志重复算成 Loaf 编辑；不是另一个同名覆盖层项目 | [官方实现与产品说明](https://github.com/levifig/loaf) |
| MCPJam | 云端 Playground `sessions show/trace` 可读 raw messages/spans；browser.json 是浏览器连接状态 | 任意 MCP 工具结果没有统一文件语义；需具体 server 的写入回执和本地项目映射，不读连接凭据凑历史 | [官方 CLI 会话](https://github.com/MCPJam/inspector/blob/c6c376ef07d5edf68d5f26775952def18b98cebe/cli/src/commands/sessions.ts) |
| Moxby | 正确产品为 moxby.com，浏览器扩展、桌面桥和 replay/诊断 | 页面回放、第三方日志导入功能不是原生代码写入证据；缺编辑历史导出和成功结果 schema | [官方更新](https://moxby.com/changelog) |
| Ona | 云端环境，官方支持 Codex 等 agent | 宿主记录可交给对应 reader；没有获得独立 Ona agent 原始编辑导出，不对云端目录作本地猜测 | [Ona Codex](https://ona.com/docs/ona/agents/codex)、[数据持久化](https://ona.com/docs/ona/environments/persistent-storage) |
| Posit Assistant | 当前产品默认存储目录 `~/.posit/assistant`；可导出完整 JSON 会话树，压缩不删除原树 | 已确认位置和导出入口；缺当前节点编码、工具成功字段和项目路径样本。不能套用已停用的旧 Positron Assistant | [会话导出](https://assistant.posit.co/docs/features/conversations/)、[storage.path](https://assistant.posit.co/docs/reference/config-file/) |
| Rovo Dev | `~/.rovodev/sessions/<id>/session_context.json` + metadata.json；有 fork/prune | 位置确定；需原始 message_history/tool-return schema 及 fork 归属。只凭文件名编写 Pydantic/Anthropic 猜测解析器会误算 | [官方会话管理](https://support.atlassian.com/rovo/docs/manage-sessions-in-rovo-dev-cli/) |
| Sarvam Code | 官方 early beta，建立在 Codex 基础上，并有模型路由 | “基于 Codex”不证明日志完全兼容；需当前版本 rollout/成功编辑记录 | [官方产品说明](https://www.sarvam.ai/epoch/summary) |
| Terramind | Nucleus 多 agent 工作区，含 Claude Code/Codex/Grok/Terramind | 可从实际宿主记录归因；Terramind 自身和云端路径映射还缺原始历史。不是 IBM 同名地理模型 | [官方工作区](https://terramind.com/nucleus) |
| Tinycloud | 官方 cloudglue/tinycloud launcher 下载校验后的原生二进制，另有视频/图像技能 | `.tinycloud/versions` 是程序缓存，不是会话库；未公开可验证的原生编辑序列化，需要真实导出 | [官方 launcher/skills](https://github.com/cloudglue/tinycloud) |
| Zencoder | IDE agent/会话、模型选择与 MCP；和 Zenflow 是不同入口 | 没有证据证明其磁盘 schema 等于 Cline/OpenCode；需成功编辑持久化样本 | [官方 Coding Agent](https://docs.zencoder.ai/features/coding-agent)、[IDE 入口](https://docs.zencoder.ai/get-started/install-ide-plugin) |
| Zenflow | 本地 worktree、每轮 checkpoint、可编排不同宿主 agent | git checkpoint 可显示变更，但整次变更不能自动归给当前模型；需每轮执行者和已接受文件结果的原始映射 | [变更与检查点](https://docs.zencoder.ai/zenflow/tracking-changes)、[宿主 CLI](https://docs.zencoder.ai/clis/overview) |
| PromptScript | 指令编译器；公开 telemetry 是 command/feature 次数、耗时和 outcome | **由执行宿主归因**；编译成功和遥测成功不代表代码编辑 | [官方 agent 指令功能](https://getpromptscript.dev/features/agents/)、[遥测字段](https://github.com/mrwogu/promptscript/blob/main/packages/telemetry/src/types.ts) |

## 验证与限制

新增集成测试建立真实 Git 测试仓库，构造与上游序列化一致的 JSONL/SQLite/官方导出，运行真实 CLI：13 项各 8 行进入证据库和最终 `credits.json`，再次扫描仍各 8 行；将原生成功回执改为失败后，各项持久证据均撤回。同一条集成测试还切换了 Reasonix schema 1–5、Jazz gzip、Deep Agents MessagePack 和 fx 外部数据块；另有解析器测试覆盖失败、未配对、错误项目、DAG rewind/redact、模型留空、MessagePack 数据解码和 CRC/外部数据块篡改。构造记录验证不等于真实厂商账号验收。

Mux 的 archive/active 重复消息与 fork 前缀有集成覆盖；Kode 子 agent 独立身份防止合并时覆盖主会话。复制的同会话数据库按最近修改的完整读取结果去重；不能把一个备份目录里互相冲突的不同版本都作为独立会话。当前成功读取的空/失败回执可撤回证据；缺失或损坏的原文件不是撤回授权，保留已存证据并报告读取错误。

未知模型保持未知。没有历史时间的格式不会使用导入时间伪装编辑时间；AstrBot、ForgeCode、Deep Agents 的可用时间粒度已经逐项列明。未验证所有历史版本、云端文件同步、Windows/Linux 实机和已安装 App；未发布或部署。

Hero 使用原图标区的 40 个有来源 SVG，去掉误加的独立 logo 墙；桌面和窄屏检查包括中英文 320px 的边缘提示框、触屏模拟和真实资源加载。未进行真实手机 Safari 验收。logo 的出现不代表该工具已经完整适配。

最终检查：Rust 工作区 308 通过（1 个原有重型测试默认忽略），App Rust 8、renderer 29、CLI 9、App 前端 27，共 381 通过；工作区和 App Clippy、CLI bundle、App 前端构建通过。现有重复 units 属性和 Vite chunk 大小提示仍存在。
