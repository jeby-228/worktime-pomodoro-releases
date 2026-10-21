# CI/CD 配置說明

本專案使用 GitHub Actions 進行持續整合和持續部署。

## 🔧 工具選擇

### 前端
- **Linter**: [oxlint](https://oxc.rs/) - 超快速的 JavaScript/TypeScript linter
- **Formatter**: oxlint (使用 `--fix` 模式)

### 後端（Rust）
- **Formatter**: `cargo fmt` (rustfmt)
- **Linter**: `cargo clippy`
- **Tests**: `cargo test`

## 📋 本地開發指令

### 前端檢查
```bash
# Lint 檢查
npm run lint

# 格式檢查（dry-run）
npm run format:check

# 自動修復格式問題
npx oxlint --fix .
```

### Rust 檢查
```bash
# 格式化
cargo fmt --manifest-path=src-tauri/Cargo.toml --all

# 格式檢查
cargo fmt --manifest-path=src-tauri/Cargo.toml --all -- --check

# Clippy 檢查
cargo clippy --manifest-path=src-tauri/Cargo.toml --all-targets -- -D warnings

# 運行測試
cargo test --manifest-path=src-tauri/Cargo.toml
```

### 一鍵檢查
```bash
# 安裝 pre-commit hook（自動在提交前檢查）
cp .githooks/pre-commit .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit

# 手動運行所有檢查
.git/hooks/pre-commit
```

## 🔄 CI 工作流程（`.github/workflows/ci.yml`）

### 觸發條件
- `push` 到 `main` 分支
- Pull Request 到 `main` 分支

### 檢查項目

#### 1. **Rust Check & Test**
- ✅ Rust 格式檢查（`cargo fmt --check`）
- ✅ Clippy 靜態分析（`-D warnings`）
- ✅ 單元測試（`cargo test`）
- ✅ Release 建置（`cargo build --release`）

#### 2. **Frontend Lint & Build**
- ✅ oxlint 檢查
- ✅ 格式檢查（oxlint dry-run）
- ✅ Astro 建置（`npm run build`）
- 📦 上傳 dist 產物

#### 3. **Integration Build**
- ✅ Tauri 完整建置
- 📦 上傳 bundle 產物
- ⏱️ 保留 7 天

## 🚀 Release 工作流程（`.github/workflows/release.yml`）

### 觸發條件
- 推送 `v*` tag（例如：`v0.1.0`, `v1.2.3`）

### 建置平台
- 🐧 **Linux**: x86_64-unknown-linux-gnu (Ubuntu 22.04)
  - 產出：`.AppImage`, `.deb`
- 🪟 **Windows**: x86_64-pc-windows-msvc
  - 產出：`.msi`
- 🍎 **macOS**: x86_64-apple-darwin, aarch64-apple-darwin
  - 產出：`.dmg`

### Release 流程
1. 從 tag 創建 GitHub Release
2. 在各平台並行建置 Tauri 應用
3. 上傳簽名的安裝包到 Release
4. 生成 `latest.json` 更新清單

## 🔐 GitHub Secrets 配置

在 GitHub 倉庫設置 → Secrets and variables → Actions 中添加：

### Required（Release 需要）
```
TAURI_SIGNING_PRIVATE_KEY
生成方式：npm run tauri signer generate -- -w ~/.tauri/worktime-pomodoro.key
內容：私鑰檔案的完整內容
```

### Optional
```
TAURI_SIGNING_PRIVATE_KEY_PASSWORD
如果私鑰有密碼保護，在此設置
```

**⚠️ 重要**: `GITHUB_TOKEN` 由 GitHub 自動提供，無需手動添加。

## 📦 發佈新版本

### 步驟 1：更新版本號
```bash
# 更新 Cargo.toml
vim src-tauri/Cargo.toml
# version = "0.2.0"

# 更新 tauri.conf.json
vim src-tauri/tauri.conf.json
# "version": "0.2.0"

# 更新 package.json
vim package.json
# "version": "0.2.0"
```

### 步驟 2：提交並創建 Tag
```bash
git add -A
git commit -m "chore: bump version to 0.2.0"
git push origin main

# 創建 tag
git tag -a v0.2.0 -m "Release v0.2.0"
git push origin v0.2.0
```

### 步驟 3：等待 CI/CD
- GitHub Actions 會自動：
  1. 運行所有檢查
  2. 在各平台建置
  3. 創建 Release
  4. 上傳安裝包
  5. 生成 `latest.json`

### 步驟 4：編輯 Release Notes（可選）
在 GitHub Releases 頁面編輯自動生成的 Release，添加更新說明。

## 🔍 oxlint 配置（`.oxlintrc.json`）

```json
{
  "rules": {
    "correctness": "warn",    // 正確性檢查
    "suspicious": "warn",      // 可疑代碼檢查
    "perf": "warn"            // 性能問題檢查
  },
  "env": {
    "browser": true,
    "es2021": true
  },
  "globals": {
    "__TAURI__": "readonly"   // Tauri 全局變量
  },
  "ignorePatterns": [
    "node_modules",
    "dist",
    "src-tauri/target",
    ".astro"
  ]
}
```

## 🐛 故障排除

### CI 失敗：Rust 格式問題
```bash
cargo fmt --manifest-path=src-tauri/Cargo.toml --all
git add -A
git commit -m "style: format rust code"
```

### CI 失敗：Clippy 警告
```bash
cargo clippy --manifest-path=src-tauri/Cargo.toml --all-targets --fix --allow-dirty
git add -A
git commit -m "fix: resolve clippy warnings"
```

### CI 失敗：前端 lint
```bash
npx oxlint --fix .
git add -A
git commit -m "style: fix linting issues"
```

### Release 失敗：簽名問題
確認 GitHub Secrets 中已正確設置 `TAURI_SIGNING_PRIVATE_KEY`。

### 本地測試 CI
```bash
# 模擬完整 CI 流程
./scripts/ci-local.sh
```

## 📊 CI 狀態徽章

在 README.md 中添加：

```markdown
[![CI](https://github.com/yourname/worktime-pomodoro/workflows/CI/badge.svg)](https://github.com/yourname/worktime-pomodoro/actions)
[![Release](https://github.com/yourname/worktime-pomodoro/workflows/Release/badge.svg)](https://github.com/yourname/worktime-pomodoro/releases)
```

## 🎯 最佳實踐

1. **每次提交前運行檢查**：使用 pre-commit hook
2. **小步提交**：每個 commit 應該通過 CI
3. **描述性提交訊息**：遵循 [Conventional Commits](https://www.conventionalcommits.org/)
4. **定期發佈**：小版本快速迭代，大版本慎重測試
5. **監控 CI**：PR 合併前確保 CI 通過

## 🔗 相關資源

- [oxlint 文檔](https://oxc.rs/docs/guide/usage/linter.html)
- [GitHub Actions 文檔](https://docs.github.com/en/actions)
- [Tauri Actions](https://github.com/tauri-apps/tauri-action)
- [Conventional Commits](https://www.conventionalcommits.org/)
