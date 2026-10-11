# creditia

> 为你和 AI 一起做出的作品署名。

**开发中，尚未发布。** 读取 Git 和本机编程工具记录，整理为 `credits.json` 与 `CREDITS.md`；同一份档案用于排行、片尾字幕、小票、徽章和统计图。保留原有 15 套主题及各形式的可选版式。

人的构思、设计、开发、审阅和决策采用用户确认的独立署名。代码行占比不代表人的价值或劳动时间。证据不足时保留“未确认”，不依赖大模型推断。口径、多路径和各数据源边界见 [产品说明](PRODUCT.md)，原版视觉保护范围见 [工作台说明](DESIGN.md)。

## 本地开发

需要 Git、Rust、Node 22.18 或以上，以及对应平台的 Tauri 系统依赖。

```bash
cargo build --release
npm ci --prefix packages/render
npm ci --prefix apps/desktop
node packages/cli/bin/creditia.mjs --help
```

桌面首次构建还需生成随 App 打包的本地字体：

```bash
cd apps/desktop
npm run fonts
npm run tauri dev
```

## 使用

开发期可将 `creditia` 命令指向本仓库的 `packages/cli/bin/creditia.mjs`。

```bash
creditia init --repo /path/to/project
creditia analyze --repo /path/to/project --branch HEAD --json
creditia doc --repo /path/to/project
creditia export --repo /path/to/project --form tier --theme minimal-light --format png --out tier.png
creditia export --repo /path/to/project --form roll --theme film --format html --out credits.html
creditia link --repo /path/to/project --path /another/project-copy
creditia doctor --json
```

添加 `--github` 时才通过已登录的 GitHub CLI 读取 PR 元数据。盖章属于提交声明，不代替逐行证据。默认不公开人员姓名；使用 App 的“制作人员”，或 `creditia people --file people.json --show` 保存用户确认的署名。

## 组成

| 目录 | 职责 |
|---|---|
| `crates/core` | 归因规则、配置、行哈希 |
| `crates/adapters` | Git 与七类工具记录的只读解析 |
| `crates/store` | 本地作品库、证据、路径关联和快照 |
| `crates/cli` | CLI 与桌面共用的分析服务 |
| `packages/render` | 原版作品与五种导出格式 |
| `packages/cli` | 文档、导出、终端与 App 启动命令 |
| `packages/schema` | 档案格式及类型 |
| `apps/desktop` | Tauri 桌面工作台 |
| `skills/creditia` | 面向 agent 的操作说明 |

MIT，见 [协议文件](LICENSE)。
