# 剩余 8 个 Agent：原生格式复查与实现

后续更新：Rovo Dev、灵码已在 [6 项续查](SUPPORT_AGENT6.md) 中落地，Kode 官方标志也已找到；下文保留本轮当时的证据边界。

核查日期：2026-10-23，延续 [18 项调查](SUPPORT_AGENT18.md)。本轮新增 **Firebender、Moxby 两个原生读取器**，接入 CLI / App 共用的扫描、诊断和持久化流程。78 项目录现为 **69 个历史读取入口、3 个执行宿主、6 个尚未获得完整可验证编辑格式**。reader 包含有限编辑、会话、导入和条件读取，不能解释为全面支持 69 个产品。

发行包只做静态提取和代码分析，没有运行安装包中的程序。下面的集成记录依照官方实现重建；未获得这些产品的真实账户历史，不宣称逐账号、逐平台验收。

## 已实现

| 来源 | 原生格式与读取位置 | 可归因内容 | 边界 |
|---|---|---|---|
| Firebender 1.0.12 | 每个选择项目的 `.idea/firebender/chat_history.db`；schema_info 版本 1 / 2，chat_sessions 与 subagent_conversations；`CREDITIA_FIREBENDER_DIR` 可指定目录或原库 | MessageAdapter 的 assistant tool_calls 与 tool_call_id 配对。仅原生 `Write` 的完整成功回执，核对实际绝对目标、原始写入正文；子 agent 独立会话，fork 共享调用去重，按毫秒更新顺序选择最新回执 | 没有逐消息模型和时间：模型未知，时间是会话快照。排除待执行、失败、IDE 自动缩短全限定名、Edit/API 替换及 shell。worktreePath 优先于数据库所在项目。完整删除/失败快照撤回旧证据，损坏库不伪造撤销 |
| Moxby Desktop 3.2.0 | Tauri app-data 下 `com.moxby.agent/moxby_chats.db`（开发版 com.moxby.agent.dev）；`MOXBY_STATE_DIR` / `CREDITIA_MOXBY_DIR`；读取 chat_threads、chat_turns、tool_calls、tool_results | 内置 `file_edit` 的执行后结构化结果：status=applied、replaced=1、绝对 path、oldText/newText；用当前 call.result_event_id 精确连接结果，要求 latest_event_id 同一回执、done、未截断及 file.edit.result v1；模型来自对应 turn | 不从输入建议或可变聊天配置猜实际文本/模型。排除陈旧回执、失败、待执行、mod、Claude/Codex CLI 镜像、shell、多处替换。绝对目标由项目路径规则匹配；不从聊天 workspace_id 猜 Git 根目录，因此不提供自动项目发现 |

实现：[Firebender](crates/adapters/src/firebender.rs)、[Moxby](crates/adapters/src/moxby.rs)、[共享注册表](crates/adapters/src/registry.rs)。数据库均以只读事务访问。

## 逐项查证

