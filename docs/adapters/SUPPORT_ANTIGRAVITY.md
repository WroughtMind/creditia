# Antigravity 完整原生历史

核查于 2026-10-17，依据本机官方 Antigravity 2.23.0 的实际数据库及应用内协议描述。读取器在 `crates/adapters/src/antigravity_ide.rs`，通过 `editor_history` 进入 CLI 与 App 共用入口。

## 数据与成功判据

- 默认 `~/.gemini/antigravity/conversations/*.db`；`CREDITIA_ANTIGRAVITY_IDE_DIR` 可以指定 Antigravity 根、conversations 目录或单个数据库。
- 只读 `trajectory_meta`、`trajectory_metadata_blob`、`steps`、`gen_metadata`。项目来自真实 `workspace_uris`；数据库与 WAL 的纳秒指纹共同控制重新解析。
- `step_format=0` 的 `step_payload` 是 `gemini_coder.Step` protobuf，未知格式报错，不猜正文。
- 完成的 `CortexStepGeneric` 通过 `GenericStepResult.payload` 中明确的 `type.googleapis.com/gemini_coder.Step` 取得实际执行子步骤。外、内步骤都必须 `DONE=3`，无错误；取消、失败、明确拒绝的 code action 排除。
- 只取实际 `CortexStepCodeAction.action_result.edit` 中的目标 URI 与统一差分。插入、删除、未改动分别是协议枚举 1、2、3；扣除重复旧行，未改动上下文、请求参数和展示 Markdown 不算编辑。
- 模型来自 `CortexStepGeneratorMetadata.step_indices` 关联的 `ChatModelMetadata.response_model_full/response_model`，或步骤自己的 model info；同一步生成模型有歧义就留空，不用最后选择的模型补历史。
- 时间来自步骤的完成/创建时间。完整原生会话优先，`conversation_summaries.db` 只补没有原生会话的摘要，不重复计算。

## 协议依据

核查了官方安装包内 `Resources/bin/language_server` 的内嵌 FileDescriptorProto，未启动或修改应用。该二进制 SHA-256：`c1dd0c9940f712adf2185e756470752d5f5afee15441e32879e4a0f127e18273`。

协议文件为 `third_party/gemini_coder/proto/trajectory.proto`、`third_party/jetski/cortex_pb/cortex.proto`、`third_party/jetski/codeium_common_pb/codeium_common.proto` 与 `third_party/jetski/diff_action_pb/diff_action.proto`。实现只保留读取所需字段编号及执行状态，不复制私人会话正文。此为已核实的原生版本契约，未宣称所有历史 `.pb` 文件或未来版本兼容。

## 验证

构造记录覆盖内外状态、失败/取消/拒绝、实际插入与上下文、逐生成模型、未知格式和损坏 protobuf。

本机只读运行完整读取入口：5 个原生会话，按实际 Git 历史路径匹配产生 221 条行哈希，221 条均读到逐生成模型，0 警告。摘要补充开启后仍为 5 个会话。计数只证明读取链路，不是准确率；未复制或输出私人代码、提示词、路径或认证资料，未修改原始数据库。
