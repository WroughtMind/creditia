# Creditia 命令行

分析调用共享 Rust 核心；文档、终端输出、图片和动态导出调用共享渲染器。开发期先在仓库根目录执行 `cargo build --release`，安装渲染器依赖，再运行 `node packages/cli/bin/creditia.mjs --help`。

| 命令 | 作用 |
|---|---|
| `discover` / `projects` | 查找本机项目 / 列出已分析的作品 |
| `init` / `analyze` / `update` | 初始化 / 分析 / 更新；`--branch HEAD` 选择当前分支 |
| `collect` / `evidence export` / `evidence import` | 采集和跨机器合并证据 |
| `link --path P` | 明确关联项目的其他完整路径 |
| `people --file F --show` | 保存已确认的姓名、角色、说明，选择公开署名 |
| `doctor` / `blame 文件:行号` | 检查数据源 / 查看归因证据 |
| `doc` / `release-notes` / `pr-comment` | 生成文本；只输出内容，不自动发布评论 |
| `export --list` | 查看原版形式与风格 |
| `export --form F --format png --out FILE` | 导出作品，格式为 SVG、PNG、PDF、GIF、HTML、Markdown 或 JSON |
| `fetch` | 显示终端名片 |
| `stamp` / `hook` | 为提交写入工具与模型声明 |
| `app open` | 打开已构建或安装的桌面 App；可用 `CREDITIA_APP` 指定路径 |

所有分析命令支持 `--json`，退出码为 0 成功、1 失败、2 带警告完成。导出与 `--json` 一起使用时须提供 `--out`，返回文件路径与字节数。核心可用 `CREDITIA_CORE` 明确指定；默认依次查找仓库 release/debug、包内二进制和 PATH。

`--size 1200x630` 指定画布，`--width` 指定栅格输出宽度；不静默裁剪或缩小。PDF 当前是图片文档，矢量用途使用 SVG。HTML 自包含，无需线上组件包。人员通过 `--people` 或配置 `display.people` 开启；姓名别名继续应用。`privacy.hideTitles` 与 `privacy.hidePaths` 在分析生成档案时生效。

人员文件示例：

```json
[{ "name": "署名", "roles": ["构思", "设计", "开发"], "note": "想留下的说明" }]
```
