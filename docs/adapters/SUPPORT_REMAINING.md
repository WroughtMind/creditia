# 最后五家的原生历史：发行物核实与实现

核实日期：2026-10-17。`crates/adapters/src/remaining_native.rs` 新增 Cortex、Nova、Poolside、DimCode 的原生历史读取；Corust 已查到具体产品端阻碍。此文替代 `SUPPORT_LONGTAIL.md` 中这五家旧的“尚无 schema”结论。全部发行物只读下载、解包、静态检查，没有安装、启动 agent、执行发行物或调用模型。构造记录测试不是实际安装后的用户会话验收。

| 来源 | 已实现 | 成功编辑与模型依据 | 明确边界 |
|---|---|---|---|
| Cortex Code | 原生快照/增量日志 + 编辑行 | assistant 嵌套 `tool_use.tool_use_id/name/input` 配对 user 嵌套 `tool_result`，必须 `status=success` 且专属成功回执；支持 `edit`、`apply_patch` | `serializeHistoryEntry` 不保存模型，不用设置里的最后模型。`write` 没有旧文本，不能把覆盖文件的所有行当新增。 |
| Nova | 原生 chat archive、CLI `session.json`、**产品自己已有的 ACP 磁盘快照** + 编辑行 | 完成的 ACP assistant `parts[type=tool].state.status=completed`、输入与输出路径匹配、输出 JSON `success=true`，从真实 `changes[].before/after` 抽取新增；内容块历史也支持配对 tool-use/result | 排除 dry-run、failed/pending/error、`execution.applied=false`。ACP 快照只有可变会话模型，行模型留空；只有消息自己保存的模型可关联该消息工具。CLI 某些路径只保存最终文字，因此这些记录只提供会话，不能还原工具结果。 |
| Poolside | 原生 `.ndjson` trajectory + 编辑行 | Event `type=tool_call.result` 的 `tool_call_result.edit_tool_result.old_contents/new_contents` 是实际结果，排除 `is_error` / `execution_error_kind`；模型来自相同 `step_id` 的推理请求 `chat_completion_request.model` | 不把 approval、parsed 参数、transient 事件当执行成功；`write_tool_result` 只有显示用 lines，缺旧文本，不当完整新增。 |
| DimCode | 只读原生 SQLite `sessions/messages` + 编辑行 | assistant `parts[type=tool_use]` 配对 `role=tool_result`，`toolMetadata.status=success/toolName=edit`，结果 `structuredContent.revision/diff` 来自真实写入之后 | fuzzy matching 用实际 diff，不用请求参数猜最终改动；模型仅在该 `runId` 的 `usage_ledger` 只有一个不同 `modelId` 时关联，多模型/缺账本留空。普通 write 缺旧文本不猜。 |
| Corust Agent | 原生数据目录说明 + 确认产品端阻碍 | 官方 v0.6.0 二进制内明确字符串：`load_session is not implemented in this agent; start a new session instead.` | 没有发现可以核实的会话文件序列化或写入链。`CORUST_HOME` / `~/.corust-agent` 下的模型偏好、权限与记忆并不是历史编辑；不制造会话解析器。 |

所有四个已实现读取器都有 `scan/discover/doctor`；项目扫描使用实际 roots 读取 Nova 项目本地档案，不从 PathMap 的关键词猜仓库路径。所有编辑相对路径经过 `trace_import::push_scoped` 按会话 cwd 解析，并受项目边界限制。新增行证据为单次编辑事件（`single_use=true`）。

## 官方分发与可复核证据

### Cortex Code 1.0.73+180523.e6179a031de9

