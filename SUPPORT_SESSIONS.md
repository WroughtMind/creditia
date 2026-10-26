# Pi、OMP、Command Code、Muse、T3、Vibe、MiniMax 原生读取证据

检查日期：2026-10-17。这里区分“源码契约已核实并有解析测试”和“安装产品实际操作验收”。本次未安装、启动、调用模型，也未发布任何产品。日志读取只读；没有打印用户提示词、代码或凭据。

| 来源 | 已核实入口与实现 | 成功与失败边界 | 模型归因边界 |
|---|---|---|---|
| Pi | `~/.pi/agent/sessions/**.jsonl`，支持 `PI_CODING_AGENT_DIR`、`CREDITIA_PI_DIR` | `toolCall.id` 配 `toolResult.toolCallId`，必须 `isError:false`；edit 必须有 oldText/newText，按差分取新增；普通 write 回执不保留 before 或新建证明，排除整份覆盖；bash 成功命令为候选 | 每条 assistant 自带 model/provider；测试模型切换；分叉中早于新 header 的继承消息不当作新工作 |
| Oh My Pi | `~/.omp/agent/sessions`、命名 profile 与 XDG 目录；`CREDITIA_OMP_DIR` | 配对成功结果；hashline、apply_patch 用实际 `details.oldText/newText` 差分或逐文件结果，快照被裁剪时用结果 diff；只有实际 op=create 可将无 before 的 newText 作为新文件。普通 write 会 format-on-save 且不保留 before，故不按请求整文归因 | 每条 assistant 的真实模型；共享 `PI_CODING_AGENT_DIR` 时按当前格式识别，避免同一份日志同时计为 Pi 与 OMP |
| Command Code | `~/.commandcode/projects/**.jsonl` v3，`CREDITIA_COMMAND_CODE_DIR` | 当前官方 npm 1.83.0 序列化 tool_result 丢弃 isError；edit_file 匹配明确成功回执并取 old/new 差分；write_file 只有明确 File created successfully 回执允许记录整份新建内容，普通 updated 覆盖不保留 before 故排除；失败/拒绝/中断不生成行证据 | assistant entry 的 model；model_change 仅作缺失时上下文；有独立成功、拒绝、模型切换测试 |
| Mistral Vibe | `$VIBE_HOME/logs/session/**/meta.json + messages.jsonl`，旧 `session_*.json`，`CREDITIA_MISTRAL_VIBE_DIR` | `PersistedToolResult.output` 只在成功 response 时生成；失败与取消不算；用实际 write/edit output，不依赖请求或 hook 改写显示文本 | 历史消息没有逐调用模型/时间，metadata.config 被覆盖；行证据模型留空，会话仅显示最终配置模型，不倒灌历史 |
| MiniMax Code | `~/.minimax[-profile]/v2/sqlite/runtime-state.sqlite` 和 canonical `v2/sessions/**/messages.jsonl`，兼容旧 Pi history SQL rows/JSON | canonical 优先，避免同历史迁移重复读取；工具调用配 `isError:false`；MiniMax edit 的 old_string/new_string 按官方定义转换后只取新增；普通 write 缺 before 或新建证明，排除整份覆盖 | 每条 Pi assistant 真实 model/provider，成功/失败/模型切换测试 |
| T3 Code | `~/.t3/userdata/statev2.sqlite`，也查旧 `state.sqlite`、dev 目录与 `T3CODE_HOME`；`CREDITIA_T3_DIR` | v2 completed file_change 取 oldStr/newStr/diffStr，命令必须 exitCode=0；旧 v1 当前只读会话，不补猜编辑 | driver 决定执行 agent；每个 run.modelSelection 决定该 run 模型；nativeThreadRef.nativeId 与既有原生来源去重；每次扫描重建宿主组以响应后来出现的原生日志 |
| Muse Code | `$XDG_DATA_HOME/muse/sessions/YYYY/MM/DD/<id>/session.jsonl` 及 child logs，官方 trajectory v1 导出；`CREDITIA_MUSE_DIR` | 官方 approval requested 保留 raw_args，edit_file 必须有对应 intent 与 completed task 才按 old_string/new_string 差分取新增；write_file 的完成状态没有 before 或新建证明，故排除整份覆盖；旧 tool_result 必须 outcome=success，bash 另需 exit_code=0；只有路径/字节数回执无法还原文件内容 | 旧日志工具参数可能只在加密模型响应中；未核实内部模型字段不猜。原生模型留空。T3 中的 Muse 明文编辑/模型由 T3 读取器覆盖 |

