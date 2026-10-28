# Eve、inference.sh 与 MCPJam 的执行适配

实测日期：2026-10-24。承接 [此前调查](SUPPORT_AGENT3.md)，本轮包含代码修改和真实执行。没有发版或安装替换用户的 Creditia。

2026-10-25 补充：MCPJam 的官方 Codex / Claude 桥与实际执行器验收、Claude 独立历史入口、默认链接路径的 macOS 限制，以及两枚新 Hero 标志见 [后续记录](SUPPORT_MCPJAM_LIVE.md)。下表保留本轮当时的边界。

| 项目 | 实际完成 | 验收边界 |
|---|---|---|
| Eve 0.83.0 | 官方本地源码沙箱真实创建文件并读回；新增原生运行版本、沙箱挂载与仓库绑定读取；CLI 最终 verified 3 行 | 验收的是本地官方 `eve/self-modification/sandbox`；GitHub 云端、任意自定义挂载、覆盖写及 shell 编辑不由本次结果推定通过 |
| inference.sh | 官方 agentprotocol CodexBackend 调用真实模型写文件，官方 codec 导出原生历史；补齐无 SQLite 索引的读取，CLI 最终 verified 3 行 | 执行者为 Codex / gpt-6-luna；独立云端工具和其他 harness 未完成此次运行验收 |
| MCPJam | 自动发现本地 Codex harness 的独立历史目录、解析工作区链接、与普通 Codex 历史去重；接入正式分析、discover 和 doctor | 原生目录集成经过受控端到端测试；没有声称完整 MCPJam 登录、模型代理、云端 RPC 已实测 |
| CodeMaker AI CLI 1.6.0 | 下载官方 macOS arm64 安装包、校验摘要、真实执行 generate code | 返回没有有效 API key，输入文件未变。用户确认没有账户，在线正向验收停止于此；不能标成已支持。Skills 目录中的 CodeMaker 与 Y3 移植版的对应仍需独立证据 |

当前目录口径是 **72 个历史入口、5 个执行宿主、1 个未确认格式**。入口数量仍包含有限读取和条件读取，不代表所有产品的所有执行方式均已通过验收。

## Eve：真实落盘、绑定与反例

隔离目录 `/private/tmp/creditia-native-live-20261024/eve` 安装 npm `eve@0.83.0`、`just-bash@3.6.0`，通过官方 `chatgpt()` 模型提供者使用现有本机登录。未读取、打印或复制账户密钥。服务仅监听 `127.0.0.1:42931`，验收后已关闭。

官方 self-modification 动态模型继承在该版本不能直接继承 Codex 模型对象，因此使用官方扩展覆盖入口显式配置子 agent 模型。子 agent 的 `sandbox.ts` 为：

```ts
export { default } from 'eve/self-modification/sandbox';
```

该覆盖是测试项目自己的配置，未修改 Eve 安装包。官方源码说明此沙箱将 `/source` 挂载到本地 authored app 的 `agent/`，使用 `ReadWriteFs`，并拒绝符号链接。普通 `/workspace` 虚拟目录及只有同名子 agent、没有该沙箱的 `/source` 都不能证明项目写入。

本次确实遇到了这些反例：前两轮工具回执成功，但文件仅存在于虚拟目录，项目目录没有文件。读取器继续排除这两条记录。启用官方挂载后的新建文件使用原生 `write_file`，`existed: false`，同一会话 `read_file` 返回相同的三行；磁盘文件逐字节一致：

```text
creditia-eve-verified-write
executor=official-eve-0.83.0
evidence=native-write-receipt
```

关键身份：子会话 `wrun_01M5PY4MVN21ZH8YS1X3T8XJ9Y`，写入调用 `call_b45685d113eb4b1b8d22a6b7372bed39`；Git 保存提交 `8d83c1c33740782bc96047817c256fb26d6ac49b`。该保存提交没有手工填写 agent 署名。

实现位于 [`eve.rs`](crates/adapters/src/eve.rs) 与 [`eve/local.rs`](crates/adapters/src/eve/local.rs)，依次核验：

