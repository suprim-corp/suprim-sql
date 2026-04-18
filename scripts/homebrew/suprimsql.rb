# Homebrew Cask formula for SuprimSQL
# Repo: suprim-corp/homebrew-tap
# Install: brew install --cask suprim-corp/tap/suprimsql
#
# This is a template — SHA256 and version are updated by CI on each release.

cask "suprimsql" do
  version "0.1.0"
  sha256 "f0b7d290b91f94240f0497bb4d70e85545dc51c11bbd8a6cc1336ab157b4cd8b"

  url "https://github.com/suprim-corp/suprim-sql/releases/download/#{version}/SuprimSQL-#{version}-macos-universal.dmg"
  name "SuprimSQL"
  desc "Native database management tool. Fast, lightweight, no Electron."
  homepage "https://suprim.dev"

  depends_on macos: ">= :monterey"

  app "SuprimSQL.app"

  zap trash: [
    "~/Library/Application Support/SuprimSQL",
    "~/Library/Preferences/com.suprim.sql.plist",
  ]
end
