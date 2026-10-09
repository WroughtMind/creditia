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
creditia discover                 本机有 agent 活动的项目
creditia init                     写 .creditia/config.json（主导者、邮箱、排除规则）
creditia analyze --github         分析，写 .creditia/credits.json
creditia update                   增量
creditia doc                      写 CREDITS.md（只改 <!-- creditia:start/end --> 区间）
creditia release-notes v1.0.0     该版本的 credits 段落
creditia pr-comment 519           该 PR 的一句话归因
creditia export --list            全部形式、风格、格式
creditia export --form tier --theme film --format png --out tier.png
creditia export --form badge --format md       # 写 assets/creditia-badge.svg 并打印 Markdown
creditia export --form tier --format embed     # <creditia-card> 片段
creditia fetch                    终端名片
creditia blame src/a.ts:42        这一行是谁写的
creditia doctor                   各数据源是否可读
creditia hook install             之后每次提交自动盖章
creditia stamp --harness codex --model gpt-5.6-sol    给下一次提交盖章（agent 调用）
creditia app open --form tier     在 App 里打开
```

所有命令支持 `--json`。退出码：0 成功，1 失败，2 有警告。

## 展示选项

- `--director`：在图上写出主导者（默认不写）
- `--lang zh|en`
- `--size 1200x630`：把形式居中放进指定画布（社交媒体尺寸）
- `--theme`：minimal-light、minimal-dark、film、terminal、glass、editorial、mono

## 隐私

`.creditia/config.json` 里 `privacy.hideTitles`、`privacy.hidePaths`、`privacy.aliases` 生效后，`credits.json` 本身就不含 PR 标题、文件名和真名，所有导出自然也没有。
