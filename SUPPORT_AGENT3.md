# 剩余三项续查与真实历史验收

> 后续进展：Eve 本地源码沙箱、inference.sh Codex 驱动/原生导出已真实验收；MCPJam 已接入本地 Codex 独立目录。本文保留当轮证据，最新范围见 [SUPPORT_NATIVE_LIVE.md](SUPPORT_NATIVE_LIVE.md)。

核查日期：2026-10-24。承接 [上一轮](SUPPORT_AGENT4.md)。本轮修改了灵码和 Eve 的实际读取器，并用新找到的公开原始历史检查现有实现。目录仍为 **72 个历史入口、3 个宿主、3 个待确认编辑格式**；没有把新线索直接改成“已支持”。

| 对象 | 本轮结果 | 验收边界 |
|---|---|---|
| 灵码 | 修复原生枚举大小写及跨系统快照定位；真实 SQLite + 原始快照经过 CLI、持久库、最终档案 | 一份公开库中一条完整、摘要匹配的新建文件记录通过正向验收；不是所有记录、所有版本或真实 Windows App 验收 |
| Eve | 修复 Workflow `args` 包装和发布包步骤 ID；新增官方 `traces --json` 导出读取；补齐 durable action 失败状态 | 新找到的官方 CI 原始写入/读回回执通过正向解析验收；缺少配套仓库绑定，完整正向归因仍未完成 |
| CodeMaker / inference.sh / MCPJam | 继续读新找到的源码和原生导出链，逐一核对执行者、路径、正文、接受状态 | 本轮没有找到可闭合的新编辑合同；具体原因如下，不以命令成功或工具名猜测文件贡献 |

## 灵码：真实库推动的修复

