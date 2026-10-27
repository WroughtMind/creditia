# 78 个 Agent 入口的支持范围

中文 | [English](AGENT_SUPPORT.en.md)

支持状态更新：2026-10-26。目录来自 [Vercel Skills 的 agent registry](https://github.com/vercel-labs/skills/blob/main/src/agents.ts)，源文件 blob `c4af0e54d6f0c87d7d6863315be5c7acb4ed62fe`。其 Universal 是通用安装目标，不作为一种 agent，余下 78 项与用户清单一致。权威机器目录是 [`packages/schema/agents.json`](packages/schema/agents.json)，CLI 与展示名称共同读取。

全部 78 项已接入工具 ID、明确的启动身份、提交署名和 skill 安装指引。其中 **72 项已有历史读取入口、5 项从实际执行宿主归因、1 项仍未确认原生编辑格式**。本轮完成 Eve 本地源码沙箱和 inference.sh Codex 驱动/原生导出的真实写入 → CLI → 持久证据 → 最终档案验收；MCPJam 的本地 Codex/Claude 历史、默认工作区链接与官方启动链已通过脚本模型验收，包含成功、拒绝、后台命令与重复采集。CodeMaker 已运行官方 CLI，但缺少账户密钥。详见 [原生实测](SUPPORT_NATIVE_LIVE.md)和 [MCPJam 验收](SUPPORT_MCPJAM_LIVE.md)。这些数字包含有限编辑、会话、导入和条件读取，不代表 78 个产品全部模式已验收。

`creditia agents --json` 返回逐项状态及安装命令；`creditia agents --agent <ID>` 查询单项。表中 ID 用于 `--agent`，内部 harness 是档案里的稳定工具身份。Antigravity 编辑器 / CLI 分开；TRAE / TRAE CN 共用编辑器工具身份，Qoder / Qoder CN 共用 Qoder 工具身份并读取各自目录。安装 skill、设置身份、提交声明、读到会话、确认编辑是不同能力。

## 全部入口

所有行均可使用 `creditia stamp --agent <ID>`，提交盖章依赖用户已安装的项目 hook；没有模型记录时不填写模型。路径为各 agent 的项目级 skill 目录，由 Skills 安装器管理。

| Agent | --agent ID | 历史读取范围 | 项目 skill 目录 |
|---|---|---|---|
| AiderDesk | `aider-desk` | 原生 context v2：成功 Power 编辑和已提交 Aider 差分 | `.aider-desk/skills` |
| Amp | `amp` | 已有读取 | `.agents/skills` |
| Antigravity | `antigravity` | 原生编辑 | `.agents/skills` |
| Antigravity CLI | `antigravity-cli` | 已有读取 | `.agents/skills` |
| AstrBot | `astrbot` | 原生 SQLite：成功文件工具；仅能映射的绝对路径，模型未知 | `data/skills` |
| Autohand Code CLI | `autohand-code` | 有限原生编辑 + ACP | `.autohand/skills` |
| Augment | `augment` | 原生编辑 | `.augment/skills` |
| IBM Bob | `bob` | 原生编辑 + ACP | `.bob/skills` |
| Claude Code | `claude-code` | 已有读取 | `.claude/skills` |
| OpenClaw | `openclaw` | 原生成功 edit 差分；旧无差分记录仅会话 | `skills` |
| Cline | `cline` | 已有读取 | `.agents/skills` |
| CodeArts Agent | `codearts-agent` | 条件读取：已知 OpenCode v1 表结构与成功差分；厂商实录待验 | `.codeartsdoer/skills` |
| CodeBuddy | `codebuddy` | 原生编辑 | `.codebuddy/skills` |
| Codemaker | `codemaker` | 官方 CodeMaker AI CLI 1.6.0 已实测，因无 API key 停在认证；Y3 移植版产品对应和落盘回执仍未确认 | `.codemaker/skills` |
| Code Studio | `codestudio` | 原生已接受编辑时间线与文本快照；逐请求模型 | `.codestudio/skills` |
| Codex | `codex` | 已有读取 | `.agents/skills` |
| Command Code | `command-code` | 原生编辑 | `.commandcode/skills` |
| Continue | `continue` | 已有读取 | `.continue/skills` |
| Cortex Code | `cortex` | 原生编辑 + ACP | `.cortex/skills` |
| Crush | `crush` | 已有读取 | `.crush/skills` |
| Cursor | `cursor` | 已有读取 | `.agents/skills` |
| Deep Agents | `deepagents` | 原生 LangGraph SQLite：安全解码 JSON/MessagePack、CLI 本地文件回执 | `.agents/skills` |
| Devin for Terminal | `devin` | 有限 hook 编辑 + 云端 ACP 导入 | `.devin/skills` |
| Dexto | `dexto` | 原生 SQLite kv/list：配对成功写入与编辑；工作区路径映射 | `.agents/skills` |
| Droid | `droid` | 原生编辑 | `.agents/skills` |
| Eve | `eve` | Eve 0.83.0 本地官方源码沙箱已真实验收：写入回执、固定版本挂载和仓库归因；GitHub 绑定/eve-code 另有条件读取，覆盖写及 shell 编辑仍排除，模型未知 | `agent/skills` |
| Firebender | `firebender` | 原生项目 SQLite：完成 Write 回执及子 agent；模型未知，排除 IDE 自动改写 | `.agents/skills` |
| ForgeCode | `forgecode` | 原生 .forge.db：逐消息模型、成功 fs 工具实际差分 | `.forge/skills` |
| fx | `fx` | 原生 v2：CRC32C 日志、SHA-256 数据块和保存后的文件结果 | `.agents/skills` |
| Gemini CLI | `gemini-cli` | 已有读取 | `.agents/skills` |
| GitHub Copilot | `github-copilot` | 已有读取 | `.agents/skills` |
| Goose | `goose` | 已有读取 | `.goose/skills` |
| Grok Build | `grok` | 已有读取 | `.grok/skills` |
| Hermes Agent | `hermes-agent` | 原生成功 patch 回执；模型未知 | `.hermes/skills` |
| inference.sh | `inference-sh` | Codex 模式已用官方 agentprotocol 驱动与导出真实验收：支持无 SQLite 索引的原生历史，贡献归实际执行器；独立云端工具仍未确认 | `.inferencesh/skills` |
| Jazz | `jazz` | 原生 JSONL/gzip：完整成功差分，历史模型未知 | `.jazz/skills` |
| Junie | `junie` | 有限原生编辑 | `.junie/skills` |
| iFlow CLI | `iflow-cli` | 原生编辑 | `.iflow/skills` |
| Kilo Code | `kilo` | 已有读取 | `.agents/skills` |
| Kimchi | `kimchi` | 原生编辑 + ACP | `.kimchi/skills` |
| Kimi Code CLI | `kimi-code-cli` | 已有读取 | `.agents/skills` |
| Kiro CLI | `kiro-cli` | 有限原生编辑 | `.kiro/skills` |
| Kode | `kode` | 原生 projects JSONL：成功 Write/Edit structuredPatch；子 agent 分开 | `.kode/skills` |
| Lingma | `lingma` | 原生 local.db + workingSpace：已接受且前后 MD5 匹配；已用公开真实库验证，支持迁移的 Windows 快照，模型未知 | `.lingma/skills` |
| Loaf | `loaf` | 已查原生工作日志；代码贡献从实际执行的宿主 agent 读取 | `.agents/skills` |
| MCPJam | `mcpjam` | 自动读取本地 Codex 和 Claude 执行器历史，解析项目链接并按原生会话去重；官方桥与真实执行器的脚本模型验收通过，贡献归执行器；账户模型代理、完整受管启动及云端 RPC 未验收 | `.mcpjam/skills` |
| MiniMax Code | `minimax-code` | 原生编辑 | `.minimax/skills` |
| Mistral Vibe | `mistral-vibe` | 原生编辑 | `.vibe/skills` |
| Moxby | `moxby` | Desktop 3.2.0 原生 SQLite：最终成功 file_edit 回执、绝对路径及逐轮模型 | `.moxby/skills` |
| Mux | `mux` | 原生 active/archive JSONL：成功编辑差分，排除 fork 复制前缀 | `.mux/skills` |
| OpenCode | `opencode` | 已有读取 | `.agents/skills` |
| OpenHands | `openhands` | 官方导入编辑 | `.openhands/skills` |
| Ona | `ona` | 当前官方 Codex Agent 使用 Codex 执行器；在执行环境读取其原生日志 | `.ona/skills` |
| Pi | `pi` | 原生编辑 | `.agents/skills` |
| Posit Assistant | `posit-assistant` | 原生 V2/V3 活动分支；成功编辑回执与逐消息模型 | `.posit/assistant/skills` |
| Qoder | `qoder` | CLI 原生编辑（国际 / CN 独立目录） | `.qoder/skills` |
| Qoder CN | `qoder-cn` | CLI 原生编辑（国际 / CN 独立目录） | `.qoder/skills` |
| Qwen Code | `qwen-code` | 已有读取 | `.qwen/skills` |
| Replit | `replit` | 官方导入编辑 | `.agents/skills` |
| Reasonix | `reasonix` | 原生 flat/append/replace/schema-2 DAG：当前分支、成功写后回执 | `.reasonix/skills` |
| Rovo Dev | `rovodev` | 原生 session_context：成功创建和实际编辑差分、逐响应模型；保留已裁剪历史证据 | `.rovodev/skills` |
| Roo Code | `roo` | 已有读取 | `.roo/skills` |
| Sarvam Code | `sarvam-code` | 原生 rollout；成功 patch_apply_end、逐轮模型 | `.agents/skills` |
| Tabnine CLI | `tabnine-cli` | 有限原生编辑 | `.tabnine/agent/skills` |
| Terramind | `terramind` | Nucleus 原生本地工具执行流水；成功 Write/Edit 与逐步骤模型 | `.terramind/skills` |
| Tinycloud | `tinycloud` | 原生 SessionStore JSONL；绝对路径、实际写入文本与模型 | `.tinycloud/skills` |
| Trae | `trae` | 原生编辑 | `.trae/skills` |
| Trae CN | `trae-cn` | 原生编辑 | `.trae/skills` |
| Warp | `warp` | 原生编辑 | `.agents/skills` |
| Windsurf | `windsurf` | 官方导入编辑 | `.windsurf/skills` |
| Zed | `zed` | 已有读取 | `.agents/skills` |
| ZCode | `zcode` | 原生编辑 | `.zcode/skills` |
| Zencoder | `zencoder` | CLI 原生历史与 IDE 执行后差分；CLI 逐消息模型 | `.zencoder/skills` |
| Zenflow | `zenflow` | 原生标准化日志数据库；最终成功编辑，模型未知 | `.zencoder/skills` |
| Neovate | `neovate` | 原生编辑 | `.neovate/skills` |
| Pochi | `pochi` | 原生 LiveStore SQLite：成功编辑结果；内存/OPFS 需持久化导出 | `.pochi/skills` |
| PromptScript | `promptscript` | 已查指令编译与遥测：无代码编辑回执；从执行宿主归因 | `.agents/skills` |
| AdaL | `adal` | 官方 JSON/NDJSON 导入：成功 replace_by_string；HTML 历史未解码 | `.adal/skills` |

## 接入命令

```sh
# 查看入口；不会安装任何 agent
creditia agents --agent pi --json

# 原命令的参数、输入、输出、退出码保留；身份由子进程继承
creditia run --agent pi -- pi

# 只给下一次提交声明身份，stamp 本身不安装 hook
creditia stamp --agent pi

# 安装当前本地开发版 skill；将 /path/to/creditia 换成此仓库路径
npx skills add /path/to/creditia --skill creditia -a pi

# 已确认有 ACP 命令的工具，在其 ACP 客户端内包装原启动命令
creditia capture --agent <ID> -- <该工具原来的 ACP 启动命令>
```

`run` 不捕获日志、不产生编辑证据，也不自动安装 hook。显式 `--model` 只用于声明当前已知执行模型；不会从品牌、默认设置或父 agent 环境猜模型。`--` 后所有参数交给原命令。

ACP 是客户端和 agent 间的通信协议。通用采集器接受全部已登记工具身份，但只有工具实际支持 ACP、且协议提供成功编辑差分或文件写入结果时才能采到逐行证据。没有逐一核实的 ACP 启动命令不能称作该产品已经接通。已核对的命令见 [ADAPTERS.md](ADAPTERS.md)。

## 原生历史核查记录

最新实测及代码实现见 [SUPPORT_NATIVE_LIVE.md](SUPPORT_NATIVE_LIVE.md)和 [SUPPORT_MCPJAM_LIVE.md](SUPPORT_MCPJAM_LIVE.md)，剩余执行方式及账户验收在 [Issue #110](https://github.com/WroughtMind/creditia/issues/110) 跟踪。此前调查和灵码真实历史验收保留在 [SUPPORT_AGENT3.md](SUPPORT_AGENT3.md)，Eve 初始实现见 [SUPPORT_AGENT4.md](SUPPORT_AGENT4.md)。更早的读取器记录继续保留在 `SUPPORT_AGENT*.md`。

- **Hermes Agent**：读取 `HERMES_HOME/state.db`（默认 `~/.hermes/state.db`，Creditia 覆盖入口 `CREDITIA_HERMES_AGENT_DIR`），配对 `messages.tool_calls` 和 tool 回执，从成功 `patch` 的实际 diff 与确认目标提取新增行。排除 inactive、继承消息、no-change、失败结果与没有配对调用的回执。会话末尾可变的 model 不用于行归因。官方源码：[状态表](https://github.com/NousResearch/hermes-agent/blob/main/hermes_state_common.py)、[消息](https://github.com/NousResearch/hermes-agent/blob/main/hermes_state_messages.py)、[文件工具](https://github.com/NousResearch/hermes-agent/blob/main/tools/file_tools.py)。
- **OpenClaw**：读取 `OPENCLAW_STATE_DIR/agents/*/sessions/openclaw-agent.sqlite` 和旧 session JSONL，默认 `~/.openclaw`（Creditia 覆盖入口 `CREDITIA_OPENCLAW_DIR`）。读取 `transcript_events` 的 JSON / zstd 内容，只使用配对成功 `edit` 回执内的实际 unified patch，模型来自当次 assistant 消息；无差分的旧日志仅提供会话。当前数据库优先于同 agent 的旧 JSONL；排除 ACP-owned 会话、分支继承和 superseded 记录，冲突副本不产生贡献。格式依据 [OpenClaw 源码固定版本](https://github.com/openclaw/openclaw/tree/1197ee98bb5eab98f61befcbeafba382aa382e8b)，包括 `openclaw-agent-schema.sql`、`transcript-payload-read.ts` 与 `sessions/tools/{edit,file-diff}.ts`。
- **Qoder CN**：增加独立 `~/.qoder-cn/projects/**/*.jsonl`，尊重官方 `QODERCN_CONFIG_DIR`，也提供 `CREDITIA_QODER_CN_DIR`。国际版 `~/.qoder` 仍独立读取。只计算现有 reader 能确认的配对成功编辑；[官方 session 文档](https://docs.qoder.cn/en/cli/sessions)确认目录和环境变量，尚未真实账号运行验收。

SQLite 只读打开，并用事务读取同一份快照。新增 Hermes / OpenClaw 完整重读的会话用于撤销已失效的旧编辑，读取失败不伪造成功。除这些新增实现外，本次还整合了原独立工作区中的本地 reader、官方云端导入和 ACP 采集；其来源、格式限制和原核查时间保留在 [ADAPTERS.md](ADAPTERS.md) 及 `SUPPORT_*.md`。

## 验证范围

2026-10-26 合入前复验：Rust 工作区 **362 项通过、3 项默认忽略**，严格 Clippy 通过；渲染器 38 项、桌面前端 38 项、CLI/打包/下载 17 项通过。完整双语站点、嵌入 App 与桌面前端构建通过，仓库外安装实际 npm 包后可生成署名文档和 PNG。隔离 Chromium 在中英文、桌面/手机、深浅色八组检查中均显示 77 个单色 Hero 标志，无页面横向溢出或控制台错误。这里没有重新安装或验收本机原生 App。

MCPJam 官方启动链与 Codex 0.164.0 的 attended/unattended 模式已在最终修复后复跑，各得 5 行 verified，重复扫描不变，历史原件摘要不变；模型端为脚本服务。Eve 与 inference.sh 的此前真实执行验收另见 [SUPPORT_NATIVE_LIVE.md](SUPPORT_NATIVE_LIVE.md)，MCPJam 详情见 [SUPPORT_MCPJAM_LIVE.md](SUPPORT_MCPJAM_LIVE.md)。

- 构造隔离仓库与官方格式记录，逐一运行 78 项的真实 CLI `run` / `whoami` / `stamp`，确认身份、模型留空、没有隐式安装 hook；未知 ID 与冲突参数被拒绝。
- Eve 0.83.0 官方本地源码沙箱经真实模型调用完成写入、读回和 Git 保存；Creditia 最终 verified 3 行，重复扫描仍为 3，原生文件哈希不变。普通虚拟沙箱写入被排除。GitHub/eve-code 路径仍以受控格式回归为主，不能由这次本地测试推定云端全部通过。
- 灵码使用固定版本的原始 SQLite 与 214 行快照完成 CLI → 持久证据 → 最终档案验收；verified 119 行、declared 95 行，重复分析不增加贡献，原件字节不变。比较用 Git 仓库是隔离重建，未运行 Windows 客户端。Rovo 的原有验证见 [上一轮](SUPPORT_AGENT6.md#hero-与验证)。
- Firebender / Moxby 经过原生数据库 → CLI → 持久证据 → 最终档案验证；覆盖子 agent、fork、失败/删除撤回、损坏保留、跨会话和截断回执排除。[上一轮验证](SUPPORT_AGENT8.md#验证与-hero)。
- 上一轮 7 个读取器经过原生格式 → 真实 CLI → SQLite 持久库 → 最终 verified 档案；覆盖逐请求模型、重复导出/fork、失败/拒绝/丢弃撤销，以及读取损坏保留旧证据。详见 [本轮验证](SUPPORT_AGENT18.md#验证与-hero-图标)。
- `run` 验证原参数、标准输入输出、错误输出、退出码和父 agent 环境隔离。
- 合入审查新增真实 CLI/持久库回归：Claude 主/子会话分别保存并迁移旧库；Codex 与共享原生读取器遇到截断或损坏时保留持久证据；原生快照按会话进度选取，同进度冲突会提示并保留已有证据。共享路径入口排除 `..` 与可确认的外部软链接，不借用进程目录补全缺少原生工作目录的路径。
- Hermes / OpenClaw / Qoder CN 经过原生记录 → reader → 持久库 → 最终档案验证；重复分析不增加证据，失败或 inactive 记录能撤销持久证据，旧 OpenClaw JSONL 不复活已撤销贡献。
- 全目录在两套渲染器中的名称、比例、徽章出图和原版片尾名单检查；新增品牌 SVG 源文件与生成数据一致。缺 logo 的工具仍显示名字与首字母，不把 logo 存在与否当成作者资格。
- 这些包括源码、构造记录及部分公开原始历史验证，不是 78 个产品的真实账户、Windows / Linux 或已安装 App 验收。未发版，未修改任何 agent 的真实日志。

## 仍需补齐

CodeMaker 的原生编辑格式仍未确认，官方 CLI 正向实测需要可用账户。Eve 本地官方源码沙箱已完成正向验收；inference.sh / MCPJam 的 Codex 执行方式已有实际读取入口，独立云端工具、其他 harness 及 MCPJam 完整账户运行仍有边界。Ona、Loaf、PromptScript 继续从执行宿主归因。详见 [SUPPORT_NATIVE_LIVE.md](SUPPORT_NATIVE_LIVE.md)。

Hero 原图标区现为 **77 个真实品牌标志**，包含 Kode、灵码、Rovo Dev、Eve、inference.sh 和 MCPJam 的官方标志，统一使用页面单色前景；使用官方项目/网站/发行包素材及有来源的图标目录。同一品牌的编辑器、CLI、地区入口不重复堆放，图标不是“完整适配”标记。手机图标条可横向滚动。[来源记录](site/assets/agents/SOURCES.md)。
