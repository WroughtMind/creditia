# 剩余 6 个 Agent：原生历史续查与实现

后续更新：[4 项续查](SUPPORT_AGENT4.md) 已实现 Eve 条件原生读取，当前目录为 72 / 3 / 3；灵码真实历史验收仍未完成。下文保留本轮当时的实现和验证记录。

核查日期：2026-10-24。承接 [8 项核查](SUPPORT_AGENT8.md)，本轮落实 **Rovo Dev、灵码两个原生读取器**，均接入 CLI / App 共用扫描、发现、诊断和持久化流程，并通过真实 CLI 的重建样本测试。78 项目录现为 **71 个历史读取入口、3 个执行宿主、4 个尚未闭合的原生编辑格式**。读取入口包含有限编辑、会话、导入和条件读取，不表示 71 个产品全部功能或所有版本都已验收。

## 已实现：Rovo Dev

读取 `~/.rovodev/sessions/**/session_context.json`，同时检查所选项目的 `.rovodev/sessions`。可用 `CREDITIA_ROVODEV_DIR` 指向原生历史目录；设置后仅使用指定位置。

- 用原生 `workspace_path` 确认项目；配对 `message_history` 中相同 `tool_call_id` 和 `tool_name` 的调用与回执。
- `create_file` 仅接受 Nautilus 写入成功的精确回执。按官方实现移除 `!U!SHALL!NOT!PARSE!` 正文前缀；覆盖已有文件还要求 `overwrite: true`。
- `find_and_replace_code` 读取工具**写入后返回的实际 unified diff**。工具会修正缩进和 HTML 实体，不能直接把调用参数中的 replacement 当作落盘正文。
- 模型取每条响应的 `model_name`。排除失败、未配对调用、shell 推测和项目外路径。
- 原生 fork 会复制工具调用；按调用 ID 去重。官方 `/prune` 会删除工具结果，因此缺失或裁剪不撤销已确认的旧贡献；明确失败的同一调用才撤销。损坏历史不覆盖既有证据。

证据链：