| Agent | 本轮具体检查 | 结论 |
|---|---|---|
| Firebender | [JetBrains 1.0.12](https://plugins.jetbrains.com/files/25224/1109733/Firebender-1.0.12.zip) 的 ChatHistoryDatabaseService、MessageAdapter、SerializedName、ClaudeCodeWriteTool 与 FileEditExecutor。静态还原 ZKM 字符串，解出 SQL、版本迁移、字段名和写后回执 | 已落地。Write 在文档写入后返回含真实路径的固定回执；IDE 自动缩短会另加提醒，这类文本不复用输入。Edit 委托 API 重写，回执不含实际新文本，未硬套成精确替换 |
| Moxby | Chrome 3.2.0 包找到 [官方 Bridge 更新清单](https://github.com/ChainAI-Org/moxby-bridge-releases/blob/main/update.json)，进一步取得 [Desktop 3.2.0 macOS arm64](https://github.com/ChainAI-Org/moxby-agent-releases/releases/download/v3.2.0/Moxby_arm64.app.tar.gz)。原生 Rust 符号、SQL、dispatch_file_edit、tool_result_payload、derive_tool_result 与 resolve_state_dir 相互核对 | 已落地。执行器 apply_exact_edit → fs::write 成功后生成结构化回执；Pi 封套的 output 变成 structuredOutput，derive_tool_result 将它存入 result_json。这里的证据是桌面文件操作，不是浏览器 recorder |
| Lingma | [Open VSX 2.6.10](https://open-vsx.org/api/Alibaba-Cloud/tongyi-lingma/2.6.10/file/Alibaba-Cloud.tongyi-lingma-2.6.10.vsix) 内 Linux Go 服务；从 .gopclntab 恢复函数地址，反汇编 Serialize/Deserialize、EncodeData/DecodeData、saveSingleWorkingSpaceFileContent、GetFullContent 与数据库加解密 | 已确认内容是 **Go gob 序列化 + Snappy**，聊天数据库另有 AES/Base64 加密；此前“codec 未知”已收窄。仍缺完整 KV key/type/版本引用链、密钥来源与 ACCEPTED 状态对应的正文校验，没有真实库可交叉核对，暂未落地 |
| Rovo Dev | [官方插件 202610.16.1b1](https://acli.atlassian.com/plugins/rovodev/linux/amd64/202610.16.1b1/rovodev.zip)、[Atlascode](https://github.com/atlassian/atlascode/tree/7ef48ac216e6ecb8da95ef5af92a94e5fea23f2d)，追加检查 [官方 rovo-dev-action](https://github.com/atlassian-labs/rovo-dev-action/blob/3677dd130d7be0ebc359b06be774832209cced09/action.yml) | 会话位置确认，session_context 实现仍受 PyArmor 保护；官方 action 仅调用 CLI，未导出可验证的编辑历史。本机目录无可用样本，公开 API stream 类型不当作磁盘格式。未落地 |
| Eve | 更新至 [vercel/eve 09f8de5](https://github.com/vercel/eve/tree/09f8de5904abd523d8a4fd3c2fa276852c34b1da)：本地 trace-span-processor、invoke-tool-span、apply_patch 和 workspace-root | 确认 OTLP v4 的参数/结果、完成状态、文件操作和沙箱根目录校验。`.eve/traces` 所在 appRoot 不是实际文件 cwd；默认 /app 等沙箱路径缺少到用户 Git 项目的可信绑定。未把可选遥测直接等同本地编辑历史 |
| CodeMaker | [Go CLI 482346a](https://github.com/codemakerai/codemaker-cli/blob/482346a34b55991f55c44065b066c0ab5d7e0d17/cli/cli.go) 的 generateCode/fixSyntax、logger，与 [Marketplace 1.59.0](https://marketplace.visualstudio.com/items?itemName=codemakerai.codemakerai) 的实际写文件路径 | CLI 调 API 后 os.WriteFile，logger 写 stderr；所检查路径没有持久编辑日志。扩展写入路径同样未连接可回放历史。配置中的凭据不是历史，不将一次 API 响应冒充过去的编辑。未落地 |
| MCPJam | [c6c376e 源码](https://github.com/MCPJam/inspector/tree/c6c376ef07d5edf68d5f26775952def18b98cebe)：sessions 导出、shared/harness-session、HarnessBuiltinToolsSection、codex-appserver-builtin-tools | workdir/machine 是 transient session 数据，不能证明云端记录与本地项目绑定；内置 harness 编辑实际来自 Claude/Codex/Cursor，已有宿主读取器不重复记功。任意 MCP server 仍无统一成功文件操作语义。未新增独立原生 reader |
| inference.sh | [官方 SDK guide 与 skills 0cbf91c](https://github.com/inference-sh/skills/tree/0cbf91c26ada28702c79962406286f0c14292150)，追加检查 onToolCall、executeTool、submitToolResult | 客户端工具由应用自定义；云端 codeExecution / app task 成功及 artifacts 不能单独证明用户 Git 文件被写入。没有稳定原生文件编辑合同及仓库映射，未落地 |

### 可复核的安装包与静态定位

| 安装包 | SHA256 |
|---|---|
| Firebender 1.0.12 zip | `136d479c50222a3ccc95e67e1c6ac57edbb174f9c45cc06c508e2a5580560fb3` |
| Moxby Desktop 3.2.0 arm64 tar.gz | `287e00933c7436de74e263f3f6ffa303efaa908bf132abf543233eb6128edd53` |
| Moxby Chrome 3.2.0 CRX | `e4bfd58df79ea8ba301760dd83d54d7083daf235bada3a02e5016d0bc5240a27` |
| Lingma 2.6.10 VSIX | `74a4cc84734de7a6de58d214b4ccd866e06956fd55a0fefc3c839c03b619a114` |
| Rovo Dev 202610.16.1b1 zip | `44194bc350289b1a1b3c8daabb6bc2e55b91d2ba251f33a62621e631a1538429` |
| CodeMaker 1.59.0 VSIX | `0a052a4503d025f0fb5e2b9653ca8889b8c4c8e1ed99d4948ba32e3c14bb8a51` |

Moxby 此构建的 Mach-O 函数地址：dispatch_file_edit `0x1015e8060`、tool_result_payload `0x10158160c`、derive_tool_result `0x1025b4658`、resolve_state_dir `0x1014f4efc`。Lingma 此构建的 Go EncodeData `0xe9d700`、DecodeData `0xe9d7a0`、Serialize `0xe9d2c0`、doEncrypt `0x14f5220`、doDecrypt `0x14f52c0`。地址只适用于上述哈希的包，不作为运行时硬编码。

## 验证与 Hero

[native8 集成测试](crates/cli/tests/native8/mod.rs) 从隔离 Git 仓库中的原生数据库运行真实 CLI，检查持久 evidence 和最终 credits.json：Firebender 主会话及子 agent 的 16 行，Moxby 的 8 行；覆盖 fork、毫秒更新顺序、最终失败/删除、错误 JSON 保留旧证据，以及 Moxby 陈旧/截断/跨会话/外部工具回执的排除。每次扫描前后逐字节比较源数据库，确认读取没有改写历史。模型未知时保持未知。

当前工作区验证：Rust workspace 324 项通过、1 项忽略；App 8 项、渲染器目录 2 项通过；workspace / App 的 Clippy 严格检查、CLI 打包和站点构建通过。实际 CLI 的 agents 命令能列出两个新入口及其限制。

Hero 原图标区从 55 增至 **71 个真实品牌标志**。新增 AutoHand、Bob、CodeArts、Cortex、fx、iFlow、Kimchi、Mux、Tabnine、Terramind、ZCode、Zenflow、Neovate、AdaL、Firebender、Moxby；所有标志均使用页面的单色前景，含浅色、深色和悬停状态。官方文件保留，显示时处理轮廓、透明度和文字裁切。[全部素材与处理来源](site/assets/agents/SOURCES.md)。Kode 已有读取器，但尚未找到独立官方图标，未拿社区截图或自造字母冒充官方 logo。品牌数与目录入口数统计口径不同。

隔离 Chromium 中完成中英文 × 深浅色 × 1440 / 901 / 900 / 390 / 320px 共 20 组布局检查：71 个标志均保留，无横向溢出和末行提示框裁切。修复短桌面窗口的导航重叠与底部裁切；实际 901 ↔ 900 ↔ 899px 缩放、正常桌面滚动及语言/主题切换通过，返回顶部后标题和动效恢复。尚未覆盖 Safari、Firefox 与真实触屏。

本轮未发布，也未修改任何真实 agent 日志。真实厂商账户和 Windows / Linux 实机仍未逐一验收。
