# 剩余 18 个 Agent：安装包、源码与原生历史适配

后续进展：Firebender / Moxby 已新增原生读取器，当前 69 / 3 / 6 和 Hero 单色 71 标志见 [SUPPORT_AGENT8.md](SUPPORT_AGENT8.md)。下文保留上一轮核查时的状态。

核查日期：2026-10-23。本轮范围是 [上一轮 31 项记录](SUPPORT_AGENT31.md) 留下的 18 项；没有把新产品加入分母。

**新增 7 个原生历史读取器**：Posit Assistant、Tinycloud、Zencoder、Code Studio、Terramind（当前 Nucleus）、Zenflow、Sarvam Code。均已接入 CLI 与 App 共用的扫描注册表、诊断和持久化流程。目录现为 **67 个历史读取入口、3 个执行宿主、8 个仍缺可验证编辑格式**，共 78 项。67 包括有限编辑、仅会话、导入和条件读取，不能解释为 67 个产品全面适配。

本轮读取的是官方源码或公开发行包里的存储、执行器和回执实现。发行包只做静态提取，没有运行其中的可执行程序；没有安装登录 18 个产品或发起真实模型任务。测试记录依照这些固定版本构造，不能替代真实账户验收。没有修改任何真实 agent 历史。

## 已落地的 7 条路径

| Agent | 原生位置及覆盖入口 | 已实现的编辑证据 | 排除项与边界 |
|---|---|---|---|
| Posit Assistant | `~/.posit/assistant`、`~/.positai`，含 `settings.json` 的 `storage.path`；覆盖 `CREDITIA_POSIT_ASSISTANT_DIR`。完整 V2 JSON 导出可放 `<repo>/.creditia/imports/posit-assistant/` | V2 树或 V3 `conversation.json` + `lm-messages.jsonl`，沿活动分支配对 write/edit 回执；apply_patch 按 `affectedPaths` 逐操作确认。cwd 来自 `workspace.json.path`，模型取消息 metadata | 不计 inactive 分支、外部 engine 镜像、shell 建议和未成功操作。导出换 root ID 仍按 tool-call ID 去重；未知树版本、缺失引用报错并保留旧证据 |
| Tinycloud | `~/.tinycloud/projects/*/sessions/*.jsonl` 和旧 `sessions/*.jsonl`；`TINYCLOUD_HOME` / `CREDITIA_TINYCLOUD_DIR` | SessionStore 消息封套里的 assistant toolCall 与成功 toolResult，核对目标路径、实际字节回执和内容规范化；消息携带的模型 | 项目目录哈希不能充当 cwd，仅匹配绝对路径。排除失败、脱敏正文、未知 JS 替换展开；复制的同会话日志不重复计数 |
| Zencoder | `~/.zencoder/sessions/*.jsonl`；VS Code 系 workspaceStorage 内 `zencoder-chat/sessions/*.json`；`CREDITIA_ZENCODER_DIR` | 原生 CLI Write/Edit 执行后的 diff，配对工具结果与逐消息模型；IDE `cliToolCall` 的 SUCCESS output diff 补充缺失调用，模型未知 | 不用 codeHunk 预览或当前 chat.model。已配对的原生调用优先；header-only / pending 原生日志不遮掉其他 IDE 成功编辑；复制/fork 共享调用去重 |
| Code Studio | 系统应用配置目录 `Syncfusion Code Studio/User/workspaceStorage/*` 和 `~/.syncfusion-ai-server-code-studio/data/User`；`CREDITIA_CODESTUDIO_DIR` | `chatEditingSessions/<id>/state.json` v1/v2、contents 快照与 `chatSessions`。重放 create/textEdit 时间线，严格匹配 accepted 最终快照，再按 requestId 归属当次模型 | UTF-16 编辑范围、CRLF；不归因 rejected、notebook、rename 或无法重建的操作。accepted 证明编辑缓冲区已接受；上游保存异常可能被捕获，因此不宣称已验证磁盘保存。最终贡献仍须与 Git 实际内容匹配 |
| Terramind / Nucleus | Electron 用户目录 `nucleus-desktop/data/agents.db`（stable 沿用 package name，非显示名称 Nucleus）；`CREDITIA_TERRAMIND_DIR` | 只读事务连接 `sub_chats` 与 `harness_tool_executions`。仅 local + settled 的 `native:Write` / `native:Edit`，使用执行流水的 normalized_arguments 与成功输出；step_id 对齐历史模型 | 排除 cloud、provider 自定义工具、uncertain、replace-all、只有 UI 预览的消息。消息被裁剪而模型缺失时留空；不猜云端/工作树路径映射 |
| Zenflow | macOS `~/Library/Application Support/ai.forgoodai.zenflow/db.sqlite`；Windows `%APPDATA%/forgoodai/zenflow/data`；Linux `$XDG_DATA_HOME/zenflow`；`ZENFLOW_DATA_DIR` / `CREDITIA_ZENFLOW_DIR` | 读取 `execution_process_normalized_logs.logs`，重放 JSON Patch 到最终 entries，仅 successful file_edit 的写入文本或无行号差分；task/container 与 project 提供路径 | 排除 dropped、失败、pending 和行号渲染。共享 tool-call ID 去重；已由原生 Zencoder/Codex/Claude/Gemini 读取的执行会话优先，Zenflow 镜像不再重复记功。模型未知，不使用可变 profile |
| Sarvam Code | `~/.sarvam/sessions/**/*.jsonl`、`archived_sessions`；`SARVAM_HOME` / `CREDITIA_SARVAM_CODE_DIR` | 原生 session_meta、turn_context 和 event_msg；仅 `patch_apply_end.success=true` 的 add/update/move，按当轮模型和 cwd 归因 | 不根据“基于 Codex”猜格式：以 0.55.0 包内 rollout 文档和协议序列化定义为依据。不计 pending、失败和 shell 提案；最后回执覆盖同调用，fork 复制调用只算一次 |

