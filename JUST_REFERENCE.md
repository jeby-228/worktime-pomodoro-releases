# Just 指令快速參考

本文件列出專案中常用的 `just` 指令。

## 安裝 Just

```bash
# 使用 Cargo 安裝
cargo install just

# macOS
brew install just

# 查看完整安裝說明
# https://github.com/casey/just#installation
```

## 快速開始

```bash
# 列出所有可用指令
just --list

# 完整初始化專案（首次設置）
just init

# 啟動開發模式
just dev

# 執行所有檢查
just check

# 建置 release 版本
just build-release
```

## 開發工作流程

### 日常開發

```bash
# 啟動開發伺服器
just dev

# 格式化所有程式碼
just fmt

# 執行測試
just test

# 檢查專案狀態
just status
```

### 提交前檢查

```bash
# 執行所有 pre-commit 檢查
just pre-commit

# 或個別執行
just check-fmt-rust  # 檢查 Rust 格式
just clippy          # 執行 Clippy
just test-rust       # 執行 Rust 測試
just lint-frontend   # 檢查前端程式碼
```

## Rust 專用指令

```bash
# 格式化 Rust 程式碼
just fmt-rust

# 檢查 Rust 格式（不修改）
just check-fmt-rust

# 執行 Clippy 檢查
just clippy

# 執行 Rust 測試
just test-rust

# 執行測試並顯示詳細輸出
just test-verbose

# 建置 Rust（debug）
just build-rust

# 建置 Rust（release）
just build-rust-release

# 清理 Rust 建置產物
just clean-rust
```

## 前端專用指令

```bash
# 建置前端
just build-frontend

# 啟動前端開發伺服器
just dev-frontend

# 檢查前端程式碼
just lint-frontend

# 格式化前端程式碼
just fmt-frontend
```

## 整合指令

```bash
# 格式化所有程式碼（Rust + 前端）
just fmt

# 執行所有檢查（格式、Clippy、測試、Lint）
just check

# 建置所有（前端 + 後端 debug）
just build

# 建置 release 版本（前端 + 後端 release）
just build-release

# 清理所有建置產物
just clean
```

## Tauri 專用指令

```bash
# 啟動 Tauri 開發模式
just dev

# 建置 Tauri 應用程式
just build-tauri
```

## CI/CD 指令

```bash
# CI 執行的完整檢查流程
just ci-check

# CI 建置流程
just ci-build

# Pre-commit 檢查（本地使用）
just pre-commit
```

## 安裝與設置

```bash
# 完整初始化（安裝依賴 + 建置）
just init

# 安裝系統依賴（Ubuntu/Debian）
just install-system-deps

# 安裝 Node.js 依賴
just install-node

# 更新所有依賴
just update-deps
```

## 工具指令

```bash
# 顯示專案狀態（git、Rust、Node 版本）
just status
```

## 指令組合範例

### 完整開發工作流程

```bash
# 1. 初始化專案
just init

# 2. 啟動開發模式
just dev

# 3. 修改程式碼...

# 4. 格式化程式碼
just fmt

# 5. 執行檢查
just check

# 6. 提交前最後檢查
just pre-commit

# 7. 建置 release 版本
just build-release
```

### 快速修復工作流程

```bash
# 修改程式碼...

# 格式化 + 檢查
just fmt
just check

# 如果測試失敗，查看詳細輸出
just test-verbose
```

### CI 本地模擬

```bash
# 模擬 CI 檢查
just ci-check

# 模擬 CI 建置
just ci-build
```

## 與 npm 指令對照表

| Just 指令 | npm 指令 | 說明 |
|----------|----------|------|
| `just dev` | `npm run tauri:dev` | 啟動開發模式 |
| `just build-frontend` | `npm run build` | 建置前端 |
| `just lint-frontend` | `npm run lint` | 檢查前端 |
| `just fmt-frontend` | `npm run fmt` | 格式化前端 |
| `just build-tauri` | `npm run tauri:build` | 建置 Tauri 應用 |

## 與 cargo 指令對照表

| Just 指令 | cargo 指令 | 說明 |
|----------|-----------|------|
| `just fmt-rust` | `cargo fmt --manifest-path=src-tauri/Cargo.toml --all` | 格式化 Rust |
| `just check-fmt-rust` | `cargo fmt --manifest-path=src-tauri/Cargo.toml --all -- --check` | 檢查格式 |
| `just clippy` | `cargo clippy --manifest-path=src-tauri/Cargo.toml --all-targets -- -D warnings` | Clippy 檢查 |
| `just test-rust` | `cargo test --manifest-path=src-tauri/Cargo.toml --all` | 執行測試 |
| `just build-rust` | `cargo build --manifest-path=src-tauri/Cargo.toml` | 建置 debug |
| `just build-rust-release` | `cargo build --manifest-path=src-tauri/Cargo.toml --release` | 建置 release |

## 提示

- 使用 `just --list` 隨時查看所有可用指令
- 所有指令都有中文註解，執行時會顯示正在做什麼
- Just 會自動處理相對路徑和工作目錄
- 指令失敗時會立即停止，不會繼續執行後續指令
- 可以在專案根目錄的任何子目錄中執行 `just` 指令

## 常見問題

### Q: Just 和 Make 有什麼不同？

A: Just 專注於指令執行，語法更簡單，跨平台支援更好，不需要處理 Make 的製表符（tab）問題。

### Q: 為什麼不直接用 npm scripts？

A: Just 可以管理 Rust 和前端指令，提供統一的介面，並且更適合複雜的建置流程。

### Q: CI 會用 Just 嗎？

A: 是的，CI 工作流程已經更新為使用 Just 指令，確保本地和 CI 環境一致。

### Q: 如果沒有安裝 Just 怎麼辦？

A: Pre-commit hook 會自動 fallback 到直接執行 cargo/npm 指令，不會影響開發。

## 更多資訊

- [Just 官方文檔](https://just.systems/)
- [Just GitHub](https://github.com/casey/just)
- [專案 justfile](./justfile) - 查看完整指令定義
