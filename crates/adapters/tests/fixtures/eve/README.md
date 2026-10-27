# Eve 原生合同样本

这些是受控样本，不是真实用户账号历史。测试会话、GitHub 仓库和正文均为测试构造；不含用户提示词或凭据。

`generate.mjs` 使用固定官方源码与编码器：

- [vercel/eve 3c03b204d0fabb8fc155a93a40ba34d2c51f1628](https://github.com/vercel/eve/tree/3c03b204d0fabb8fc155a93a40ba34d2c51f1628)：直接调用 `packages/eve/src/shared/value-delta.ts` 的 `diffValue` 和 `packages/eve-code/extension/lib/patch.ts` 的 `applyPatchToSandbox`。
- [devalue 6.0.3](https://registry.npmjs.org/devalue/-/devalue-6.0.3.tgz)：生成原生引用图，tarball SHA256 为 `85c9aaabc79dc55a4b16dd3d97c9b047ab5a1a90e7d99e24683dc940145e3e13`。
- [@opentelemetry/otlp-transformer 0.214.0](https://www.npmjs.com/package/@opentelemetry/otlp-transformer/v/0.214.0)：使用原生 `JsonTraceSerializer.serializeRequest` 生成 OTLP JSON。
- Workflow 的 `devl` / Uint8Array envelope 按 `@workflow/core 5.2.3`、`@workflow/serde 5.0.0`、`@workflow/world-local 5.1.2` 的实际合同构造；没有调用整个 Workflow 引擎。

生成器将官方补丁执行器接到隔离临时文件系统，实际写入并核对 304 字节正文，再把其真实 `files` 结果写入 OTLP。测试架只将 Linux `realpath -m` 映射为 macOS 可用的路径正规化；官方补丁代码未修改。新建文件 span 与检查点 envelope 由受控输入构造，不能视为完整 Eve agent 运行。

| 文件 | 用途 |
|---|---|
| `checkpoint.json` | Workflow JSON 中的 base64 / devalue 引用图，包含原生 GitHub channel 增量和 runtime revision |
| `checkpoint-readable.json` | 同一检查点解码后的输入/输出，便于审阅 |
| `write.otlp.json` | 框架新建文件工具的受控成功回执 |
| `patch.otlp.json` | 官方补丁执行器的实际文件结果，使用官方 OTLP 编码器序列化 |
| `expected.json` | 预期写入正文 |
| `generate.mjs` | 可复现生成器，不需要模型、账号或网络请求 |

在已核对的官方源码目录和临时依赖目录下，使用 Node 24（支持直接导入 TypeScript）：

```sh
npm install --ignore-scripts --no-audit --no-fund --prefix /tmp/creditia-eve-fixture-runtime devalue@6.0.3 @opentelemetry/otlp-transformer@0.214.0
node crates/adapters/tests/fixtures/eve/generate.mjs /absolute/path/to/pinned-eve-source /tmp/creditia-eve-fixture-runtime/node_modules
cargo test -p creditia-adapters eve_codec
cargo test -p creditia-cli --test e2e native4
```

生成器只在临时目录执行已核查的官方补丁函数，不安装或启动 Eve。输出路径记录在终端，正文/轨迹不依赖该随机路径。CLI 测试另外构造最小编译清单与 metadata，并替换 `__EVE_BUNDLE_ROOT__` 为隔离路径；该 metadata 不是实际编译器输出。测试校验其 hash 与回合版本，覆盖被替换的清单和缺失 metadata。

没有使用真实模型或托管沙箱，尚不构成厂商真实账号验收。完整读取条件见 [核查记录](../../../../../SUPPORT_AGENT4.md)。

后续真实样本发现 Workflow 输入还包含 `StepArguments.args` 包装；生成器与样本已纠正。原始公开样本的哈希、可重复验收和限制见 [本轮记录](../../../../../SUPPORT_AGENT3.md)。