Pi 本机发现 20 份历史 JSONL，仅查看头部与字段形状；没有以这些用户历史执行归因验收。其它六家本机未发现可用于该工作完整验收的真实记录。测试是按已核实公开契约构造的针对性用例，不能视为真实运行、跨平台或用户整体验收。

## 当前上游依据

- [Pi session-manager](https://github.com/earendil-works/pi/blob/698a2d2/packages/coding-agent/src/core/session-manager.ts)、[消息类型](https://github.com/earendil-works/pi/blob/698a2d2/packages/ai/src/types.ts)，检查快照 `698a2d2`。
- [OMP session entries](https://github.com/can1357/oh-my-pi/blob/470b7ef/packages/coding-agent/src/session/session-entries.ts)、[实际编辑结果聚合](https://github.com/can1357/oh-my-pi/blob/470b7ef/packages/coding-agent/src/edit/index.ts)、[hashline 结果测试](https://github.com/can1357/oh-my-pi/blob/470b7ef/packages/coding-agent/test/edit-tool-details.test.ts)、[目录解析](https://github.com/can1357/oh-my-pi/blob/470b7ef/packages/utils/src/dirs.ts)。
- [Command Code 会话文档](https://commandcode.ai/docs/sessions) 与公开 npm `command-code@1.83.0` 的 `dist/cli.mjs`；直接读分发包，未安装/运行。
- [Vibe 消息类型](https://github.com/mistralai/mistral-vibe/blob/4ae5c59/vibe/core/types.py)、[成功结果持久化](https://github.com/mistralai/mistral-vibe/blob/4ae5c59/vibe/core/agent_loop_hooks.py)、[会话保存](https://github.com/mistralai/mistral-vibe/blob/4ae5c59/vibe/core/session/session_logger.py)。
- [MiniMax canonical history](https://github.com/MiniMax-AI/minimax-code/blob/9a93560/packages/local-runtime-v2/src/infra/file/canonical-history-jsonl.ts)、[SQL session schema](https://github.com/MiniMax-AI/minimax-code/blob/9a93560/packages/local-runtime-v2/src/infra/db/schema/sessions.ts)、[写入工具参数](https://github.com/MiniMax-AI/minimax-code/blob/9a93560/packages/agent-tools/src/desktop/builtin-defs.ts)。
- [T3 v2 schema](https://github.com/pingdotgg/t3code/blob/cfafb4b/apps/server/src/persistence/Migrations/055_OrchestrationV2.ts)、[contract](https://github.com/pingdotgg/t3code/blob/cfafb4b/packages/contracts/src/orchestrationV2.ts)、[Muse 原始状态映射](https://github.com/pingdotgg/t3code/blob/cfafb4b/packages/provider-muse/src/server/itemPresentation.ts)。
- [Muse 官方审计说明](https://dev.meta.ai/docs/cookbook/audit-agent-sessions)、[官方实际 approval_slice](https://github.com/meta-models/meta-model-cookbook/blob/main/04_muse_code/01_append_only_audit_log/artifacts/approval_slice.json)、[官方 SDK](https://github.com/meta-models/muse-code-sdk/tree/bb863c9) 与 SDK 1.4.9 类型；旧工具结果字段另由 [ACP 已核实 0.2.1 契约](https://github.com/bex-co/muse-code-acp/blob/990afd5/src/muse-events.ts)佐证。不能把 SDK 的 MSP 视图字段直接当作原生 runtime.* 存储字段。

## 整份覆盖归因修正

成功落盘不等于旧文件的每一行都由 agent 新增。当前 Pi/OMP/MiniMax/Muse 的普通 write 回执不足以证明先前内容或创建新文件，因此保守排除整份 write；没有依赖不完整 read 或请求参数冒充 before。保留已确认成功的 edit old/new 差分、OMP 实际编辑 diff 与 op=create、Command Code 明确新建回执。缺失 oldText/old_string 不能自动解释为空文件。

针对性回归使用 `human_original_line` 与 `model_new_line`：成功整份覆盖不记录旧行，仅成功 edit 中新增的模型行命中；失败、pending、缺失 before、普通 updated 回执与真实新建回执分别验证。共享 STORE_VERSION=5 由主任务统一使旧缓存重算。