1. Workflow v2 已完成步骤中的 `eve.bundle`、子 agent node ID 和动态运行版本。
2. 原生 run 的 `deploymentId`、Eve 版本及该代 `generation.json`，不能用当前清单替代历史清单。
3. 该代编译清单及 source graph。Eve 会把清单中的根目录改到 snapshot，保留原编译摘要；读取器还原这次目录改写后校验原摘要。
4. 清单绑定的沙箱源码必须仍与原生 `revisionHash` 一致，并是上述官方沙箱直接导出；普通默认沙箱、不同源码、缺少旧代文件都排除。
5. `/source` 只映射到该应用真实 `agent/`，且必须位于用户选中的精确项目路径内，不能靠项目同名匹配。
6. 只有成功的新建文件回执提供正文；覆盖写缺少前态、shell 无完整差分时仍不计。每次调用保持独立去重身份。

真实 CLI 两次扫描都得到 **Eve 3 条持久行证据、最终 verified 3 行**，两次结果一致。Workflow、trace、run 与 snapshot JSON 扫描前后摘要一致。其他真实写入走了 shell 或覆盖写，按既有边界没有冒充可核验行。模型保持未知，没有拿测试项目设置反填全部工具记录。

原始成功回执保留在 [`live-local`](crates/adapters/tests/fixtures/eve/live-local)，包含 SHA-256 锁定的写入与回读回归；运行检查点含账户能力和其他上下文，完整数据只留在隔离目录，不提交到仓库。验收结果为 `/private/tmp/creditia-native-live-20261024/eve-acceptance.json`。

