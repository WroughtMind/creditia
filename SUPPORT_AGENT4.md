# 剩余 4 个 Agent：Eve 原生读取与逐项续查

后续进展：真实灵码库验收及 Eve 参数包装/导出修复见 [SUPPORT_AGENT3.md](SUPPORT_AGENT3.md)。本页保留上一轮结论。

核查日期：2026-10-24。承接 [上一轮](SUPPORT_AGENT6.md)，本轮新增 **Eve 条件原生读取器**，接入 CLI / App 共用扫描、发现、诊断和持久化。目录现为 **72 个历史读取入口、3 个执行宿主、3 个仍待确认的编辑格式**。72 包括有限编辑、会话、导入和条件读取，不表示 72 个产品已完成真实账号或全部版本验收。

灵码仍未获得真实用户历史库。现有源码合同和重建数据库测试继续有效，真实历史验收保持未完成。

## 已实现：Eve

默认读取 `~/.eve`，分析时也检查所选项目的 `.eve`。`CREDITIA_EVE_DIR` 可指定包含 `traces` 与 `.workflow-data` 的原生 `.eve` 目录，设置后仅使用该位置。

读取器关联两种原生记录：

- `traces/v1/<traceId>/segments/*.otlp.json`：Eve v4 工具轨迹中的调用参数、执行结果、会话和调用 ID。
- `.workflow-data/steps/**/*.json`：已完成的 `turnStep` 检查点，保存 session v2 和序列化上下文。支持 Uint8Array/base64 内的 `devl` 引用图及 `gzip` / `zstd` 压缩，只解码所需 JSON 字段，不恢复流、函数、类或凭据。

从检查点的 `eve.channel` 及其原生状态增量还原 GitHub `owner/repo/repositoryId/checkoutPath`。必须与本地仓库的**完整远程地址**匹配；不同组织下的同名仓库不互认。一个会话出现冲突绑定时跳过。不会把 agent 的 appRoot 或沙箱 `/app` 直接当成用户仓库。

当前落地的编辑范围：

| 原生工具 | 计入条件 | 排除范围 |
|---|---|---|
| 框架 `write_file` | v4 `agent.tool.is_framework: true`，已结束且没有失败状态，实际结果 `existed: false`，结果路径在原生 checkout 内，调用含完整正文 | 已有文件覆盖缺少可靠前文；仅有调用没有结果；自定义同名工具；项目外路径 |
| `eve-code` 的 `apply_patch` | 检查点记录 disk bundle；原生 v54 清单和 v6 compile metadata 匹配；清单 SHA256 正确，源码图 hash 与该回合记录的 runtime revision 相符；绑定指向 `eve-code` 扩展的实际 patch 模块；参数显式给出 checkout 内的 `root`；结果 `files` 列明成功写入路径 | 当前重新编译的清单冒充旧版本；任意同名/MCP 工具；未明确 patch root；提案路径不在成功结果内；绝对路径或目录越界；纯删除不产生新增行 |

只读取已成功执行工具给出的新增正文；模型保持未知。shell 命令、加密检查点、无 GitHub 绑定、development/bundled 逻辑选择器未解析、未知清单版本和缺失正文均不推测。补丁需要保留当时的磁盘编译产物；新建文件回执不依赖该清单。

按完整仓库、session ID、tool call ID 合并，重复复制轨迹不增加贡献。明确失败撤销对应调用；文件缺失、加密或读取损坏不抹除以前已验证的证据。读取过程不改写原始日志。

实现：[读取器](crates/adapters/src/eve.rs)、[引用图与增量解码](crates/adapters/src/eve_codec.rs)、[CLI 端到端测试](crates/cli/tests/native4/mod.rs)。

### Eve 来源与样本性质

