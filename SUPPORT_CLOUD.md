# 云端、编辑器与运行轨迹读取范围

核查日期：2026-10-17。这里区分“读取成功编辑”“只读会话元数据”和“尚缺可信数据契约”。读到了目录、Git 提交署名或项目源码，不代表读到了 agent 编辑记录。所有导入都在本机只读进行；creditia 不登录账户，不安装 hook，也不替用户调用云端接口。

| 来源 / 入口 | 当前实现 | 证据边界 |
| --- | --- | --- |
| Windsurf Cascade / Devin Desktop Cascade | 已有 hook 日志与 transcript 的成功编辑 | `post_write_code` 的 `tool_info.edits`；或 transcript 中 `status=done` 的 `code_action.path/new_content`。hook 的 `model_name` 原样保留；没有模型或时间的 transcript 不补猜。原生 `.pb` 未解码。 |
| Devin Cloud（ACP 导入） | 原生 ACP 更新的成功编辑 | 离线读取网页客户端原生 envelope（loadPage 响应、session/update 与 session_update_batch）：最终 completed 且 kind=edit 的 diff 正文；引用 key 的正文跳过；模型取同回合 agent 消息唯一值，时间取工具自身 timestamp。详见[协议核查](SUPPORT_DEVIN.md)。 |
| Devin Cloud（v3 元数据） | 会话元数据 | 官方 v3 会话 ID、时间和 PR 仓库关联。公开 `SessionResponse` / `SessionMessage` 没有逐文件编辑正文或模型字段；`devin_mode` 不是模型名。被 ACP 导入覆盖的会话不再重复计 CLI hook 副本。 |
| Devin CLI / Devin Local agent | 成功 hook 的有限编辑 | 读取 `PostToolUse`、`tool_response.success=true`、无 error 的 `exec`，只解析命令里标准 `apply_patch` 的绝对文件路径。官方 edit 工具入参结构尚未确认；不假定与 Cascade 相同。 |
| Jules | 产生的变更补丁 | 读取 agent activity 的 `artifacts[].changeSet.gitPatch.unidiffPatch`；须同时提供官方 Sources 响应，以 `githubRepo.owner/repo` 把 source 资源 ID 关联到仓库。无模型字段时保持未知。 |
| OpenHands | 0.60 格式的成功文件工具 | 读取官方序列化事件 JSON / JSONL；agent 的 write/edit 通过 `cause` 和 path 与 file observation 配对。支持 native `events/<number>.json` 目录内配对；error、明确未应用的编辑不计。现代 SDK 格式尚未据此宣称兼容。 |
| SWE-agent | 真实提交产物 | `.traj` 的 `info.exit_status=submitted` / `submitted (...)`，以及真实 `info.submission` 统一补丁；仓库与模型来自 `replay_config`。不把轨迹里的拟执行 action 当成功编辑；无可靠墙钟时间时留空。 |
| mini-SWE-agent | 1.1 格式成功工具调用 | `.traj.json` 的 `extra.actions` 与 tool result 以 `tool_call_id` 配对，要求 `returncode=0` 且无 exception。支持直接执行的标准 apply_patch、带引号 delimiter 的 literal cat / tee heredoc；其他 shell 写法不推断。 |
| Warp Agent 本机 / 云会话任务列表 | 原生成功编辑 | 读取 `warp.sqlite` 的 `agent_conversations` 与 protobuf `agent_tasks.task`；也读公开客户端 GraphQL `ListAIConversations` 的 `finalTaskList`（base64 编码的官方 ConversationData）。`apply_file_diffs` 请求必须与同一 tool_call_id 的 `Success` 结果配对，并核对返回文件正文；排除失败、未返回对应文件、人工改过和已撤销操作。允许覆盖的 new_files 缺少旧正文，不当作全新文件归因。模型按 request_id 对应真实 `ModelUsed.model_id`，保留消息时间。另支持 ConversationData 二进制 / protobuf JSON。 |
| Warp Oz 运行信息 | 官方 Oz run 元数据 | run/session 时间、PR 仓库和 `agent_config.model_id`。与本机记录的 conversation_id / run_id / server_conversation_token 匹配时优先本机记录，不重复计会话。Oz 通用 action input/output 未限定编辑结构；Markdown 导出尚无可验证的稳定机器格式。 |
| v0 | v2 message 元数据 | `usage.model`、token 与时间；只有 file-edit 中提供可映射项目的绝对 path 才进入项目。当前 v2 file-edit parts 明确只含 operation/path/time，没有正文或 diff。Get Chat Files 返回当前完整文件，但没有历史 message/snapshot 参数，不能确定是哪一次 AI 编辑产生的内容。v1 version 快照混有导入或人工文件，未作每行 agent 证据。 |
| Replit Agent | 官方原始响应 / 录制的成功编辑 | 当前 Timeline SyncBlocksResponse 与官方 feed recording v1；重放 block/update/close/reset，读取 completed CreateFile/FileEdit 的实际正文/差分。跨文件按原生 block 身份合并；只接受可映射的绝对路径，模型未知。详见[当前客户端契约](SUPPORT_CLOUD_NATIVE.md)。 |
| Bolt.new | 原始 HAR 已证实的保存链 | 离线读取原始 HAR 中互相核对的回合起始提交、聊天工具正文、bolt-merged 回执、apply-changes 提交正文与同一 baseSha 的原文；只计验证过的新增行，模型与执行时间未知。详见[保存链核查](SUPPORT_BOLT.md)。 |
| Lovable | 原始 trajectory 的官方 Hunk 链 | 已闭合执行结果到 HunkDiffViewer 的直接消费链；相对路径须由原始 HAR 中同项目 trajectory 与已同步 gitsync.repo_url 绑定。详见[链路核查](SUPPORT_LOVABLE.md)。 |
| TRAE 编辑器 | 原生 SQLCipher 成功编辑 | 官方 3.6.2、3.6.4 的固定 codec、原生表结构与 task 成功回执已核对；直接读取实际前后差分，执行模型未知。官方 SQLCipher 加密的构造原生测试库已通过 CLI 最终归因，真实账号另验。详见[原生读取范围](SUPPORT_TRAE.md)。TRAE CLI 单独读取。 |
| Antigravity 编辑器 | 完整原生会话与成功编辑 | 实际 conversations SQLite / protobuf 中 DONE 的 ActionResultEdit 差分及逐生成模型；排除失败、拒绝、上下文。摘要只补原生记录缺少的会话，详见[原生格式与验证](SUPPORT_ANTIGRAVITY.md)。CLI 另算。 |
| DeepSeek Harness（DSH） | native v0-v4 成功编辑 | assistant `tool-call` 与 tool/result 的 callId 配对，显式 `isError=false`；v0-v3 wrapper 与 v4 tool-role 分别读取。支持 write/edit/str_replace_editor，保留每次生成模型和 provider、usage，跳过 seeded 继承历史。 |