读取器分别位于 `crates/adapters/src/{posit,tinycloud,zencoder,codestudio,terramind,zenflow,sarvam}.rs`。非默认目录的覆盖变量只改变 Creditia 读取位置，不会修改原产品配置。数据库以只读事务读取；完整且可读的失败/删除快照可以撤销旧贡献，缺文件、坏 JSON 或重建失败不能伪造撤销。

## 格式依据与可复查版本

| Agent | 官方发行来源 | 在包内查到的具体实现 |
|---|---|---|
| Posit Assistant | [官方安装脚本](https://assistant.posit.co/builds/install-tui.sh)、[pa Linux x64](https://assistant.posit.co/builds/tui/pa-linux-x64.gz) | Bun 包中的 `core/dist/conversation/conversation-tree.js`、`conversation-manager-disk.js`、`nodepaths`、`tools/{write,edit,apply-patch}` 和 `engines/engine-transcript-mirror.js`。V3 外置 LM 消息、V2 full export、createNewIds 与 engine 最后消息盖章均沿代码核对 |
| Tinycloud 0.3.25 | [官方分发清单](https://media.cloudglue.dev/tinycloud-dist/manifest.json)、[Linux 包](https://media.cloudglue.dev/tinycloud-dist/tinycloud-linux-x64-v0.3.25.tar.gz) | 打包 JS 中 SessionStore 的封套、项目/旧会话路径，write/edit 的成功详情与 TinyCloud/CloudGlue 文本规范化逻辑；不是只看 launcher |
| Zencoder 3.85.9010 | [官方发布的 Open VSX 扩展](https://open-vsx.org/api/ZencoderAI/zencoder/darwin-arm64/3.85.9010/file/ZencoderAI.zencoder-3.85.9010@darwin-arm64.vsix) | 扩展 FileSystemChatStore 与 bundled zen-cli SessionStorage、Write/Edit、modelDetails 序列化；对照 IDE input preview 与 output actual diff 的不同字段 |
| Code Studio 2.0.5 | [Syncfusion macOS 安装包](https://downloads.sfcodestudio.com/v2.0.5/mac-silicon/syncfusioncodestudio.dmg) | 内置 workbench 的 ChatEditingSessionStorage、timeline、accept/reject 状态与 ChatSessionStore。状态 1 为 Accepted，currentEpoch 指向下一 epoch。旧 agent-traces.db 的参数不是执行后文本，未拿来归因 |
| Nucleus 0.2.177 | [Terramind 官方 stable 元数据](https://terramind.com/a/nucleus/api/releases/stable)、[macOS 包](https://storage.googleapis.com/terramind-com.firebasestorage.app/public/releases/stable/v0.2.177/Nucleus-darwin-arm64.zip) | `out/main/index.js` 内 schema、数据库位置、native 工具注册、授权后 normalized_arguments、settled 持久化、Write/Edit 与 step metadata。发布 commit `7bf6e7f8e3a715fb6faba804dc3efa98e4c6ad08` |
| Zenflow 2.5.3 | [官方 Linux 包](https://download.zencoder.ai/zenflowapp/latest/linux-x64/Zenflow.deb) | Electron `app-dist/js/paths.js` 与打包 Rust 后端的 SQL/serde，前端 normalized entry 消费器的 file_edit/status/JSON Patch；三处相互核对。没有把整个 checkpoint git diff 归给当前模型 |
| Sarvam Code 0.55.0 | [官方安装脚本](https://code.sarvam.ai/install.sh)、[Linux 包](https://code.sarvam.ai/v0.55.0/sarvam-code-linux-amd64)、[官方 SHA256SUMS](https://code.sarvam.ai/v0.55.0/SHA256SUMS) | 包内 `self-knowledge/rollout-traces` 的原生日志说明与 `PatchApplyEndEvent`、`FileChange` 序列化字段。下载 SHA256 与官方校验文件一致；VS Code 扩展 0.0.14 只连接 CLI app-server，不另造 IDE 历史格式 |

以下 SHA256 固定本轮实际检查的字节，避免以后可变下载链接更新后仍套用同一结论：

| 安装包 | SHA256 |
|---|---|
| Posit pa-linux-x64.gz | `60ae07ccd47cf70c988552f91aecf3e1816034c6f59c7ff14283a4577e1180d6` |
| Tinycloud 0.3.25 | `31fae7ac86ab1441673a639696c1228abdf56ed49af7d9304d4729d4e50c413f` |
| Zencoder 3.85.9010 | `0b2da5bdb3d6a8c522e7a4eb8397035fdc021a5a309320b9c5547e6799ec07d3` |
| Code Studio 2.0.5 | `033537f10ad2cd2c3f648e7ce6f5926520cf2be6f498c0e0c114e1a4d76b70b4` |
| Nucleus 0.2.177 | `c697ed4b325b8ad0ef09697153bf07c707b775a4549c0caae599ec1f238f70b5` |
| Zenflow 2.5.3 | `79df5402af411bc18cd6386e26b05b40b71d5e8f8cd506e4975cb81540d45795` |
| Sarvam Code 0.55.0 | `bd89f898e0f956f7c80125bf09b562120bec29696ccecee94faabf4e001857a0` |

## 另外 11 项的逐项结果

三项应从实际执行器归因，不能为了凑独立 reader 再记一遍贡献：

| Agent | 本轮查证 | 处理 |
|---|---|---|
| Ona | [当前官方 Codex Agent 文档](https://ona.com/docs/ona/agents/codex.md)明确使用开源 Codex harness；旧 overview 已重定向至此页 | 目录归为 host。在云端或客户自管执行环境读取 Codex 原生日志；没有把本机没有的云端文件当成已获得的历史 |
| Loaf | [固定版本 SQL](https://github.com/levifig/loaf/blob/5905ca59c82a5e9fad673c6a63c0d276f92a4a63/docs/schema/0020_facts_and_journal_envelope.sql)有 observed_worktree 与 harness_session_id，facts/journal 本身没有实际文件文本及模型回执 | 保留实际宿主身份；工作日志不能再记成 Loaf 的独立编辑 |
| PromptScript | [固定版本源码](https://github.com/mrwogu/promptscript/tree/ff8e75e13b37323def46a4ec9d77474d593d1f95)的遥测是命令、功能、耗时及 outcome | 指令编译器/宿主入口；从执行生成指令的 agent 读取贡献 |

八项仍未获得足以实现可靠逐行归因的完整格式。本轮均继续深入了源码或发行包，以下是具体停点，不是把产品全部判为“无历史”：

| Agent | 本轮实际查到的官方证据 | 仍缺什么，为什么没有硬套解析器 |
|---|---|---|
| CodeMaker | [Marketplace 扩展](https://marketplace.visualstudio.com/items?itemName=codemakerai.codemakerai) 1.59.0 的 `dist/extension.js`，API/process 结果通过 workspace.fs.writeFile 落盘；另查官方 Go CLI | 检查到的写入路径未连接可回放的本地历史存储。远端 assistant 会话存在，但未得到包含成功文件正文、项目路径和模型的持久格式/导出；不能将当前 API 返回值冒充过去历史 |
| Eve | [vercel/eve 固定版本](https://github.com/vercel/eve/tree/25c9fff80b79504c73127f169eb9fa7182bc349a)：`packages/eve-code/extension/tools/apply_patch.ts` 与 `packages/eve/src/tracing/local/trace-span-processor.ts`。可选 `.eve/traces/v1/<traceId>/segments/*.otlp.json` 保存工具参数/结果 | 默认工作在远端 sandbox；local trace spool 的 appRoot 不是工具执行 cwd。缺 sandbox 路径到用户 Git 项目的可信绑定，且 trace policy 可关闭参数记录。没有套用同名任意 MCP tool，也没有把本地 OTLP 目录误当编辑目录 |
| Firebender | [JetBrains 1.0.12 安装包](https://plugins.jetbrains.com/files/25224/1109733/Firebender-1.0.12.zip)，JAR 内 ChatHistoryDatabaseService、消息与 sourceSessionId 关系 | 当前数据库 SQL/字段采用 ZKM 混淆；旧 XML/Gson 路径不能证明当前格式。未解出完整 schema/codec，也没有真实数据库样本，因此不编造通用 SQLite 读取 |
| inference.sh | [官方 skills 固定版本](https://github.com/inference-sh/skills/tree/0cbf91c26ada28702c79962406286f0c14292150)的 `tools/infsh-cli/references/running-apps.md`：belt app run / task get JSON 和输出 artifacts | 任意云 app 成功只证明任务结果，不证明用户仓库文件被写入；缺确定的代码编辑工具语义和远端到本地项目映射 |
| Lingma | [官方 Open VSX 2.6.10](https://open-vsx.org/api/Alibaba-Cloud/tongyi-lingma/2.6.10/file/Alibaba-Cloud.tongyi-lingma-2.6.10.vsix)，扩展及内置 Go 服务里确认 chat.db 的 chat_session/chat_record/chat_snapshot/chat_working_space_file/chat_message，以及 Bolt KV | 已区分 APPLIED 预览与 ACCEPTED 接受；文件内容通过 key/content_type 指向 KV，EncodeData/DecodeData 编码尚未解出。没有真实库可核对这个引用链；不能拿相近产品 Qoder 的 schema 代替 |
| MCPJam | [官方 CLI sessions 源码](https://github.com/MCPJam/inspector/blob/c6c376ef07d5edf68d5f26775952def18b98cebe/cli/src/commands/sessions.ts)可读 raw messages/trace spans；`.mcpjam/browser.json` 是连接状态 | Playground 的任意 MCP server 没有统一文件写入语义，cloud project ID 也不是 Git cwd。缺具体 server 身份、成功回执和项目映射；连接凭据不会当成历史 |
| Moxby | [官方安装入口](https://moxby.com/install)指向 Chrome 扩展，取得 3.2.0 CRX 并检查 service-worker、storage.local 和 Bridge WebSocket；[官方 Chat 页面](https://moxby.com/product/moxby-chat)说明保存会话 | 已知保存聊天，但拿到的浏览器包主要是连接/回放/浏览器 recorder。Desktop Bridge 的文件写入结果与持久化格式仍未取得，不能把浏览器动作当 Git 文件编辑 |
| Rovo Dev | [Atlassian 插件 202610.16.1b1](https://acli.atlassian.com/plugins/rovodev/linux/amd64/202610.16.1b1/rovodev.zip)与 [Atlascode 固定版本](https://github.com/atlassian/atlascode/tree/7ef48ac216e6ecb8da95ef5af92a94e5fea23f2d)的 API 类型/会话测试；确认 `~/.rovodev/sessions/<id>/{session_context,metadata}.json` | 当前 Python 会话实现受 PyArmor 保护；公开测试只有 metadata，未含成功写入历史。API 流的 tool-call/tool-return 不能证明 session_context 的持久形状、fork 和裁剪规则；未把推测的 Pydantic 格式当适配 |

上述发行包 SHA256：CodeMaker `0a052a4503d025f0fb5e2b9653ca8889b8c4c8e1ed99d4948ba32e3c14bb8a51`；Firebender `136d479c50222a3ccc95e67e1c6ac57edbb174f9c45cc06c508e2a5580560fb3`；Lingma `74a4cc84734de7a6de58d214b4ccd866e06956fd55a0fefc3c839c03b619a114`；Moxby `e4bfd58df79ea8ba301760dd83d54d7083daf235bada3a02e5016d0bc5240a27`；Rovo Dev `44194bc350289b1a1b3c8daabb6bc2e55b91d2ba251f33a62621e631a1538429`。

## 验证与 Hero 图标

`crates/cli/tests/native18/mod.rs` 使用隔离 Git 仓库和上述原生格式，运行实际 CLI、检查 SQLite 持久证据以及最终 `credits.json`，覆盖：

- 7 个来源的成功编辑，每个 8 行进入最终 verified 归因；Code Studio 同会话两次请求的 4 + 4 行分别保留模型。
- 重复分析、复制历史、Zencoder/Sarvam fork、Posit V3 到 V2 与更换 root ID 的完整导出不增加证据。
- 失败回执、rejected 快照、uncertain 流水、Zenflow dropped/最终 failed 状态撤销既有证据；坏 JSON、无法匹配的快照保留旧证据。
- 模块测试另覆盖 UTF-16 编辑范围、Posit 部分失败 patch、engine 镜像、Tinycloud 正文规范化、Zencoder 最终 diff 与缺失调用补充、Sarvam 缺模型留空。

最终全套检查为 392 项通过、1 项原有重型测试忽略；工作区/App Clippy、CLI bundle 与网站构建通过。Hero 已在隔离 Chromium 中检查中英文、1440/390/320px、深浅主题的 55 个标志及提示框；最后的单色标志修正又在 1440/320px 深浅主题复核。未验证真实手机或其他浏览器。详见 [支持范围的验证部分](AGENT_SUPPORT.md#验证范围)。这些是固定版本的格式和集成验证；真实厂商账户、Windows/Linux 实机和已安装 App 均未逐一验收。

网站在**原 Hero 图标区**由 40 增至 **55 个真实品牌标志**，新增 15 个：Warp、Augment、Droid、AiderDesk、Deep Agents、Dexto、ForgeCode、Jazz、Pochi、AstrBot、Reasonix、Posit Assistant、Tinycloud、Code Studio、Sarvam Code。来源和裁切/主题处理见 [图标来源](site/assets/agents/SOURCES.md)。编辑器/CLI/地区版同一品牌不重复堆放，图标数量不是完整适配数量。