固定核查版本：[vercel/eve 3c03b204](https://github.com/vercel/eve/tree/3c03b204d0fabb8fc155a93a40ba34d2c51f1628)。关键路径包括：

- [OTLP 本地写入](https://github.com/vercel/eve/blob/3c03b204d0fabb8fc155a93a40ba34d2c51f1628/packages/eve/src/tracing/local/trace-span-processor.ts) 与 [工具 instrumentation](https://github.com/vercel/eve/blob/3c03b204d0fabb8fc155a93a40ba34d2c51f1628/packages/eve/src/tracing/eve/agent-tool-instrumentation.ts)。
- [持久上下文编解码](https://github.com/vercel/eve/blob/3c03b204d0fabb8fc155a93a40ba34d2c51f1628/packages/eve/src/runtime/sessions/runtime-context-keys.ts)、[GitHub channel state](https://github.com/vercel/eve/blob/3c03b204d0fabb8fc155a93a40ba34d2c51f1628/packages/eve/src/public/channels/github/state.ts)、[ValueDelta](https://github.com/vercel/eve/blob/3c03b204d0fabb8fc155a93a40ba34d2c51f1628/packages/eve/src/shared/value-delta.ts)。
- [实际 write_file](https://github.com/vercel/eve/blob/3c03b204d0fabb8fc155a93a40ba34d2c51f1628/packages/eve/src/execution/sandbox/write-file.ts)、[实际补丁执行器](https://github.com/vercel/eve/blob/3c03b204d0fabb8fc155a93a40ba34d2c51f1628/packages/eve-code/extension/lib/patch.ts)、[补丁工具结果](https://github.com/vercel/eve/blob/3c03b204d0fabb8fc155a93a40ba34d2c51f1628/packages/eve-code/extension/tools/apply_patch.ts)。
- [编译清单](https://github.com/vercel/eve/blob/3c03b204d0fabb8fc155a93a40ba34d2c51f1628/packages/eve/src/compiler/manifest.ts)、[编译 metadata](https://github.com/vercel/eve/blob/3c03b204d0fabb8fc155a93a40ba34d2c51f1628/packages/eve/src/runtime/loaders/compile-metadata.ts)、[运行版本计算](https://github.com/vercel/eve/blob/3c03b204d0fabb8fc155a93a40ba34d2c51f1628/packages/eve/src/runtime/cache-key.ts)。
- Workflow 磁盘与序列化合同核查自固定依赖 `@workflow/world-local 5.1.2`、`@workflow/core 5.2.3`、`@workflow/serde 5.0.0`；下载原件摘要保留在 [上轮记录](SUPPORT_AGENT6.md#安装包复核)。

本轮使用官方 `devalue 6.0.3`、`@opentelemetry/otlp-transformer 0.214.0` 和上述源码中的 `diffValue` 生成受控记录，并实际调用官方 `applyPatchToSandbox` 写入隔离临时文件，核验其 304 字节正文及返回路径。随后经过真实 Creditia CLI → SQLite 持久证据 → 最终档案检查。

这是**原生执行器、编码器和存储合同的受控验证**。会话、GitHub 上下文、编译清单与输入是测试构造，不是厂商真实账号历史，也没有运行完整 Eve 托管 agent 会话。[样本与可复现生成器](crates/adapters/tests/fixtures/eve/README.md)。

## 其余三项逐个续查

| Agent | 本轮新证据 | 尚缺什么，为什么没有增加 reader |
|---|---|---|
| CodeMaker | 补读 [官方 JetBrains 实现 f309ce3](https://github.com/codemakerai/codemaker-jetbrains/tree/f309ce37207e7a40566d0722e361ef48883ab13a)。`CodeMakerService.assistantCodeCompletion → writeFile` 直接改文档并提交；`AcceptInlineCompletionAction` 直接插入并清空 inlay；设置落入 `SdkSettingsPlugin.xml`，聊天通过 webview DOM 追加 | 在这些路径未找到带工具身份、路径、正文和接受状态的持久编辑记录。IDE 通用 Local History 不能凭时间猜为 CodeMaker。结论只限所查版本/路径，不宣称所有用户都没有历史；上轮 VS Code / Go CLI 的核查仍保留 |
| inference.sh | 补查 [原生 REST 搜索与 session-end 合同](https://inference.sh/docs/api/rest/search)、[agents / harness 合同](https://inference.sh/docs/api/rest/agents)、[CLI 路径](https://inference.sh/docs/extend/cli-setup)。`/session/end` 仅保存 session / agent / 计数，文档明确不保存路径、提示词或回复；旧 cwd / summary 参数忽略。harness 工具由外部客户端执行，平台记录不等于另一次执行 | `~/.belt/history`、计数和远端 agent 轨迹不能还原具体文件写入。任意 app / MCP 工具没有统一编辑语义。Claude/Codex 等宿主记录由现有读取器处理，不重复挂给 inference.sh |
| MCPJam | 继续读 [sessions CLI](https://github.com/MCPJam/inspector/blob/c6c376ef07d5edf68d5f26775952def18b98cebe/cli/src/commands/sessions.ts)、[聊天会话路由](https://github.com/MCPJam/inspector/blob/c6c376ef07d5edf68d5f26775952def18b98cebe/mcpjam-inspector/server/routes/v1/chat-sessions.ts)、[原生 eval trace](https://github.com/MCPJam/inspector/blob/c6c376ef07d5edf68d5f26775952def18b98cebe/mcpjam-inspector/shared/eval-trace.ts)。核查 raw messages、toolOrigin、wireCorroborated、outputSource、computerId、browser context 与 personal-cloud/sandbox bash | 这些字段能区分轨迹证据与叙述，但还不能闭合到 Git checkout、目标文件正文和特定工具的写入结果。云 project ID / computer ID 不能代替 Git 项目身份；任意 bash/MCP 调用成功也不能推断代码差分。Claude/Codex/Cursor harness 仍归原执行宿主 |

新鲜度检查：Eve 源码截至 `eda61225e10bcc6a88314b69b89819343db07152`，与固定版本之间未见上述编码/持久格式改动；MCPJam 截至 `6ce034b1699b876c541eed02081d5e6c2875fca9`，三次增量提交未修改所查 session/trace/harness 路径。它们只是源码差异核查，不扩大固定版本的验收承诺。

## 灵码真实历史验收

本轮只读检查 `~/.lingma`、平台缓存与应用支持目录、VS Code 目录，以及项目和下载目录中的 `local.db` / 灵码文件；未找到可用的真实历史库，`COSY_HOME` 也未配置。用户确认没有另一台机器或自定义目录可提供历史。

公开仓库继续搜索 `local.db`、`chat_working_space_file`、`workingSpace` 与 `sourceMd5`：

- [ctx 的 Lingma fixture 说明](https://github.com/ctxrs/ctx/blob/56aaf35233ce7f8aa739c207525a3d67e6ddddbd/tests/fixtures/provider-history/lingma/README.md) 明确称样本是按 WayLog 合同建模，主要读取 `chat_record` 摘要，不是可供本次接受编辑验收的真实用户数据库。
- 其他找到的导出器源码描述聊天记录读取，没有随附可配对的真实 `chat_working_space_file` 数据和 `workingSpace` 前后快照。
- `~/.lingma/vscode/sharedClientCache/cache/db/local.db` 等子目录已在现有递归扫描范围内，无须为它增加虚构的新格式。

因此没有把摘要、人工生成数据库或当前项目正文当成真实原生验收样本。当前仍是：官方 2.6.10 包的写入/接受/MD5 合同已核查，读取器及重建样本测试通过，**真实历史库验收未完成**。

## 验证

新增解码单测与 CLI 端到端测试覆盖：完整仓库绑定、同名不同组织、压缩引用图、opaque 值不恢复、损坏/加密保留旧证据、实际写入、新建与覆盖区分、清单来源和版本匹配、明确 patch root、成功路径与提案分离、越界、重复片段、失败撤销和只读性。新建文件和补丁分别产生 8 行，重复读取仍为 16 行，明确失败后仅对应调用的 8 行被撤销。

本轮 Rust 工作区 **333 通过，1 个原有实仓库重型测试默认忽略**；App Rust **8 通过**；目录渲染 **2 通过**；工作区 / App 严格 Clippy 与 CLI bundle 通过。网站完整构建通过（含 App 网页版与中英双语产物）。CLI bundle 保留原有重复 `units` 属性提示。现有 Hero 保留 74 个真实品牌标志、随页面前景单色显示；本轮没有改变图标素材或布局。没有发布或替换已安装 App。