## 导入与检查

每种来源的目录由既有环境变量指定，例如 `CREDITIA_JULES_DIR`、`CREDITIA_SWE_AGENT_DIR`、`CREDITIA_MINI_SWE_AGENT_DIR`、`CREDITIA_OPENHANDS_DIR`、`CREDITIA_DEVIN_DIR`、`CREDITIA_WARP_DIR`、`CREDITIA_V0_DIR`。可以指向单个导出文件或包含导出文件的目录；只识别各家已确认的格式，不接受自编的通用“成功编辑 JSON”。

```sh
CREDITIA_JULES_DIR=/path/to/jules-exports creditia doctor --json
CREDITIA_JULES_DIR=/path/to/jules-exports creditia analyze --repo /path/to/repo --fresh --json
```

Jules：用已有账户自行保存官方 `GET https://jules.googleapis.com/v1alpha/sources` 和 `GET .../sessions/{sessionId}/activities` 响应为 JSON，放入同一导入目录。分页响应按各自 `nextPageToken` 保存完整页。只下载 activities 而没有 Sources 的仓库映射时，不把同名相对路径的补丁猜配到本地仓库。

SWE-agent：把运行产生的 `.traj` 原文件放入 `CREDITIA_SWE_AGENT_DIR`，保留 `replay_config`。mini-SWE-agent：使用 `trajectory_format=mini-swe-agent-1.1` 的 `.traj.json` 原文件；`info.config.environment.cwd` 须能关联本项目。Docker / 远程路径不同时，可使用已有 `creditia link /remote/repo --repo /local/repo` 关联实际路径。OpenHands：指向 0.60 serialized trajectory 导出目录，或包含各会话 numbered event 文件的持久化目录。