1. [官方 CLI 插件 202610.16.1b1](https://acli.atlassian.com/plugins/rovodev/linux/amd64/202610.16.1b1/rovodev.zip)：静态提取 PyInstaller 包内未被保护的 `nautilus.tools.create_file`、`find_and_replace_code` 与文件工具模块，核对真实写文件路径和成功字符串。外层受保护不等于整个工具实现不可核查。
2. [公开原生 session_context 样本](https://github.com/ahmedalbanna/ai-document-qa/blob/1bf1bf6643fed7b34246a83f99f065cc7d4e8544/.rovodev/sessions/wd2c0476-f044-4308-8a0f-e2c6b1b3f8db/session_context.json)：交叉确认磁盘字段、请求/响应、调用参数、工具回执和逐响应模型。另核查 battlefield-of-mind 与 dotfiles-zsh-refactor 的公开原生会话，未将其用户正文带入项目测试。
3. [官方会话管理说明](https://support.atlassian.com/rovo/docs/manage-sessions-in-rovo-dev-cli/)：确认 `/prune` 删除工具结果的语义，用于持久化合并规则。

实现：[原生读取器](crates/adapters/src/rovodev.rs)。未在本机安装并登录 Rovo Dev 运行真实模型任务。

## 已实现：灵码 2.6.10

上轮定位到的 gob + Snappy 编码和聊天 AES 确实存在，但不是本轮采用的全文快照存储。本轮继续追踪官方安装包的调用链，确认：

- `GetDatabasePath` 返回 `GetCosyHomePath()/cache/db/local.db`，不是先前推测的 chat.db。
- `chat_working_space_file` 保存版本、状态、文件路径、快照 key、extra 与时间；`LocalFileWorkingSpaceFileProvider` 在 `workingSpace/<item_id>__<文件名>` 保存 UTF-8 明文全文。
- `acceptWorkingSpaceFile` 先读取全文、写目标文件，成功后才更新状态为 `ACCEPTED`。初始化的人类文件基线同样是 ACCEPTED，必须结合 version 和 local_id 排除。
- `updateWorkingSpaceFileDiffInfo` 保存 `extra.diffInfo.sourceMd5` 与 `targetMd5`。读取器要求前后快照分别匹配原生摘要，只从两者差分提取新增行。
- EDIT 模式或 `LINGMA_TAG::CONTENT_USE_WORKING_SPACE_LOCAL` 引用可变本地行，当前不作为独立历史快照读取。预览 APPLIED、缺少基线、摘要不匹配、缺失正文、项目外路径均不新增贡献；明确拒绝撤销对应记录。
- 不用目标文件的当前正文代替丢失快照。不从默认配置猜测模型，当前保留模型未知。

默认检查 `~/.lingma`、`COSY_HOME` 与平台缓存目录中的 `.lingma`；覆盖位置为 `CREDITIA_LINGMA_DIR`。数据库只读打开并在读取事务内查询；正文缓存只读。

证据来自 [Alibaba Cloud 官方 Open VSX 2.6.10 包](https://open-vsx.org/api/Alibaba-Cloud/tongyi-lingma/2.6.10/file/Alibaba-Cloud.tongyi-lingma-2.6.10.vsix) 内 Linux Go 服务与扩展：从 Go 符号、反射结构标签和反汇编核对表字段、plaintext provider、接受动作及 MD5 版本关系。关键函数地址：`GetDatabasePath` 0x14ed740、`GenerateKey` 0x159b440、provider `Get` 0x159b5c0、`acceptWorkingSpaceFile` 0x15c6860、`updateWorkingSpaceFileDiffInfo` 0x15caca0、`getFullContentWithEmptyFile` 0x15cbc00。这些地址仅对应上述安装包。

实现：[原生读取器](crates/adapters/src/lingma.rs)。**本轮没有可用的真实用户 local.db，验证使用按官方存储合同重建的数据库和快照**，不能称为真实灵码账号验收。

## 其余四项逐个续查

| Agent | 本轮追加核查 | 当前未实现的具体部分 |
|---|---|---|
| CodeMaker | [官方 Go CLI 482346a](https://github.com/codemakerai/codemaker-cli/tree/482346a34b55991f55c44065b066c0ab5d7e0d17)、[官方扩展 1.59.0](https://marketplace.visualstudio.com/items?itemName=codemakerai.codemakerai)。继续读完整 webview 消息、复制和反馈逻辑，以及 extension 的 assistantCodeCompletion → API → workspace.fs.writeFile 路径 | 所检查的写入链没有持久编辑 journal；webview 未保存 localStorage / IndexedDB / VS Code state 历史，反馈 ID 不含可回放的文件正文与路径。CLI logger 写 stderr。这个结论只覆盖所查版本与路径，不宣称所有用户都没有数据 |
| Eve | [官方源码 3c03b20](https://github.com/vercel/eve/tree/3c03b204d0fabb8fc155a93a40ba34d2c51f1628)、其固定依赖 world-local 5.1.2 / core 5.2.3 / serde 5.0.0。继续追 `.eve/.workflow-data/steps/<runId>`、durable-session-store、state-delta、GitHub channel state 和 sandbox provider | 已找到 GitHub `owner/repo/checkoutPath`，修正此前笼统的“没有仓库绑定”描述。但这些状态在 Workflow 检查点中，工具记录在会话历史或 OTLP 中；磁盘正文有 Uint8Array/base64、devl 引用图、可选压缩/加密以及增量状态。尚未取得能交叉验证整个关联链的原生存储样本，也未实现可靠重放，仍不把 appRoot 或沙箱 `/app` 当用户仓库 |
| inference.sh | [官方 belt CLI 1.26.14](https://dist.inference.sh/cli/v1.26.14/inferencesh-cli-v1.26.14-linux-amd64.tar.gz)、[官方 CLI setup 文档](https://inference.sh/docs/extend/cli-setup)、[官方安装仓库 9736524](https://github.com/belt-sh/cli/tree/9736524264825acd558521646d20bc33873ad992)。静态核查 Go 结构标签、hook、历史与宿主会话 | `~/.belt/history` 是 CLI 命令历史；review 读取 Claude/Codex/Kimi/Grok 等执行宿主的 transcript。代理控制、files_edited 统计和云端 app 输出均不是独立文件写入回执。已有宿主读取器继续负责其代码贡献，不重复挂到 inference.sh |
| MCPJam | [官方源码 c6c376e](https://github.com/MCPJam/inspector/tree/c6c376ef07d5edf68d5f26775952def18b98cebe)。补查 CLI project-link、sessions raw traces 和 server harness-session-state 的持久 sidecar | `.mcpjam/project.json` 确实能把本地目录关联到云 project ID；sidecar 保存 resumeState、computerId、harnessSessionId 等。但这两者没有闭合到一次远端编辑的 Git checkout、目标路径和具体 MCP 文件工具语义。原生 Claude/Codex/Cursor harness 不重复计数；任意 MCP server 尚无独立读取器 |

Eve 的存储编码已进一步定位，仍不能把“知道有这些字段”标成已支持。四项保持 `unconfirmed`，没有新增假想 JSON 格式、以 API 成功代替落盘回执，或用配置凭据冒充历史。

## 安装包复核

下载包只作静态读取，未运行安装脚本或启动其中的 agent。

| 原件 | SHA256 |
|---|---|
| Rovo Dev 202610.16.1b1 zip | `44194bc350289b1a1b3c8daabb6bc2e55b91d2ba251f33a62621e631a1538429` |
| 灵码 2.6.10 VSIX | `74a4cc84734de7a6de58d214b4ccd866e06956fd55a0fefc3c839c03b619a114` |
| CodeMaker 1.59.0 VSIX | `0a052a4503d025f0fb5e2b9653ca8889b8c4c8e1ed99d4948ba32e3c14bb8a51` |
| Eve 3c03b20 source archive | `601e9c71cd1cb666a2034ba0ac138f36a549cab6438317da10cf28937bc126f3` |
| belt CLI 1.26.14 Linux amd64 tar.gz | `3c5dcfbec1ccbac913235ab2e6486cab678ad38795800a7444ffa37694ac6793` |
| @workflow/world-local 5.1.2 | `301c15c5668f7b86d7f8e08ae72ca229b7d155b4d0f7ea80f20fa64ab0b654e4` |
| @workflow/core 5.2.3 | `106f667da354d829dd735f2ca4783a09760ab1e1b1a3a2a3c691ef73a9b11953` |
| @workflow/serde 5.0.0 | `1155016dcaf61c69458b4a1418ab03a186dc9e6391b6774b6c43ae2b5faf7d0c` |

## Hero 与验证

Hero 新增 **Kode、灵码、Rovo Dev**，共 **74 个真实品牌标志**。Kode 来自 [官方 README 固定版本](https://github.com/shareAI-lab/Kode-CLI/blob/400bbf0449707846465895ef64cfbbd18bea4763/README.md) 所引用的官方横幅：只截取左上角产品标志，保留原件，构建时输出小尺寸 alpha mask，显示随页面单色前景变化。灵码使用官方包内单色 sidebar 标志，Rovo Dev 使用 Atlascode 官方 sidebar 标志。[素材来源](site/assets/agents/SOURCES.md)。

[端到端测试](crates/cli/tests/native6/mod.rs) 调用实际 CLI，检查 SQLite 中的持久证据和最终 `credits.json`。覆盖成功、失败、未配对、模型区分、fork 去重、裁剪与损坏保留、明确拒绝撤销、项目边界、人工初始基线、MD5 不匹配、可变引用、只读数据库和缓存。它验证 Creditia 的读取与归因行为，不替代厂商真实账号运行验收。

本轮检查结果：Rust 工作区 **329 通过、1 个原有重型实仓库测试忽略**；App Rust **8 通过**；目录渲染 **2 通过**；工作区与 App 严格 Clippy、CLI bundle、网站完整构建通过。浏览器使用隔离 Chromium，确认 1440px 下深浅色与 Kode 悬停，390px 中文页面保留 74 个标志且无横向溢出。Rovo 的 fill 为 none、stroke 为页面前景；Kode 加载约 6 KB 的构建蒙版。未覆盖 Safari、Firefox 或真实触屏。
