# Creditia 发布

桌面以 GPUI 安装包交付。macOS 的 Apple 芯片与 Intel 各有 DMG 和 ZIP，Windows x64 提供 NSIS 与 ZIP，Linux x64 提供 DEB 和 tar.gz。CLI 使用同版的独立分析核心，另支持 Linux ARM64。

## 桌面候选

在开发 PR 上添加 `gpui-candidate` 标签，或手动运行 GPUI 候选工作流。每个平台构建安装包并校验安装后的启动、旧项目、作品、导出和重启。主平台运行项目切换、取消与重试、设置、导出恢复及高 DPI 动画；性能对比使用同机的 Tauri 与 GPUI 构建。完整作品目录和完整性能矩阵可按需运行。

将实际查看的代表作品和使用流程记入评审记录。依据 [GPUI 发布步骤](gpui/RELEASE.md) 选择同一源码的原生、核心、性能结果和评审，生成带有安装包、验收结论和 SHA256SUMS 的候选 Release。记录发布标签及实际测试的源码 SHA 到 `packaging/desktop-channel.json`。每个平台都从这一次构建取文件。

## 正式版本

1. 检查 Rust、渲染器和 CLI 版本，运行 `node packaging/release.mjs check`。确认已公开对应 GPUI 候选并更新 desktop-channel。
2. 创建匹配版本的 `vX.Y.Z` 标签。release 工作流在五种目标上运行独立核心的分析，并把通过桌面验收的原始安装包复制进正式 Release。它检查候选源码、发行说明、文件哈希和实际桌面代码。
3. 打开草稿核对文件、校验和和 Homebrew cask，发布正式 Release。官网通过已有的发布资产显示下载。npm 和 Homebrew 继续按各自版本单独发布。

## 回退

退出当前应用，保留项目目录中的 `.creditia`、证据库、偏好和旧安装包。[上一个 Tauri 0.1.0 安装包](https://github.com/WroughtMind/creditia/releases/download/gpui-v0.1.0-rc.1/Creditia-Tauri-rollback.zip)使用历史构建的原始文件。安装旧版后检查项目数量与最近一次分析。具体系统操作见[安装说明](gpui/INSTALL.md)。
