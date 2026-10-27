# 发布 creditia

正式发布由维护者在整体验收后发起。宣传片更新单独处理，不是本次安装包准备的前置条件。

## 首版渠道

- macOS：Apple 芯片和 Intel 分别提供 DMG；生成双架构 Homebrew cask，维护者更新既有 `taekchef/homebrew-tap`。
- Windows：x64 NSIS 安装程序。
- Linux：x64 AppImage、deb；分析核心另外提供 arm64。
- CLI：npm 包 `creditia`，首次分析从同版本 GitHub Release 下载核心。
- Windows ARM64、MSI、winget、Scoop、Flatpak、RPM、CLI formula、安装脚本和 Docker 暂不提供，不在官网显示安装命令。

文件名由 `packaging/release.mjs` 定义，官网与打包流程共用。不得用别的架构产物改名补齐。

## 先构建候选包

PR 涉及发布文件时，或手动运行 `release` workflow 时，只上传 Actions artifacts，不创建公开 Release。`release-candidate` 包含所有安装包、分析核心、逐文件 `.sha256`、`SHA256SUMS` 和生成的 `creditia.rb`。

1. 检查版本一致：`node packaging/release.mjs check`；标签必须与 manifest 版本相同。
2. 确认每个构建任务通过。macOS App 与随包核心均须通过 `codesign --verify --deep --strict`；独立核心也使用 ad-hoc 签名。
3. 在没有源码构建产物、`CREDITIA_CORE` 或 PATH 中开发版核心的环境验证：安装 → 打开 → 添加项目 → 分析 → 导出。分别记录 Apple 芯片、Intel、Windows、Linux；只有构建通过不能记成用户安装验收通过。
4. 浏览器下载后的 Gatekeeper / SmartScreen 首次打开属于可选实机抽查，未做时如实记录，不要求为此专门准备设备。抽查时保留浏览器下载产生的隔离标记，按实际系统提示操作，不要预先移除隔离标记或重签名制造通过。当前 macOS 为 ad-hoc 签名、未公证，Windows 安装包未签名；这些限制应在安装说明中保留。损坏或签名失败先修产物。
5. Homebrew 不自动清除隔离标记。校验值由最终签名后的文件计算，不使用占位值。
6. 用 `npm pack` 生成 tarball，在仓库以外安装并执行 `creditia --help`、用公开档案执行 `creditia doc`，检查包内容和渲染依赖。首次下载核心的端到端检查在 Release 公开后执行。

## 刷新自身档案

在有真实 agent 日志的机器、最新待发布提交上操作：

```sh
creditia analyze
creditia publish --check
# 检查 PR/提交标题、文件路径、署名、工作时间、仓库地址；需要时用 --hide 隐藏相应类别。
creditia publish
creditia doc --credits credits.json --out CREDITS.md
```

`publish` 只写公开 JSON，不会顺便更新 Markdown。不要把本机路径、邮箱或日志原文提交进仓库。核对档案的 head、统计与当前文件；历史记录中的旧名称不能靠搜索替换伪造。

## 正式发布顺序

1. 完成整体验收，由维护者打 `v<version>` 标签。
2. 等 `release` 全部构建成功，检查生成的 **draft Release** 资产与 `SHA256SUMS`。缺任一目标文件时汇总任务会失败，不应公开残缺 Release。
3. 维护者发布草稿 Release。
4. 维护者发布 npm 包；在全新缓存、无开发版核心的环境执行 `npx creditia@<version> analyze`，确认下载同版本 Release 核心并完成分析。
5. 将 Release 附带的 `creditia.rb` 更新到 `taekchef/homebrew-tap/Casks/creditia.rb`，验证安装。无需为本仓库设置跨仓库写入 token。
6. 完成上述入口验证后更新 README 安装说明（PR #94 需与当前源码安装和 macOS 提示协调），手动运行 `pages` workflow。
7. 检查官网中英文、各系统入口、校验链接、复制命令和 `self/` 页面。Release published 会触发一次网站更新；npm/tap 随后上线后还需再运行 pages。

官网构建实时核对当前版本的公开 Release、npm 与 cask；没有发布的显示准备中，查询异常则构建失败，不伪造渠道状态。不会自动发布 npm 或修改 Homebrew tap。

## 安装验收

`installer-acceptance` workflow 只下载指定 `release` 运行的 `release-candidate`，核对所有 SHA-256，再执行 Windows NSIS 安装、Linux deb 安装 / AppImage 解包运行、Intel Mac DMG 复制安装。它不打标签、不构建新安装包、不发布版本。每次候选包汇总后会自动验收同一次运行的产物；也可手动填入既有候选运行 ID 重试。

每个平台使用安装目录中的核心分析当前仓库，核对生成档案的 head 和文件记录，并验证 App 能持续运行。Windows 通过系统原生控件、Linux 通过实际 WebKit 界面自动操作安装后的 App：重新分析、切换语言/模型与工具分组/风格/展墙与漫游，导出 PNG、SVG、PDF、HTML、GIF，再正常退出并重启验证设置恢复。Linux 同时覆盖 deb 和直接执行 AppImage。Intel Mac CI 覆盖 DMG 安装、签名、随包核心分析与启动。结果和日志保存在 `acceptance-<runner>` artifact；CI 中缺少本机 agent 日志应如实保留警告。

上述 CI 加上 Apple Silicon 本机的分析、五种导出和重启恢复，作为当前候选包的功能验收依据。额外逐平台人工体验为可选抽查；自动操作不等于人工手感，也不证明浏览器下载后的 Gatekeeper / SmartScreen 流程已通过。macOS 本机验收使用独立安装目录和 `CREDITIA_HOME`，保护原有安装和数据。正式 Release、npm 首次下载核心、Homebrew 与官网渠道互通仍须在正式发布后另行验证。