官方来源：[源码挂载](https://github.com/vercel/eve/blob/3c03b204d0fabb8fc155a93a40ba34d2c51f1628/packages/eve/src/self-modification/filesystem.ts)、[沙箱源码摘要](https://github.com/vercel/eve/blob/3c03b204d0fabb8fc155a93a40ba34d2c51f1628/packages/eve/src/compiler/normalize-sandbox.ts)、[开发代清单重写](https://github.com/vercel/eve/blob/3c03b204d0fabb8fc155a93a40ba34d2c51f1628/packages/eve/src/internal/nitro/dev-runtime-artifacts.ts)。

## inference.sh：官方驱动、官方原生导出

使用固定源码 [agentprotocol a336c97](https://github.com/inference-sh/agentprotocol/tree/a336c97eb0c942be66bac8dc3fbbd95ba4caa9c0)，Go 1.26.7，现有 Codex CLI 0.162.1，在 `/private/tmp/creditia-native-live-20261024/inference-repo` 创建空 Git 基线。

实际调用官方 `driver.CodexBackend.Open`、`Prompt` 和事件流，工作目录限制在该仓库，沙箱为 workspace-write，模型为 gpt-6-luna。未使用 mock Responses 服务。完成后通过官方 `transcript/codex.Codec.Read` / `Write` 导出同一原生会话到 `inference-export/.codex`。导出不创建 `state_5.sqlite`，这正是旧读取器漏读的入口。

真实会话 ID `01a16ded-6e96-7c92-9d43-831c384b15fc`，原生完成事件为 `CommandExecution`，退出码 0，命令中的字面量 apply_patch heredoc 和回执的目标文件一致。目标文件三行 SHA-256：`2dd7372eaedace4fd87fa46a01d46d668986a86a8d04ceeeec71445ed471d2b5`。原始完成事件摘录保留在 [`inference-live-command.json`](crates/adapters/tests/fixtures/codex/inference-live-command.json)，未包含账户配置或其他开发者上下文。

[`codex.rs`](crates/adapters/src/codex.rs) 与 [`codex/histories.rs`](crates/adapters/src/codex/histories.rs) 新增：

- 无索引时从原生 `session_meta` 读取 ID、cwd、时间，逐轮读取模型；不凭目录名填写项目。
- 同一原生会话从多个目录出现只保留一份，已有完整记录优先于缺失内容的索引。
- 读取 `patch_apply_end` 的成功更改、配对的原生 apply_patch 回执，以及严格限定的成功命令 heredoc；没有结果、失败、不同目标或未执行的 shell 片段不计。
- 跨 agent 转换生成的 `originator=agentprotocol` 且没有 CLI 版本的头部不当成 Codex 原生执行。本轮原生实跑时尚未实现 `history_base`；后续分段读取和受控回归见 [2026-10-28 记录](docs/2026-10-28-issue-110-114-115.md)，真实分段样本仍待验收。
- 模型取原生 turn_context，归因始终是实际执行的 Codex，不增加一份 inference.sh 贡献。

只向 Creditia 指定该导出目录，使用独立持久库分析；两次均为 **3 条 Codex / gpt-6-luna 行证据，最终 verified 3 行**。导出没有 SQLite 索引，源历史摘要前后一致。报告为 `/private/tmp/creditia-native-live-20261024/inference-analysis.json`。

官方来源：[执行入口](https://github.com/inference-sh/agentprotocol/blob/a336c97eb0c942be66bac8dc3fbbd95ba4caa9c0/driver/harness.go)、[Codex 驱动](https://github.com/inference-sh/agentprotocol/blob/a336c97eb0c942be66bac8dc3fbbd95ba4caa9c0/driver/codex.go)、[原生读取与导出](https://github.com/inference-sh/agentprotocol/blob/a336c97eb0c942be66bac8dc3fbbd95ba4caa9c0/transcript/codex/codex.go)。

## MCPJam：实际本地执行路径

官方本地模式的 Codex 会话目录为：

```text
~/.mcpjam/harness-local-sessions/<session>/work/
  project -> 授权工作区
  .agent-runs/<run>/codex-home/
    state_5.sqlite  （可选）
    sessions/**/*.jsonl
```

Creditia 现在自动发现这些目录，按原生 thread ID 去重，解析 `work/project` 的真实指向，再进入同一个 Codex reader。没有读取 bridge 配置、账户配置或任意 RPC 导出；目录中不存在历史就没有贡献。MCPJam 会清理部分会话状态，因此不能把缺目录解释成用户一定没有使用过它。

受控端到端测试通过真实 CLI 验证：独立目录两行贡献、普通 CODEX_HOME 中同一会话副本不重复、符号链接工作区正确识别、外部项目排除、discover / doctor 接通、源文件字节不变。MCPJam 的其他执行后端和云端 MCP 调用不由这些 Codex 原生日志推定支持。

官方来源：[本地根目录](https://github.com/MCPJam/inspector/blob/c6c376ef07d5edf68d5f26775952def18b98cebe/mcpjam-inspector/server/utils/harness/local/grants.ts)、[会话目录和工作区链接](https://github.com/MCPJam/inspector/blob/c6c376ef07d5edf68d5f26775952def18b98cebe/mcpjam-inspector/server/utils/harness/local/supervised-provider.ts)、[Codex 执行目录](https://github.com/MCPJam/inspector/blob/c6c376ef07d5edf68d5f26775952def18b98cebe/mcpjam-inspector/server/utils/harness/codex-appserver/codex-appserver-harness.ts)、[CODEX_HOME](https://github.com/MCPJam/inspector/blob/c6c376ef07d5edf68d5f26775952def18b98cebe/mcpjam-inspector/server/utils/harness/codex-appserver/bridge/index.ts)。

## CodeMaker 与验证

[官方 CodeMaker CLI 1.6.0](https://github.com/codemakerai/codemaker-cli/releases/tag/v1.6.0) macOS arm64 安装包 SHA-256 为 `e882974422b4b816274b0aa9243989b04d4c40c0d01bcf80c5d03f4846a5ee67`。隔离运行 `codemaker-cli generate code acceptance.go` 返回 `No valid api key found: failed to load configuration from config file`，文件摘要仍为 `92e59264b5ab5b130694700de185a65a3a2e9de6e01f7c5b196a677c4b372cd6`。这不是成功编辑。

本轮 Rust 工作区 340 项通过、3 项默认忽略，严格 Clippy 通过。自动回归覆盖原生回执、生成版本、沙箱摘要、虚拟目录排除、无索引导出、失败/提案/错目标排除和跨目录去重。验证命令：

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

真实执行与自动格式测试分开列示。当前改动在本地工作区，未提交、未发布，未替换已安装 App。
