# MCPJam 执行验收与新增历史入口

剩余实现与验收统一跟踪于 [Issue #110](https://github.com/WroughtMind/creditia/issues/110)，按读取缺口、本地入口验收、账户或样本依赖分别推进；以下已完成结果保持原有范围。

2026-10-28 更新：Codex 分段读取已实现并通过受控回归，详见[实现与验收记录](docs/2026-10-28-issue-110-114-115.md)。下文的分段“未读取”描述对应当时版本；MCPJam 真实分段链与完整受管入口仍未验收。

2026-10-26 合入前复验：修复路径边界、主/子会话持久化和截断历史保护后，再用官方启动链与 Codex 0.164.0 分别执行 attended 的 7 个场景、unattended 的 9 个场景。两组均从默认 `work/project` 软链入口完成执行，各为 5 行 verified，重复扫描不增加，原始历史摘要不变。模型仍为脚本服务。新证据在 `/private/tmp/creditia-integration-mcpjam-{attended,unattended}/`；Rust 工作区 362 项通过、3 项忽略，严格 Clippy 通过。

## 2026-10-26：默认软链入口已通过，新增后台命令归因

**修正上一轮的判断：失败来自测试启动方式遗漏了 MCPJam 官方路径转换，不能概括成 MCPJam 默认入口不可用。** 官方 `translateAdapterCommand` 调用 `confinePath`，先检查 `work/project` 的目标属于获准工作区，再将真实路径传给 bridge 的 `--workdir`。绕过这一步直接运行桥，才会把软链路径传入 Codex 的 macOS 沙箱。

| 本轮实际运行 | 结果 |
|---|---|
| 官方路径转换、回环启动器、bridge + Codex 0.149.1 | 默认 `work/project` 入口完成相对路径创建、拒绝写入、无效修改、原生回读、后台命令；最终 verified 5 行 |
| 相同入口 + 官方 Codex 0.164.0 | 同样通过；两次扫描均为 5 行，原始 JSONL 和 SQLite 摘要不变 |
| Codex 0.164.0 无人值守，官方明确限制的 workspaceWrite 策略，显式 Bash 命令 | 9 个执行场景通过；仓库和私有 TMPDIR 可写，仓库外写入与命令联网被拒；最终 verified 5 行 |
| 已用旧代码扫描过的持久库 | 修复后自动重建原生缓存，从漏计的 3 行更新为 5 行；无需删除用户资料库 |

模型端仍是官方脚本测试服务，所有写入、读文件、沙箱拒绝和历史回执均由真实 Codex 执行。没有账户或云端模型验收，也没有修改用户安装的 Codex/MCPJam。

### 根因与官方版本

核对时 Codex 最新稳定版为 [0.164.0](https://github.com/openai/codex/releases/tag/rust-v0.164.0)。它仍拒绝普通的软链 writable root；`allow_symlinked_codex_home` 只针对 CODEX_HOME，不能解决此工作目录问题。本轮没有启用这项设置、扩大 writableRoots 或关闭沙箱。

实际使用的 [路径转换](https://github.com/MCPJam/inspector/blob/520eb9aa3c8b7b965d92611c414543c91b04f16e/mcpjam-inspector/server/utils/harness/local/command-translation.ts)、[路径范围检查](https://github.com/MCPJam/inspector/blob/520eb9aa3c8b7b965d92611c414543c91b04f16e/mcpjam-inspector/server/utils/harness/local/confine.ts)、回环 launcher、bridge 和无人值守 sandbox policy 五份源码，与官方当日 `520eb9aa3c8b7b965d92611c414543c91b04f16e` 逐字节一致。临时源码归档没有 `.git`，验收保存五份源文件的 SHA-256；不伪造其 Git HEAD。0.149.1 与 0.164.0 均实际通过，证实无需依赖升级来规避软链检查。

无人值守还有一个独立执行边界：macOS 默认 zsh 的 `TMPPREFIX=/tmp/zsh` 不随 TMPDIR 改变，多行重定向会因禁止 `/tmp` 而失败。保留了该失败记录。随后使用 Codex 原生 `exec_command` 的 `shell: "/bin/bash", login: false` 参数完成同一写入流程；官方沙箱策略仍为 `networkAccess:false, excludeSlashTmp:true, excludeTmpdirEnvVar:false, writableRoots:[]`。这不等于默认 zsh 下所有 shell 语法都已通过。

### 已实现的读取修复

后台 `exec_command` 的首个回执只有进程编号；最终退出码属于后续 `write_stdin` 调用。现在按原生进程编号把空输入轮询与原命令配对，明确退出 0 后才提取原命令中的行证据，贡献仍归实际执行的 Codex。非零退出、缺少最终回执、无对应进程、stdout 中伪造进程编号均排除；发送交互输入的命令不凭初始命令文本归因。重复扫描不新增贡献，旧缓存会自动刷新。

真实样本是 [`codex-background.jsonl`](crates/adapters/tests/fixtures/mcpjam/codex-background.jsonl)，含成功、退出 9 和未完成三种情况。后两种文件真实存在，仍没有计入成功贡献。回归还验证了可读原生历史失去最终回执时，持久库会撤回旧证据。

### 可重复执行的验收

已保存 [`mcpjam.mjs`](crates/adapters/tests/live/mcpjam.mjs)，测试入口不再直接把链接传给 bridge。它构建未修改的官方转换器、bridge 和回环 launcher，从默认链接参数启动，并验证：链接仍存在、获准真实工作区正确、越界链接拒绝、实际写入/回读、后台成功与失败、最终归因、重复扫描以及原始历史只读。启动器完全退出后才开始文件摘要检查，避免原生进程最后刷新历史与扫描重叠。

需要 MCPJam 官方源码归档或 checkout、隔离的 npm 依赖和 Codex vendor 目录。依赖为 `@ai-sdk/harness@1.0.117`、`@modelcontextprotocol/sdk@1.30.0`、`zod@4.4.3`、`ws@8.22.0`、`esbuild@0.28.2`；vendor 使用官方 `@openai/codex@0.149.1` 或 `0.164.0`。脚本不安装或升级全局软件，输出目录必须是新的。

```sh
cargo build -p creditia-cli
node crates/adapters/tests/live/mcpjam.mjs "$MCPJAM_SOURCE" "$MCPJAM_DEPS" "$CODEX_VENDOR" "$NEW_RUN_DIR" target/debug/creditia-core attended
node crates/adapters/tests/live/mcpjam.mjs "$MCPJAM_SOURCE" "$MCPJAM_DEPS" "$CODEX_VENDOR" "$ANOTHER_NEW_RUN_DIR" target/debug/creditia-core unattended
```

本机最终证据位于 `/private/tmp/creditia-mcpjam-20261026/{attended-149,attended-164,unattended-bash-final}/`。各目录的 `launch.json` 保留请求的链接路径与官方转换后的 argv，`sources.json` 保留版本及来源摘要，`execution-result.json` 保留两次归因和原生文件摘要。无人值守的越界写入要求回执明确包含退出 1 和 `Operation not permitted`，不只检查文件不存在。`unattended-164/` 保留 zsh 多行重定向的失败记录。

验证：Rust 工作区 **343 项通过、3 项默认忽略**，严格 Clippy 通过。真实链路及回执回归通过。本轮未改 Hero 或发布产物；完整受管运行时安装/签名校验、账户租约代理、云端 RPC、Cursor 后端和 Codex `history_base` 分段重建仍未验收。

## 2026-10-25 的执行记录

日期：2026-10-25。承接 [上一轮](SUPPORT_NATIVE_LIVE.md)。用户确认没有 MCPJam 账户，继续完成不需要账户的部分。本轮实际运行了官方桥、Codex 和 Claude SDK；模型端使用 MCPJam 官方测试服务器，因此 **不是完整登录产品或真实商业模型的验收**。未发布或替换已安装 App。

| 执行入口 | 结果 | 明确边界 |
|---|---|---|
| 官方 MCPJam Codex 桥 + Codex 0.149.1，真实仓库 cwd | 成功写入及原生回读 3 行；拒绝创建、无效修改均排除；Creditia 两次分析 verified 3 行 | 脚本模型；没有运行账户代理和完整受管启动流程 |
| 同一 Codex 桥，MCPJam 默认 `work/project` 链接 cwd | 相对路径创建失败；绝对路径创建可落盘，但后续命令与回读被 macOS 沙箱拒绝 | 该默认路径 **未通过完整验收**，保留失败历史，没有用成功分支替代这一结论 |
| MCPJam 使用的官方 Claude 桥 + SDK，`work/project` 链接 cwd | 实际写入、原生 Read 回读、拒绝写入均通过；新增自动读取，Creditia 两次分析 verified 1 行 | 脚本模型；不是 MCPJam 账户模型代理或云端执行验收 |
| inference.sh 原生导出及其他 Codex 宿主 | 新增带退出码外壳的原生 patch 回执；与原生完成事件去重 | 本轮没有新增 inference.sh 商业云端运行；此前官方驱动真实模型验收仍见上一轮 |

## 隔离执行与来源

工作目录为 `/private/tmp/creditia-mcpjam-20261025`，测试仓库、HOME、npm 缓存、历史库均在其中。服务只监听回环地址，运行结束后关闭本次进程树；没有读取或复制用户账户密钥。测试生成的模型标识 `gpt-5.5`、`claude-haiku-4-5` 只用于验证原生字段读取，不能声称调用了这些在线模型。

- MCPJam 源码：`c6c376ef07d5edf68d5f26775952def18b98cebe`，Inspector 包版本 3.16.2。实际使用的 `bridge/index.ts` 与官方固定版本逐字节比较一致。
- Codex：官方 npm `@openai/codex@0.149.1`；`@ai-sdk/harness@1.0.117`。桥和 host-tools MCP 服务从未修改的官方源码构建；模型为官方 [Responses 测试服务](https://github.com/MCPJam/inspector/blob/c6c376ef07d5edf68d5f26775952def18b98cebe/.spike-codex-appserver/probe/fake-responses-server.mjs)。
- Claude：MCPJam 使用的 `@ai-sdk/harness-claude-code@1.0.121`、`@anthropic-ai/claude-agent-sdk@0.3.245`、原生记录版本 2.1.245。模型为官方 [Anthropic 测试服务](https://github.com/MCPJam/inspector/blob/c6c376ef07d5edf68d5f26775952def18b98cebe/mcpjam-inspector/server/utils/harness/local/conformance/mock-anthropic.mjs)。
- 本地路径来自官方 [supervised-provider.ts](https://github.com/MCPJam/inspector/blob/c6c376ef07d5edf68d5f26775952def18b98cebe/mcpjam-inspector/server/utils/harness/local/supervised-provider.ts) 和 [session-env.ts](https://github.com/MCPJam/inspector/blob/c6c376ef07d5edf68d5f26775952def18b98cebe/mcpjam-inspector/server/utils/harness/local/session-env.ts)。

执行脚本为隔离目录中的 `run-bridge.mjs`、`run-linked-bridge.mjs`、`run-claude.mjs`。其中仅测试驱动、临时仓库路径及构建输出由本次创建；没有修改第三方执行器实现。它们直接驱动官方桥，未冒充账户身份或绕过服务端登录。

## 已落实到 Creditia 的读取行为

[`harness_paths.rs`](crates/adapters/src/harness_paths.rs) 统一固定目录发现和真实工作区路径解析。只枚举已知会话层级，不遍历工作区链接或读取 bridge 凭据。

```text
~/.mcpjam/harness-local-sessions/<session>/
  work/project -> 授权仓库
  work/.agent-runs/<run>/codex-home/sessions/**/*.jsonl
  home/.claude/projects/<encoded-project>/*.jsonl
```

Claude 的新增入口接入正式 analyze、discover、doctor；解析记录中的 cwd 和工具路径，不靠编码目录名猜项目。按原生 session ID 合并普通 CLAUDE_CONFIG_DIR 与宿主副本；同一父会话的 `agent-*.jsonl` 子 agent 保留为独立历史文件。普通 Claude 日志也要求配对的成功工具回执；只有 Write 请求、没有结果，或明确拒绝/失败的记录不计。已有成功证据在可用原生会话重建后与当前回执对齐，修正旧缓存，不修改来源文件。

Codex 新增实际遇到的 `Exit code: 0 / Wall time / Output` 包装回执。只接受明确成功状态；非零退出码、仅在输出正文中出现的成功字样、进程未能启动、尚未完成的命令均不能证明成功。同一个调用同时出现在 `patch_apply_end` 和 `response_item` 时，只记一份。此次读取变化会重建旧 Codex 缓存。

原始工具调用和回执的未改写摘录存放在 [`fixtures/mcpjam`](crates/adapters/tests/fixtures/mcpjam/README.md)，包含提取范围和 SHA-256。自动测试只把临时路径绑定到自己的测试仓库；原始完整运行保留在隔离目录。实际运行后的 `creditia-result.json` 保存两次分析、证据数量及来源文件摘要：

- `acceptance/creditia-result.json`：最终 verified 3 行，重复分析一致。持久库另保留一条 shell 命令候选哈希；它不是第四行写入，最终归因仍为 3 行。
- `claude-acceptance/creditia-result.json`：1 条 Claude 行证据，最终 verified 1 行，重复分析一致。
- 两次扫描前后原生 JSONL、Codex SQLite 索引摘要一致。

## Codex 默认链接路径的失败

官方 bridge 的 workspace-write 设置在 macOS 的 Codex 0.149.1 上报告：`symlinked writable roots are not supported`。错误明确指向 MCPJam 的 `work/project`，不能把它解释成缺少历史或 Creditia 读取失败。

测试分别保留了链接 cwd 和真实 cwd 的结果。链接 cwd 的绝对路径创建有成功回执，但同轮后续命令无法启动；真实 cwd 才完成写入和原生回读。这是两个不同条件，不能以后一项替前一项宣称通过。Creditia 已能读取真实成功的写入，并排除命令启动失败；没有修改用户的 MCPJam 安装或放宽沙箱。

尚未验收的部分：MCPJam 登录与模型租约代理、完整受管运行时/工作区授权启动、云端 RPC、Cursor 后端；Codex 分段 `history_base` 重建仍不在当前读取范围。没有凭目录、配置或模拟模型输出宣称这些已支持。

## Hero 与检查

新增 inference.sh 官网三角标志和 MCPJam 官方 MCP/JAM 标志，原始素材保留在 `site/assets/agents`。Hero 仅转换显示颜色为 currentColor，MCPJam 去掉背景底板；[来源及摘要](site/assets/agents/SOURCES.md) 已补齐。总数由 75 增至 **77**，不改变历史支持口径。

完整网站构建及下载配置测试通过。独立 Playwright 浏览器使用专用测试资料和 `--password-store=basic --use-mock-keychain`；桌面浅色、手机深色及手机横向标志栏末尾、英文页均检查到两枚标志，前景填充一致，无页面横向溢出。截图保存在 `output/playwright/hero-77-*.png`。

Rust 工作区 **342 项通过、3 项默认忽略**，严格 Clippy 通过。新增回归验证真实回执、宿主目录、跨目录去重、子 agent 分离、失败/无结果排除、重复扫描及旧证据校正；旧 Claude 正向测试补上原本缺失的成功回执，另有独立无回执反例，未放宽成功条件。

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
node --test site/downloads.test.mjs
node --use-env-proxy site/build.ts
```
