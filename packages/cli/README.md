# creditia（CLI）

`creditia` 是面向人和 agent 的命令行入口：分析交给 Rust 核心 `creditia-core`，档案、导出、终端展示在这里完成。

## 安装（开发期）

```bash
cargo build --release                    # 产出 target/release/creditia-core
cd packages/render && npm i              # 渲染器依赖（字体等）
alias creditia='node ~/projects/creditia/packages/cli/bin/creditia.mjs'
```

找核心二进制的顺序：`CREDITIA_CORE` 环境变量 → 仓库 `target/release|debug` → 包内 `bin/<platform>-<arch>/` → `PATH`。

## 命令

```
creditia discover                     本机有 agent 活动的项目
creditia agents --json                78 个入口及历史读取状态、skill 安装命令
creditia agents --agent openclaw --json  单个入口
creditia run --agent pi -- pi         给子进程明确工具身份，保留原参数和输入输出
creditia stamp --agent pi             下一次提交声明（需要已安装提交 hook）
creditia capture --agent <ID> -- <ACP 命令>  在 ACP 客户端中采集后续成功编辑
creditia init                         写 .creditia/config.json（你的提交身份、口径、路径、隐私）
creditia analyze --github             分析，写 .creditia/credits.json（--level strict|standard|loose）
creditia paths | link <路径> | unlink  一个项目的多处路径
creditia whoami                       当前在哪个 agent 里（给自己盖章用）
creditia doc                          写 CREDITS.md（只改标记区间）
creditia export --list                全部作品、版式、风格、格式、画幅
creditia export --piece roll --style cinema --variant scroll --format html --out credits.html
creditia export --piece tier --style ink --format png --size og --out og.png
creditia export --piece tier --style cyber --format gif --out tier.gif
creditia board --style glass --out credits.html     全部作品一页，可交互
creditia fetch                        终端名片
```

查询与分析命令支持 `--json`。`run`、`capture` 透传子进程输入输出及退出码，`--` 后的参数原样交给子进程。`run` 的工具身份和 `stamp` 是声明，不把它们算成逐行编辑证据；未知模型不填。原生历史、官方导入与协议采集的边界见仓库的 `AGENT_SUPPORT.md` 和 `ADAPTERS.md`。

其他贡献者默认写 git 名字，`--no-names` 这次不写，配置里 `display.hideNames: true` 一直不写；`--by contributor` 按贡献者看。