公开样本来自 [autonomous-elasticsearch-evolution-agent 固定版本](https://github.com/vortsghost2025/autonomous-elasticsearch-evolution-agent/tree/d3badfd22c443db1b54e428f0d14751f7a5d35d8/sharedClientCache/cache)。其中 `db/local.db` 为 9,093,120 字节，`chat_working_space_file` 有 352 行，仓库包含 202 份 `workingSpace` 快照。本轮只查询工作区文件元数据和项目 URI；没有用聊天正文、账号配置或当前工作区文件补造快照。

实际记录与旧构造测试有差异：

- `content_type` 是 `FULL`，不是旧测试写的 `full`。旧读取器因此漏掉真实记录。
- `extra.CHAT_MODE` 是 `agent` / `edit`；可变本地引用的排除条件改为真实的 `edit`。
- `key` 是原 Windows 机器的绝对路径。读取时校验其原生 `workingSpace/<item_id>__<basename>` 结构，再从同一缓存目录的 `db/../workingSpace` 找不可变快照。不会读当前项目文件，也不依赖原机器仍然在线。
- 前后 MD5 仍逐字节校验。公开仓库中有的快照不匹配数据库摘要，包括换行可能经过 Git 处理的记录；不自动改换行、填正文或放宽摘要校验。

实现：[Lingma reader](crates/adapters/src/lingma.rs)。正常回归新增 Windows 原生 key、`FULL`、`edit`、错误 key 目录等用例，保留缺失/损坏不抹除旧证据、明确拒绝撤销、重复分析和只读验证。

正向验收使用原始条目 `846c1557-ab2b-4bd5-9dac-b96a66be3b98` 和其 `persistent-memory.js` 快照。它是 `ACCEPTED`、`agent`、版本 1、有 `local_id` 的记录；source MD5 为空文件，target MD5 与原始 214 行快照一致。数据库中的 Windows 路径、ID、时间、状态、摘要均未修改。

只重建用于比对的隔离 Git 仓库，提交该原始快照，运行真实 Creditia CLI 两次。结果为 **308 条内部行/锚点证据、1 个编辑 ID；最终档案 verified 119 行、declared 95 行**。308 不是代码行数；95 行也未算作独立逐行验证。两次结果一致，数据库与快照读取前后字节一致。样本的客户端构建号没有独立记录，不能仅凭本次通过就声称真实 2.6.10 客户端或全部历史已验收。

## Eve：原生包装、导出和真实样本

真实 Workflow 记录的 `input` 解码后是 `{args, closureVars, thisVal}`，发布包步骤 ID 形如 `step//eve@0.23.0//turnStep`。旧读取器直接取输入数组第 0 项，且仅接受源码路径形式的步骤 ID。本轮改为读取 `args[0]`，同时识别 Eve 发布包 ID。官方 `@workflow/core 5.2.3` 的 `runtime/step-executor.js` 也明确使用 `hydratedInput.args`。旧测试中的裸参数数组已纠正，没有保留这个构造出来的兼容入口。

新增读取官方 [serializeTraceForJson](https://github.com/vercel/eve/blob/3c03b204d0fabb8fc155a93a40ba34d2c51f1628/packages/eve/src/cli/commands/trace.ts) 输出：属性字典、`startTimeNs` / `endTimeNs`、`statusCode`。把 `eve traces --json` 的原始输出保存在 `.eve/traces/*.json`，与同一 `.eve` 下的 `.workflow-data` 配对；`CREDITIA_EVE_DIR` 仍可指定该目录。无需改写导出内容或自行填写仓库映射。

GitHub 完整远程绑定、session v2、框架工具身份和补丁清单要求仍然有效。一个导出内缺绑定的其他会话会产生诊断，但不阻断已能确认的会话。OTLP 与 JSON 导出同时存在不会重复记功；明确失败撤销相应调用。模型仍保持未知。

本轮找到并实际读取的材料：

| 原始材料 | 已验证 | 不能作为正向归因的原因 |
|---|---|---|
| [headless-harness-bench / golden-trace.json](https://github.com/aaronjmars/headless-harness-bench/blob/bf098250e9602d63ebfd3456f07d6bb9a61612b5/t2/eve/golden-trace.json) | Eve 0.60.1 / trace v4、27 个 span，真实 read/write 回执；原始 CLI 导出能被读取，并报告缺绑定 | `channel:eve`，没有 GitHub 检查点；写入为已有文件覆盖，旧版本也没有 `agent.tool.is_framework`。不能把 `/workspace` 当成本机仓库。该 trace 与 `golden-docker.out` 是不同会话，未拼接成一个成功运行 |
| [nextjs-eve-test 检查点](https://github.com/daniel-t7/nextjs-eve-test/blob/0886f5c9d87e1e292ed8805d98f36ce3afd80e4e/.workflow-data/steps/wrun_01KXEQKD8BHBTJH2HKQ0F4AND5-step_01KXEQKD8BAFKHND2MBMNCB4S6.json) | 原始 zstd / devalue 输入输出可解码；真实 `args` 与发布包 ID 已覆盖 | Eve 0.23.0、session v1、HTTP channel，没有所需仓库绑定；明确报告旧会话版本不支持 |
| [Eve-Agentic 检查点](https://github.com/sixscripts-ai/Eve-Agentic/blob/3ef31107609413ed12ee0f10e66ef183da5e397c/.workflow-data/steps/wrun_01KWSA53JZFHMH9SG485J1SN7X-step_01KWSA53JZQJWE48EMTYJA0A99.json) | 原始 zstd / devalue 输入输出可解码；不会恢复 auth 等不相关运行时分支 | Eve 0.19.0、session v1、HTTP channel，同样不满足正向条件 |

两个旧版目录的原始 `.workflow-data` 位于项目根；它们用于验证编码和排除行为，没有把其布局或 v1 会话混同于当前 `.eve/.workflow-data` / v2 合同。公开代码搜索的 53 个 `turnStep` 结果均来自这两个仓库；没有发现包含所需配套信息的其他公开检查点。

实现：[Eve reader](crates/adapters/src/eve.rs)、[引用图选择性解码](crates/adapters/src/eve_codec.rs)。[受控生成器](crates/adapters/tests/fixtures/eve/generate.mjs)已重新生成正确参数包装，继续调用固定官方补丁执行器和 OTLP/devalue 编码器；正向新建文件/补丁、导出、失败撤销、重复和仓库边界仍通过 CLI 测试。它们与上述真实负向样本分开报告。

### 继续核查：官方 CI 的真实写入与读回

取得 [Eve 官方运行 40056903264](https://github.com/vercel/eve/actions/runs/40056903264) 的 `eval-artifacts-agent-tools-sandbox-openai-sol` 制品，编号 `12545831396`，源码 `db0ffaf1d8997ee7f396e67ff974945c08c9e6b0`。下载 ZIP 的 SHA256 `3ccee524a117052e57e4fece90fc52ef1189e07c84777d387347b330ff461306` 与 GitHub 元数据一致。整个 workflow 失败，但其中 `sandbox/file-tools` 明确为 `passed`，四项断言均通过，不能把 workflow 总状态当作该条文件操作结果。

原始会话 `wrun_01M5NY0R0ZEY7NE4YWAC0DARDB` 由 Eve 0.83.0 实际运行：框架 `write_file` 在 `/workspace/notes/handoff.txt` 新建单行 `sandbox-file-tools-ok-Q2H`，后续框架 `read_file` 返回完全一致正文。两段原始 OTLP 共 4,867 字节，未修改内容，仅重命名，保存在 [official-ci](crates/adapters/tests/fixtures/eve/official-ci/README.md)。测试固定校验各文件 SHA256，再通过生产解析函数核对新建文件、路径、正文和读回结果；无需联网或私人数据。

这份真实记录还推动了代码修复：durable action 的状态位于 `agent.action.outcome`，旧读取器只检查 `eve.tool.outcome`，会漏掉未伴随 OTLP error code 的拒绝/失败。现在任一原生失败状态都会撤销该调用；[官方 direct invoke 路径](https://github.com/vercel/eve/blob/0a9f8579c822e6d056b3a7e32e2e09425e619f23/packages/eve/src/tracing/eve/invoke-tool-span.ts)仍实际写 `eve.tool.outcome`，所以两者都检查。新增 CLI 场景覆盖 durable failed、durable denied 和 direct denied。

此制品没有 GitHub channel 检查点，`workflow-events.json` 为空；eval 的 runtimeIdentity 只有 agent ID/name/version。CI 所在源码仓库也不能代替被编辑沙箱的仓库身份。新增测试同时确认完整读取器不生成项目贡献。因此，**真实文件回执正向解析已通过，真实仓库归因仍未通过**；没有构造检查点补齐这条链，也没有放宽绑定或新增支持计数。

## 剩余三个：新证据与具体阻断

### CodeMaker

前几轮查的是 `codemakerai` 的 CLI / VS Code / JetBrains；本轮追溯 [Skills 原始收录 PR #849](https://github.com/vercel-labs/skills/pull/849)，它只提供 `.codemaker/skills`，不能据此认定所有同名产品都是 `codemakerai`。

找到 [Y3Helper 公开 CodeMaker 移植代码 58c7dd7](https://github.com/y3-editor/y3-helper/tree/58c7dd7f4269c8c45435db79f6d25a4db0aa3d28)。其 [同步映射](https://github.com/y3-editor/y3-helper/blob/58c7dd7f4269c8c45435db79f6d25a4db0aa3d28/.codemaker/sync/config.json)明确区分上游与定制部分，因此没有把移植版的文件格式直接冒充所有 CodeMaker 发行版：

- [transcript.ts](https://github.com/y3-editor/y3-helper/blob/58c7dd7f4269c8c45435db79f6d25a4db0aa3d28/resources/webview_source_code/src/utils/transcript.ts)会在压缩时写 `~/.codemaker/persisted/sessions/<工作区编码>/<session>/<时间>.jsonl`；白名单保留调用和 `tool_result`，不直接保留接受状态。分支可复制旧 transcript。
- [本地历史 API](https://github.com/y3-editor/y3-helper/blob/58c7dd7f4269c8c45435db79f6d25a4db0aa3d28/resources/codemaker/api-server/routes/history.mjs)保存 `chat_index.json` 和 `sessions/<id>.json`。这是移植版 API Server 的本地实现。
- [edit_file](https://github.com/y3-editor/y3-helper/blob/58c7dd7f4269c8c45435db79f6d25a4db0aa3d28/src/codemaker/utils/editFile/index.ts)返回 before/final 内容时尚未落盘；真正写入另经 `ACCEPT_EDIT`。[Claude 风格 write/edit](https://github.com/y3-editor/y3-helper/blob/58c7dd7f4269c8c45435db79f6d25a4db0aa3d28/src/codemaker/utils/editFile/claudeEdit.ts)即使 `autoApply=false`，也会返回 `File created/updated successfully`。只检查成功文本会误把预览算成贡献。
- [chatApply.ts](https://github.com/y3-editor/y3-helper/blob/58c7dd7f4269c8c45435db79f6d25a4db0aa3d28/resources/webview_source_code/src/store/chatApply.ts)在实际成功通知后才更新 `accepted`；该 store 的 localStorage 部分仅持久化 `enableNewApply`。本轮未找到能和上述 transcript 稳定配对的实际接受记录或真实完整会话。

因此撤回“CodeMaker 没有持久历史”的笼统说法，改为：已发现具体产品线的原生历史，仍缺确切产品对应及已执行写入的持久证据。本轮不把 `isError:false` 或成功字符串当成接受凭据。

继续追到完整 session 的持久链：[CodeChat 的 ACCEPT_EDIT_RESULT](https://github.com/y3-editor/y3-helper/blob/58c7dd7f4269c8c45435db79f6d25a4db0aa3d28/resources/webview_source_code/src/routes/CodeChat/CodeChat.tsx)会调用 `onUserSubmit`，而 [chat store](https://github.com/y3-editor/y3-helper/blob/58c7dd7f4269c8c45435db79f6d25a4db0aa3d28/resources/webview_source_code/src/store/chat.ts)将 `message.response[toolCallId] = true` 连同 session 保存。但同一 store 的 `handleAutoExecute` 会在没有等待写入回执时生成同样的 `true`；[subagent handler](https://github.com/y3-editor/y3-helper/blob/58c7dd7f4269c8c45435db79f6d25a4db0aa3d28/resources/webview_source_code/src/modules/subagent/core/toolCallHandler.ts)接受失败后仍可能保存原始预览结果。`checkPointFiles` 只保留该用户消息下每个路径的首次 before 内容，不能稳定对应每一次 tool call。大正文落在 `~/.codemaker/persisted/<session>/tool-results/<toolID>.txt`，也不补充接受回执。这些都没有成为新的写入凭据。

### inference.sh

本轮新增检查其官方 [agentprotocol a336c97](https://github.com/inference-sh/agentprotocol/tree/a336c97eb0c942be66bac8dc3fbbd95ba4caa9c0) 的 transcript / harness 模块。[codec 注册表](https://github.com/inference-sh/agentprotocol/blob/a336c97eb0c942be66bac8dc3fbbd95ba4caa9c0/transcript/all/all.go)与 SQLite 模块读取 Claude、Codex、Cursor、Gemini、Pi 等宿主自己的存储；这些执行记录应留给实际宿主，不能再计为 inference.sh 的一份贡献。

另查 [SDK 的 MCP 回执测试](https://github.com/inference-sh/sdk-js/blob/57a64b9389334ca71784647999335b72addbba6b/src/api/mcp-servers.test.ts)及 [REST agent history / tool results](https://inference.sh/docs/api/rest/agents)。回执支持 `resultType`、任意正文和附件，但 `write_file` 示例是 mock；它不提供独立 coding agent 的已执行编辑格式或 Git checkout 绑定。平台云端自定义工具、上传文件和返回附件仍不等于本机项目文件修改。

结合前轮官方 belt 安装包的静态核查，没有获得可新增的独立文件历史读取合同。

继续核对最新 [JavaScript SDK 类型 3476973](https://github.com/inference-sh/sdk-js/blob/34769738ea5eb9e1e2ca6ad1dbe06044e8b62b7d/src/types.ts)与 [Python SDK 类型 3ae8677](https://github.com/inference-sh/sdk-py/blob/3ae8677e4cf7f0bc173d5fa72094ab22a3d3cb88/src/inferencesh/types.py)：`harness_session_id`、`work_dir` 指向远端宿主自己的会话。官方 [Coding Agents 文档](https://inference.sh/docs/extend/coding-agents)确认 review 从宿主 transcript 提取知识，session-end 为汇总计数；`~/.belt/sessions` 存在本身不证明独立文件历史。没有把同一次宿主写入再登记成 inference.sh 的贡献。

### MCPJam

本轮深入 [logger-view 的原生导出](https://github.com/MCPJam/inspector/blob/c6c376ef07d5edf68d5f26775952def18b98cebe/mcpjam-inspector/client/src/components/logger-view.tsx)：`json-rpc-logs-*.json` 保存 timestamp、source、serverId、serverName、direction、method、payload。并追到 [traffic-log-store](https://github.com/MCPJam/inspector/blob/c6c376ef07d5edf68d5f26775952def18b98cebe/mcpjam-inspector/client/src/stores/traffic-log-store.ts)及 [rpc-log-bus](https://github.com/MCPJam/inspector/blob/c6c376ef07d5edf68d5f26775952def18b98cebe/mcpjam-inspector/server/services/rpc-log-bus.ts)。

它记录 MCP 通信，包含工具测试、界面调用和 agent 调用；导出字段不提供足够的 agent 会话/调用归属或本地仓库身份。配对 `tools/call` 与成功 response 只能证明某 MCP 服务答复了调用，不能证明是 agent 写入所选 Git 项目。大输出还可能被截断。任意同名 filesystem 工具、云端路径、人工测试调用均不能直接算作 MCPJam 的代码贡献。已有 harness 的原始历史继续从执行宿主读取。

继续检查官方 `6ce034b1699b876c541eed02081d5e6c2875fca9` 的 [trace parity fixture](https://github.com/MCPJam/inspector/blob/6ce034b1699b876c541eed02081d5e6c2875fca9/mcpjam-inspector/shared/__tests__/fixtures/trace-span-parity-fixtures.json)。其中 `write_file` / `wireCorroborated` 是跨仓库校验器的构造样本，缺少真实文件正文及 checkout 身份，不能称作真实运行。原生 `harness-session-state` 的 resumeState、computerId、runtimeFingerprint 用于恢复执行环境，不是写入证明；built-in tools 与 Codex appserver 路径也没有提供一套可独立计为 MCPJam 的原生文件合同。

## 可重复的真实样本验收

原始公共数据只保存在外部临时目录，不随项目复制个人聊天或运行时上下文。测试要求固定 SHA256，缺样本时明确忽略，不以构造数据替代。它们已在本轮显式执行，常规离线测试不联网下载。

| 本地相对路径 | SHA256 |
|---|---|
| `lingma/cache/db/local.db` | `aa5abdf2d05a3ca7eaf54134d3d4f91fa9388a3ba01f0729f272a1bb48c00f1a` |
| `lingma/cache/workingSpace/846c1557-ab2b-4bd5-9dac-b96a66be3b98__persistent-memory.js` | `53b0da6da39c5ed978348606f105ebc4ab468006e697ec373b5b1e604d01abce` |
| `eve/checkpoint0.json`（上表 nextjs-eve-test 原件） | `be31282ab307535f65554f11f7c5ba9a95e0ddb12f4ae8195e90611756135ab8` |
| `eve/checkpoint1.json`（上表 Eve-Agentic 原件） | `ba8330ddac39306d2b50342ce5242f8a33b3d706f5f7a3c949393e5055d41b07` |
| `eve/golden-trace.json` | `f887c86c2c7893e096df05b3af3591af108c3e55d3593f7a523cea7b133526ec` |

从上述固定版本的 GitHub Raw 链接下载原件，保留灵码 `db` / `workingSpace` 相邻目录；Eve 两份检查点仅重命名为表中名称，内容不变：

```sh
CREDITIA_TEST_LINGMA_CACHE=/absolute/path/lingma/cache cargo test -p creditia-cli --test e2e lingma_public_native_history_acceptance -- --ignored
CREDITIA_TEST_EVE_HISTORY_DIR=/absolute/path/eve cargo test -p creditia-adapters public_native_history_decodes -- --ignored
```

第一项是原始灵码记录的正向 CLI 验收；第二项是 Eve 的真实解码及无绑定/旧版本排除验收，不是 Eve 完整正向验收。

## 验证

上一轮 Rust 工作区 **334 通过、3 个默认忽略**：一个原有实仓库重型测试，以及上面的两个外部真实历史测试。两个真实历史测试均已另行显式执行并通过。App Rust **8 通过**，目录渲染 **2 通过**；工作区和 App 的严格 Clippy、CLI bundle 通过。CLI bundle 保留原有重复 `units` 属性提示。本次针对新增修改另跑官方 CI 原始回执测试、Eve 的 3 项 CLI 回归、adapters/CLI 严格 Clippy 和下载/打包模块的 8 项测试。CLI 的 `agents --json` 仍为 78 项，支持层级没有因本轮研究而虚增。

网站缺失模块已修复：当前 `site/build.ts` 已接入发布通道判断，却遗漏了模块及其依赖。从本地提交 `0bb633aa70fab5cfa7154d0964f6b32af0026f64` 恢复 `site/downloads.mjs`、`packaging/release.mjs`、对应测试与 cask 模板，补回 Pages 的 release 触发和 GitHub 只读令牌。未整体合入该提交。下载判断只展示当前版本已发布的安装包及校验文件；网络错误仍让构建失败，未伪装成“尚未发布”。

完整构建已通过：双语署名页各 26 页、嵌入 App、作品墙、用法示例、WebP 和字体子集均生成，未设置跳过开关。本机网络代理通过 Node 的 `--use-env-proxy` 使用；CI 仍执行正常的 `node site/build.ts _site`。

浏览器检查发现旧构建入口仅输出 24 个 Hero 标志，而 74 个所需素材仍在。新增独立图标构建入口，接回 40 个共享品牌和 34 个官网素材，并新增官方 Eve 图标，共 75 个单色品牌，原始官方素材不改；Kode/灵码/Rovo Dev、轮廓几何、栅格透明蒙版及明暗主题均已检查。手机端修复标题与代码图重叠，并允许图标条横向滚动。中文和英文安装区均按实际发布状态展示，当前未发布的 App/Homebrew 没有假下载链接。初次语言切换有 7 次本地 Python 预览连接重置，逐项复查均 HTTP 200，文件本身完整；这不计作构建失败或线上部署验收。

没有发布、替换已安装 App，或修改任何 agent 的原始历史。