- [Snowflake 官方 ACP 文档](https://docs.snowflake.com/en/user-guide/cortex-code/cortex-code-acp#sessions-and-conversation-history)确认 `~/.snowflake/cortex/conversations/<id>.json` 与 `<id>.history.jsonl`。
- [官方 darwin-arm64 压缩包](https://sfc-repo.snowflakecomputing.com/cortex-code-cli/a4643c4278/1.0.73%2B180523.e6179a031de9/coco-1.0.73%2B180523.e6179a031de9-darwin-arm64.tar.gz)，SHA-256 `b998e8d4d5a43100712ecdf582498dabe26f53594e336bb3e69805d5b6926184`。
- `cortex` 为 Bun 编译的 Mach-O；静态读取嵌入 JS 的 `buildMetadataObject`、`serializeHistoryEntry`、`W1_` / `Os$`、`Ye$`、`vy$`、`Z7$`、`wy$`。快照字段 `session_id/working_directory/created_at/last_updated/history_length`；日志每行 `role/id/content/request_id/user_sent_time`，同 id 以最后一条为准。备份 `*.back.*.json` 不读。
- `Z7$` 在真正 writeFile 成功后返回 `File edited successfully:`；`wy$` 在全部 patch 操作成功后返回 `Applied patch successfully (`。`edit` 读取 `file_path/old_string/new_string`；`apply_patch` 读取 `input`。
- `serializeHistoryEntry` 只选择上述字段，没有模型；主、子会话相同格式，因此不猜每条模型。

### Nova 1.1.54

- [官方 npm](https://www.npmjs.com/package/@compass-ai/nova/v/1.1.54)，`dist/cli.js` SHA-256 `ded9f0637a9aa2514d80f955c9197a3a6e01f00cfaaba0c9822b9b9d95984fd2`。
- 包内 JS 大量字符串表混淆；只提取字符串表、字面量解码函数与纯数字 wrapper 解码常量，没有执行应用模块或入口。
- `getArchiveDir/q2e -> Sg(project, "chats")`；默认 `<project>/.nova/chats`，旧 `.compass` 回退；global/cloud runtime 保存到 `~/.nova/projects/project-<hash>/chats`。`buildArchive` 产生 `version=1.0/sessionId/projectPath/startedAt/endedAt/messages/model/operations`；`convertMessages` 保留每条 `role/content/timestamp/model/contentBlocks`。可变 archive `model` 不回填旧行。
- CLI `SessionManager.save` 在有效 runtime workspace 保存 `session.json`，含 `id/projectPath/createdAt/updatedAt/state.conversationHistory/state.currentModel`。一部分 CLI 回调只写 user/final assistant 文本，不持久化工具结果；这部分只能读真实会话。
- **另已独立核实 ACPSessionManager 的真实历史文件**：默认 `path.join(Ew(), "acp-sessions")/<id>.json`，即 `~/.nova/acp-sessions/<id>.json`（旧 `.compass` 同规则）。`create/snapshot/enqueueSnapshotState/writeSnapshot` 用 `JSON.stringify` 保存 `id/workingDir/createdAt/lastActivity/conversationHistory`；与 Creditia 自己的 ACP 捕获无关。`onToolComplete` 把已完成工具保存为 assistant `parts[]`，每项 `type=tool/callId/tool/state={status,input,output,error,title}`，成功 output 为实际工具结果 JSON。
- `registerTool("edit_file")` 先读真实旧文件，匹配/修改，落盘后才返回 success；返回 `dryRun/execution.applied` 明确区分预览。`Woe` 用真实 before/after 生成 `changes[]`，每段 `before/after` 是带 `"<行号>: "` 前缀的实际行；解析先移除行号，再按行相减，保留上下文不算新增。
- `write_file` 返回 `success/operation`，仅小文件或指定类型保留 changes；缺 changes 的覆盖写入不读。敏感模式可隐藏内容，不绕过隐私缺失制造行证据。

### Poolside pool 1.0.16

- [官方源码/文档仓库](https://github.com/poolsideai/pool/tree/fcabc59)、[官方分发包](https://downloads.poolside.ai/pool/v1.0.16/pool-darwin-arm64.tar.gz)，SHA-256 `0932af3eb2b57a863acacb664ec8b2b1d3a76c2570a788b086001608cc585f74`（与 ACP registry 发布校验和一致）。
- 分发为 Go Mach-O。没有运行目标文件：读取 Mach-O `__gopclntab` 定位函数，读取 Go `__typelink` / runtime struct metadata 恢复字段名称、类型、JSON tags；静态反汇编确认串行化链。
- `common/userconfig.Directory`（地址 `0x1004393d0`）读取 `XDG_CONFIG_HOME`，否则 homedir + `.config`。`storage.TrajectoryFilePath`（`0x100a36b70`）追加 `poolside/trajectories`，文件前缀 `trajectory-`、后缀 `.ndjson`。`storage.WriteEvent`（`0x100a37350`）对 `trajectory.Event` JSON marshal 后追加 byte 10，并调用文件 Write；`ReadTrajectory` 解码同一 Event 类型。这是本地磁盘格式，非网络 API 结构猜测。
- `trajectory.Event` 的 JSON tags：`id/step_id/timestamp/type/is_transient/session_start/tool_call_inference_start/tool_call_result`。`session_start.working_directories` 提供 cwd；`ToolCallInferenceStart.ChatCompletionRequest.Model` 的 tag 为 `model`。
- `ToolCallResult` tags `id/tool_name/is_error/execution_error_kind/edit_tool_result`。`results.EditToolResult` tags `path/old_string/new_string/old_contents/new_contents`。`old_contents/new_contents` 存完整实际前后文本。Go 布尔 `is_error,omitempty` 缺省为 false；error kind非空仍排除。

### DimCode 0.5.19

- [官方 npm wrapper](https://www.npmjs.com/package/dimcode/v/0.5.19) 指向平台 optional package；此次读取 [darwin-arm64 平台包](https://registry.npmjs.org/dimcode-darwin-arm64/-/dimcode-darwin-arm64-0.5.19.tgz)，SHA-256 `5c8678211d96bfe75fb4849f794b7e4645e515e6d55529fd6f9168f528dc1006`。
- `bin/dimcode` 为 Bun Mach-O，嵌入的 JS 保留来源段注释。静态读取 `src/agent/storage-adapter.ts`、`../agent/src/tool/builtin/edit.ts`、`../agent/src/session`、`src/config`。
- CLI ConfigManager 用 `DEFAULT_HOME_DIR=.dimcode/v2`、`STORAGE_DB_FILE=dimcode.sqlite`。CLI 的 `DIMCODE_HOME` 通过 `resolveCliHomeDirFromEnv` 规范化：末段不是 `v2` 就追加 `v2`。插件工具自己的 `resolveDimcodeHome` 不等于 CLI ConfigManager，未把二者混为数据库路径。
- schema：`sessions(sessionId,cwd,createdAt,updatedAt,...)`；`messages(messageId,sessionId,role,parts,toolMetadata,metadata,orderKey,createdAt,updatedAt)`，JSON 字段以 TEXT 存储。`usage_ledger(sessionId,runId,providerId,modelId,...)`。
- `createToolResultMessageRecord` 在工具执行结束后创建 `role=tool_result`，part 为 `tool_result/tool_use_id/content/structuredContent/is_error`，`toolMetadata.status` 从真实 `result.isError` 得到 success/error。`createEditTool.execute` 在打开、写入、关闭目标文件成功后返回 `structuredContent={path,replacementCount,revision,diff}`。失败返回 `isError=true`；修复孤儿工具的合成结果也明确 `status=error/is_error=true`。
- `diff` 由真正 beforeLf/afterLf 的 `structuredPatch` 产生，保留 hunk 行；因此只取 `+` 行，不受模糊匹配导致“请求 oldText 与真实旧文本不同”的误差影响。

### Corust Agent v0.6.0

- [官方 release manifest](https://github.com/rustingai/corust-agent-release/blob/6222c2a/extension.toml) 指向[官方 darwin-arm64 分发](https://github.com/Corust-ai/corust-agent-release/releases/download/v0.6.0/agent-darwin-arm64.tar.gz)，SHA-256 `6a74d46f645774f0a7e2a95371bad39d63927e231a34d52d2800c79f508f025f`。
- Rust Mach-O 静态字符串、符号及 serde 字段中，找到 `acp/src/agent.rs` 的明确产品错误 `load_session is not implemented in this agent; start a new session instead.`；同时找到 `Failed to persist workspace model selection` 与配置保存链，不能把这类“保存模型偏好”误认为“保存旧会话”。
- `config/src/lib.rs` 明确 `CORUST_HOME` / `.corust-agent`、`workspace-settings.yaml`、`command-permissions.yaml`。读取器只报告这个已核实数据目录，不从这些文件伪造历史。
- 检查未发现原生 JSONL/SQLite 会话序列化或 save_session 契约；仅“没有字符串”不是无历史的证明，本文的可确认结论限于官方当前 load_session 未实现及未找到可核实的旧编辑持久化读取入口。

## 验证

`CARGO_TARGET_DIR=/private/tmp/creditia-agent-target cargo test -p creditia-adapters remaining_native --offline`。

构造测试覆盖：Cortex 嵌套调用/成功配对、失败/pending/假成功排除、JSONL 最终消息去重、split history 指纹变动、相对 cwd；Nova 两条 assistant 不同模型、真实 changes 上下文扣除、缺 before 排除、native ACP 磁盘快照 completed/error/pending/preview 与最后模型不回填；Pool 同一步模型、真实 old/new、错误/审批/缺旧文本排除；DimCode 原生 SQLite schema、实际 diff、成功/错误、多模型 run 留空。

未启动上述应用，没有真实本机用户会话验收；未发布、提交或推送。