Cascade：默认读取 `~/.windsurf` 下已有 JSON / JSONL（包括官方 `transcripts`）；`CREDITIA_WINDSURF_DIR` 可改为现有采集目录。`post_cascade_response_with_transcript` hook 启用后，官方会写 `~/.windsurf/transcripts/{trajectory_id}.jsonl`。`post_write_code` hook 的 stdin 若已由用户记录为每行一个完整 JSON，也可直接读取，且能保留模型显示名。安装 / 启用 hook 是用户自选动作，creditia 不自动改 hooks.json；启用前的未导出历史不会因此自动补齐。

Devin CLI：将现有 hook 的原始 `PostToolUse` stdin 逐条保存成 JSONL，指向 `CREDITIA_DEVIN_DIR`。目前只读取成功 exec 中带绝对路径的标准 apply_patch；相对路径没有可靠工作目录字段时不会猜配。Devin Cloud：v3 元数据自行保存官方 Get/List Session JSON 响应；编辑证据需自行捕获网页客户端的 ACP 帧（loadPage 响应或 session_update_batch 批次）保存为 JSON/JSONL 放入同一目录，详见 SUPPORT_DEVIN.md。

Warp：默认检查 macOS `~/Library/Application Support/dev.warp.*` 及 `~/Library/Group Containers/*.dev.warp/Library/Application Support/dev.warp.*`；Linux 检查 `$XDG_STATE_HOME/warp-terminal*` / `warp-oss*`（默认 `~/.local/state`）；Windows 检查 `%LOCALAPPDATA%/warp/*/data`。包含各自 `tui/warp.sqlite`。`CREDITIA_WARP_DIR` 可直接指向本机原始 `warp.sqlite`、其目录，或已有官方 `ConversationData` 的 `.pb` / `.json`；后两种格式不含会话 ID，使用原文件名作导入 ID，建议保持原 conversation_id 文件名。数据库通过只读连接读取且不忽略 WAL；异机复制应使用正确的 SQLite 备份，不能只复制数据库主文件而漏掉仍在使用的 WAL。相对文件路径按该调用的官方 InputContext.directory.pwd 关联项目；远程目录可使用既有 `creditia link`。只读取申请的新增内容与成功结果共有的证据，不把结果的整个文件算作 AI 新写。未覆盖任意 shell 命令写文件。

Warp Oz / v0：自行保存官方 Get/List Run 或 Get/List Message 原始响应，分别指向 `CREDITIA_WARP_DIR` / `CREDITIA_V0_DIR`，仅作为运行信息。通常为相对 path 的 v0 导出无法可靠关联本地项目，不会跨项目按文件名匹配。没有编辑正文的导出不会提高代码行归因覆盖率。Devin Cloud 的 message 文本、attachment 下载 URL 和未限定结构的 structured_output 也不转换成成功编辑。

Warp 云会话成功编辑：若已有公开客户端使用的 `ListAIConversations` 原始 HTTP JSON 响应，可放到同一 `CREDITIA_WARP_DIR`，保留 `data.listAIConversations.__typename=ListAIConversationsOutput` 与 `conversations[].conversationId/finalTaskList/harness`。只接受 `harness=OZ` 的任务列表；第三方 agent 的终端快照不冒充 Warp 自有 agent。工作目录来自该响应的 `workingDirectory` 或任务内 InputContext。当前完整响应与 native DB 如能以 server conversation token 对上，只计一次，并优先 native DB；移走 DB 后，原始导出仍可重新读取。这个入口基于公开客户端源码，不承诺该 GraphQL 接口对外稳定，也不让 creditia 发请求或接触账户凭据。公开 REST transcript 下载的实际载荷仍另待样本核对，不按扩展名猜成上述协议。

