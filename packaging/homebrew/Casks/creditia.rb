# creditia 的 App（macOS）。放进 WroughtMind/homebrew-tap 的 Casks/ 下：brew install --cask wroughtmind/tap/creditia
# 发版时改 version，sha256 换成 creditia_<version>_universal.dmg 的：shasum -a 256 creditia_*_universal.dmg
cask "creditia" do
  version "0.1.0"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"

  url "https://github.com/WroughtMind/creditia/releases/download/v#{version}/creditia_#{version}_universal.dmg"
  name "creditia"
  desc "Work out which AI tools and models wrote the code in a repo"
  homepage "https://wroughtmind.github.io/creditia/"

  app "creditia.app"

  # App 还没有 Apple 签名：Homebrew 下载时会打上隔离标记，macOS 就说「已损坏」。装好后替用户去掉，打开就能用
  postflight do
    system_command "/usr/bin/xattr", args: ["-dr", "com.apple.quarantine", "#{appdir}/creditia.app"]
  end

  zap trash: [
    "~/Library/Caches/dev.creditia.app",
    "~/Library/Preferences/dev.creditia.app.plist",
    "~/Library/Saved Application State/dev.creditia.app.savedState",
    "~/Library/WebKit/dev.creditia.app",
  ]
end
