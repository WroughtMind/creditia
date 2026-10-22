# creditia 的 App（macOS）。和魏碑放在同一个 tap 里：taekchef/homebrew-tap 的 Casks/ 下，brew install --cask taekchef/tap/creditia
# 发版时改 version，sha256 换成 creditia_<version>_universal.dmg 的：shasum -a 256 creditia_*_universal.dmg
cask "creditia" do
  version "0.1.0"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"

  url "https://github.com/WroughtMind/creditia/releases/download/v#{version}/creditia_#{version}_universal.dmg"
  name "creditia"
  desc "Work out which AI tools and models wrote the code in a repo"
  homepage "https://wroughtmind.github.io/creditia/"

  app "creditia.app"

  # App 还没有 Apple 签名：Homebrew 下载时会打上隔离标记，macOS 就说「已损坏」。
  # 不在 postflight 里替用户去掉（Homebrew 不鼓励这么做），官网和 README 给的安装命令后面直接接着 xattr
  caveats <<~EOS
    creditia is not signed by Apple yet. If macOS says it is damaged, run:
      xattr -dr com.apple.quarantine #{appdir}/creditia.app
  EOS

  zap trash: [
    "~/Library/Caches/dev.creditia.app",
    "~/Library/Preferences/dev.creditia.app.plist",
    "~/Library/Saved Application State/dev.creditia.app.savedState",
    "~/Library/WebKit/dev.creditia.app",
  ]
end