Antigravity 编辑器默认读取 `~/.gemini/antigravity/conversation_summaries.db`；`CREDITIA_ANTIGRAVITY_IDE_DIR` 可指定其所在目录。DSH 默认读取 `$DSH_HOME/sessions`，通常为 `~/.dsh/sessions`；`CREDITIA_DSH_DIR` 可指定持久化根目录或 sessions 目录。DSH 只选每个目录最高 canonical `session.vN.jsonl[.zstd]` generation；支持 `.zst` 压缩后缀，不扫描临时 / 迁移中间文件，不遇到未来版本就退回旧 generation。

## 官方依据与验证范围

- [Cascade hooks 与 transcript](https://docs.devin.ai/desktop/cascade/hooks)、[Devin CLI hook 公共字段](https://docs.devin.ai/cli/extensibility/hooks/overview)、[PostToolUse](https://docs.devin.ai/cli/extensibility/hooks/lifecycle-hooks)、[Devin v3 OpenAPI](https://docs.devin.ai/v3-openapi.yaml)。
- [Jules Activities](https://jules.google/docs/api/reference/activities/)、[Jules Sources](https://jules.google/docs/api/reference/sources/)。
- [OpenHands 0.60 File observations](https://github.com/OpenHands/OpenHands/blob/0.60.0/openhands/events/observation/files.py)、[事件序列化](https://github.com/OpenHands/OpenHands/blob/0.60.0/openhands/events/serialization/event.py)、[TokenUsage model](https://github.com/OpenHands/OpenHands/blob/0.60.0/openhands/llm/metrics.py)。
- [SWE-agent 运行产物序列化](https://github.com/SWE-agent/SWE-agent/blob/main/sweagent/agent/agents.py)、[官方实际 .traj 测试样本](https://github.com/SWE-agent/SWE-agent/blob/main/tests/test_data/trajectories/gpt4__swe-agent-test-repo__default_from_url__t-0.00__p-0.95__c-3.00__install-1/6e44b9__sweagenttestrepo-1c2844.traj)、[mini-SWE 输出说明](https://mini-swe-agent.com/latest/usage/output_files/)、[mini toolcall/结果序列化](https://github.com/SWE-agent/mini-swe-agent/blob/main/src/minisweagent/models/utils/actions_toolcall.py)。
- Warp 原生依据固定到客户端 commit `f90f90a97f16e7b64ea5f3c1648ece15f9d3caf0`：[SQLite 任务存储](https://github.com/warpdotdev/warp/blob/f90f90a97f16e7b64ea5f3c1648ece15f9d3caf0/app/src/persistence/agent.rs)、[持久化模型](https://github.com/warpdotdev/warp/blob/f90f90a97f16e7b64ea5f3c1648ece15f9d3caf0/crates/persistence/src/model.rs)、[平台目录](https://github.com/warpdotdev/warp/blob/f90f90a97f16e7b64ea5f3c1648ece15f9d3caf0/crates/warp_core/src/paths.rs)；协议固定到 `30d54b5`：[Task / 成功结果 / ModelUsed / 时间](https://github.com/warpdotdev/warp-proto-apis/blob/30d54b5/apis/multi_agent/v1/task.proto)、[ConversationData](https://github.com/warpdotdev/warp-proto-apis/blob/30d54b5/apis/multi_agent/v1/conversation_data.proto)。读取器自写 protobuf 字段解码，没有复制客户端实现或生成代码。
- Warp 云会话的 [GraphQL 查询响应](https://github.com/warpdotdev/warp/blob/f90f90a97f16e7b64ea5f3c1648ece15f9d3caf0/crates/graphql/src/api/queries/list_ai_conversations.rs)、[AIConversation 字段](https://github.com/warpdotdev/warp/blob/f90f90a97f16e7b64ea5f3c1648ece15f9d3caf0/crates/graphql/src/api/ai.rs)、[客户端 base64 → protobuf 解码](https://github.com/warpdotdev/warp/blob/f90f90a97f16e7b64ea5f3c1648ece15f9d3caf0/app/src/server/server_api/ai.rs)。Devin 另外核对 [v1](https://docs.devin.ai/v1-openapi.yaml)、[v2](https://docs.devin.ai/v2-openapi.yaml) 和 [Outposts](https://docs.devin.ai/cloud/outposts/reference)，没有把自行定义的 structured_output、消息文本或节点工作目录当作成功编辑契约。
- [Warp Oz OpenAPI](https://docs.warp.dev/openapi.json)、[Warp CLI 导出与会话](https://docs.warp.dev/agents/cli/agent-conversations/)、[v0 官方 SDK 完整 OpenAPI（当前 2.0.0）](https://github.com/vercel/v0-sdk/blob/main/packages/v0-sdk/openapi.json)、[v0 v2 Get Message](https://v0.app/docs/api/v2/reference/messages/get-message)、[List Messages](https://v0.app/docs/api/v2/reference/messages/list-messages)。
- [Replit checkpoints](https://docs.replit.com/replitai/checkpoints-and-rollbacks)、[Replit 官方公开协议](https://github.com/replit/protocol/blob/f2874f167ad39d47d8e2ef92ab9da53a201a0a09/api.proto)；Bolt 固定核对官方 commit `eda10b121221b30825a4c16eec5da1fd3eb1eb99` 的 [ActionRunner](https://github.com/stackblitz/bolt.new/blob/eda10b121221b30825a4c16eec5da1fd3eb1eb99/app/lib/runtime/action-runner.ts)、[IndexedDB 持久化](https://github.com/stackblitz/bolt.new/blob/eda10b121221b30825a4c16eec5da1fd3eb1eb99/app/lib/persistence/db.ts)、[ChatHistoryItem](https://github.com/stackblitz/bolt.new/blob/eda10b121221b30825a4c16eec5da1fd3eb1eb99/app/lib/persistence/useChatHistory.ts)。这些是公开版契约，不推断未公开的当前托管产品相同。
- [Lovable 完整公开 OpenAPI](https://api.lovable.dev/v1/openapi.yaml)、[API 说明](https://docs.lovable.dev/integrations/lovable-api)、[audit export 完整字段](https://docs.lovable.dev/features/data-exports)、[audit logs](https://docs.lovable.dev/features/audit-logs)。
- [Antigravity CLI / 编辑器承载关系](https://www.antigravity.google/docs/cli/overview/)。编辑器数据库 schema 来自本机只读检查，不宣称是官方公开稳定格式。[DSH 源码](https://github.com/deepseek-ai/deepseek-harness) 的 Session codecs、tool-role、tool-fs、home-paths 与 bundle/base 配置在本机 checkout 核查；未改动 DSH。

自动测试验证成功 / 失败 / 未配对调用、上下文扣除、仓库路径、模型与 metadata-only 边界、各 DSH 版本结果结构、zstd 和 generation 选择。测试不能替代所有工具当前版本、所有平台、账户权限和真实运行的验收；没有相应账户的云端入口未声称完成现场验收。

Warp 的协议与 SQLite 集成测试覆盖原生 task BLOB、ConversationData、GraphQL finalTaskList、成功 / 失败 / 未配对 / 撤销 / 人工修改 / 全文覆盖排除、返回正文核对、request 模型歧义、真实 Timestamp 字段和路径不能越出项目；测试数据是依照官方字段自建的最小样本，不含外来代码。此机没有已安装 Warp 的原生会话样本，因此该来源目前完成源码契约与集成验证，尚未声称真实账户现场验收。

本机真实只读验证另完成：37 份 DSH 原生压缩会话全部可读，按真实仓库历史路径匹配产生 4,038 条行哈希证据，27 个会话有模型标识，无读取警告。Antigravity 完整原生入口读到 5 个会话、221 条行哈希，全部有逐生成模型，摘要合并不重复计数；TRAE CN 本机 state.vscdb 仅有已确认遥测键，不能据此断言所有用户没有其它历史。未把私人会话正文复制到测试或文档，也未改动对应项目。SQLite 读取在受限 sandbox 中可能因 sidecar 权限失败，当前实现报告错误，不用忽略 WAL 的陈旧快照伪装读取成功。
