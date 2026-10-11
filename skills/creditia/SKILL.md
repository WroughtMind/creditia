---
name: creditia
description: 用本地记录整理项目的协作档案，生成 CREDITS.md、图片或独立网页；保留用户选定的原版视觉，区分可核对记录与提交声明。
---

# Creditia

用户要求为项目制作 credits、署名文档或展示产物时使用。全程通过 CLI 读取结果，计算不交给大模型。遵循用户已经明确的项目、风格、公开范围和授权，不重复确认。

1. 用户指定了项目就直接用 `--repo`；没有指定时用 `creditia discover --json` 查询，再让用户选择。不要凭同名目录判断同一项目。
2. 缺少项目配置时运行 `creditia init --repo P --json`。按配置分支分析；用户明确需要当前工作分支时加 `--branch HEAD`。
3. 运行 `creditia analyze --repo P --json`。仅在用户需要 PR 信息且允许读取 GitHub 时加 `--github`。读取警告和证据覆盖，不对未确认比例设任意门槛。
4. 多路径用 `creditia link --repo P --path P2 --json` 明确关联，再更新分析。旧路径可属于另一系统；不为消除未确认部分猜测目录、模型或作者。
5. `creditia export --list --json` 查看可用形式与原版主题。沿用已有选择；未确定的产品视觉只问必要的一项。不得自行重设计已有产物。
6. 人员姓名、角色和说明须来自用户确认。保存为 JSON 数组后用 `creditia people --repo P --file FILE --show --json`；未要求公开时不加 `--show`。不自动称谁主导，不把代码行占比说成人的功劳占比。
7. 本地生成结果：`creditia doc --repo P --json` 或 `creditia export --repo P --form tier --theme minimal-light --format png --out FILE --json`。可选 PNG、SVG、PDF、GIF、独立 HTML；徽章支持 SVG/Markdown。HTML 无需外部 CDN。需要桌面预览时用 `creditia app open --repo P --form tier`，操作窗口遵循用户的后台或画中画要求。
8. 核对产物，解释记录范围与未确认部分。提交评论、推送、公开发布或部署仍须获得相应授权；普通本地生成按用户已授权范围直接完成。

`credits.json` 的占比按历史新增代码行计算。日志匹配、提交声明和未确认分别报告；不将 Token、提示次数或会话跨度折算成人工小时，不按工具或模型名猜服务商。没有专用读取器时可明确盖章，但不将声明称为逐行证明。

不得修改 agent 原始日志、账号数据库或密钥。证据交换文件包含相对路径与会话元数据，分享前核对用户允许的范围。跨平台的路径支持与实际机器验证要分别说明。

作为 agent 提交代码时，可在用户允许提交的范围内使用 `creditia stamp --harness <实际工具> --model <实际模型>`；不要填写不确定的模型。
