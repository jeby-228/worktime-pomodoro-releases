# what are you doing

離線優先的桌面工時記錄工具，基於 Tauri 2 + Rust + Astro 打造。

## 功能特色

- ✅ **離線優先**：工時存在本機 SQLite（`worktime_v2.db`），支援 WAL 模式
- 📅 **今日與紀錄**：只顯示當天開始的時段；較早的紀錄留在紀錄頁
- ✏️ **補登、分割、合併**：同一段空白可補多筆，時段可從中間切開或合併相鄰紀錄
- 📊 **匯出**：依目前語系把工作類型寫進 CSV 或 Excel（.xlsx）
- 🎯 **桌面整合**：系統匣、桌寵、迷你列、開機自啟動
- 🔄 **更新**：從 GitHub Release 下載安裝檔，並用 `SHA256SUMS` 核對

## 技術架構

### 後端（Rust）

- **Tauri 2.x**：跨平台桌面應用框架
- **SQLite + rusqlite**：本地資料庫，WAL 模式，bundled 靜態連結
- **時段模型**：工作、插隊、會議、午休存在 `slots` 表
- **tokio**：非同步執行環境

### 前端（Astro + Svelte）

- **Astro**：頁面路由
- **Svelte 5**：今日、紀錄、設定與對話框
- **@tauri-apps/api**：與 Rust 後端通訊

## 快速開始

> **⚠️ 重要建置須知：** 此專案需要先建置前端（`npm run build`）才能編譯後端。  
> 詳細說明請參閱 **[BUILD_GUIDE.md](./BUILD_GUIDE.md)**

> **💡 推薦工具：** 本專案使用 [Just](https://github.com/casey/just) 來管理常用指令。  
> 安裝：`cargo install just` 或參考 [安裝指南](https://github.com/casey/just#installation)  
> 查看所有指令：`just --list`

### 前置需求

- **Rust** 1.70+（推薦最新穩定版）
- **Node.js** 22.23.4+
- **系統依賴**（Linux）：
  ```bash
  # 自動檢查並安裝依賴
  ./check-dependencies.sh

  # 或手動安裝
  sudo apt update
  sudo apt install -y \
    libwebkit2gtk-4.1-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libxdo-dev \
    libssl-dev \
    libglib2.0-dev
  ```

### 初次設置

**使用 Just（推薦）：**

```bash
# 完整初始化（安裝依賴、建置前端和後端）
just init
```

**手動設置：**

```bash
# 1. 安裝系統依賴（僅 Linux）
./check-dependencies.sh
# 或使用 just
just install-system-deps

# 2. 安裝 Node 依賴
npm install

# 3. 建置前端（必須先執行！）
npm run build
# 或使用 just
just build-frontend

# 4. 驗證後端編譯
cargo build --manifest-path=src-tauri/Cargo.toml
# 或使用 just
just build-rust
```

### 開發模式

```bash
# 啟動開發伺服器（Tauri + Astro 熱重載）
npm run tauri:dev

# 或使用 just
just dev
```

### 生產建置

```bash
# 一鍵建置（推薦，自動建置前端 + 後端）
npm run tauri:build

# 或使用 just 分步建置
just build-release  # 建置前端 + 後端（release 模式）
```

輸出檔案位於 `src-tauri/target/release/bundle/`

> 💡 **提示：** `npm run tauri:build` 會自動執行 `npm run build`（透過 `beforeBuildCommand`），  
> 無需手動分步建置。詳見 [BUILD_GUIDE.md](./BUILD_GUIDE.md)

## 專案結構

```
worktime-pomodoro/
├── src/pages/              # index、today、records、settings、pet、minibar
├── src/components/         # 今日時間軸、紀錄、設定、補登與編輯
├── src-tauri/src/
│   ├── main.rs             # 視窗、系統匣、插件
│   ├── commands_v2.rs      # 時段、匯出、更新指令
│   ├── store_v2.rs         # SQLite 存取
│   ├── state_v2.rs         # 目前計時狀態
│   ├── github_update.rs    # GitHub Release 更新
│   └── migrations.rs       # 0001 初始表、0002 時段表
└── .github/workflows/release.yml
```

## 核心模組

- **store_v2.rs**：時段、空白、補登、分割、合併
- **commands_v2.rs**：前端呼叫的指令，含 CSV / Excel 匯出
- **migrations.rs**：嵌入式 SQL 遷移，自動備份（保留最近 5 份），拒絕降級
- **github_update.rs**：讀取最新 Release，下載前核對 `SHA256SUMS`
- **今日 / 紀錄 / 設定**：當天紀錄、歷史紀錄與匯出、語言和更新

## 資料庫遷移

遷移檔在 `src-tauri/migrations/`。新增時把 SQL 加進 `MigrationManager::new()`，並讓 `latest_version()` 等於遷移數量。可用 `cargo test migrations::tests` 檢查。

## 自動更新

設定頁會向 `jeby-228/worktime-pomodoro-releases` 查最新 Release。安裝檔名稱必須和 `src-tauri/tauri.conf.json` 的版本一致，而且 `SHA256SUMS` 要包含該檔案。推上 `v*` 標籤後，由 `.github/workflows/release.yml` 建置並發布。

## 測試

**使用 Just（推薦）：**

```bash
# 運行所有測試
just test

# 運行測試並顯示詳細輸出
just test-verbose

# 僅運行 Rust 測試
just test-rust
```

**手動執行：**

```bash
cd src-tauri
cargo test

cargo test migrations::tests
```

## 快捷鍵

- **Ctrl+Alt+T**：快速輸入
- **Ctrl+Alt+I**：插隊

## Just 常用指令

本專案使用 [Just](https://github.com/casey/just) 作為指令執行器（類似 Make）。

```bash
# 列出所有可用指令
just --list

# 開發相關
just dev                  # 啟動 Tauri 開發模式
just build                # 建置前端 + 後端（debug）
just build-release        # 建置 release 版本
just clean                # 清理所有建置產物

# 程式碼品質
just fmt                  # 格式化所有程式碼
just check                # 執行所有檢查（CI 使用）
just clippy               # 執行 Clippy 檢查
just test                 # 執行所有測試

# Rust 專用
just fmt-rust             # 格式化 Rust 程式碼
just check-fmt-rust       # 檢查 Rust 格式
just test-rust            # 執行 Rust 測試
just build-rust           # 建置 Rust（debug）
just build-rust-release   # 建置 Rust（release）

# 前端專用
just fmt-frontend         # 格式化前端程式碼
just lint-frontend        # 檢查前端程式碼
just build-frontend       # 建置前端
just dev-frontend         # 啟動前端開發伺服器

# 初始化與設置
just init                 # 完整初始化專案
just install-system-deps  # 安裝系統依賴（Linux）
just install-node         # 安裝 Node 依賴
just status               # 顯示專案狀態

# CI/CD
just ci-check             # CI 檢查流程
just ci-build             # CI 建置流程
just pre-commit           # Pre-commit 檢查
```

詳細指令說明請參考 [`justfile`](./justfile)

## 常見問題

### 建置錯誤：`frontendDist` path doesn't exist

**錯誤訊息：**

```
error: proc macro panicked
The `frontendDist` configuration is set to `"../dist"` but this path doesn't exist
```

**原因：** 直接執行 `cargo build` 時 `dist/` 目錄不存在。

**解決方法：**

```bash
# 先建置前端
npm run build

# 再建置後端
cargo build --manifest-path=src-tauri/Cargo.toml
```

或直接使用：

```bash
npm run tauri:build  # 自動處理建置順序
```

詳見 **[BUILD_GUIDE.md](./BUILD_GUIDE.md)**

### 建置錯誤：缺少 glib-2.0 或 gobject-2.0

```bash
# 執行依賴檢查腳本
./check-dependencies.sh

# 或手動安裝
sudo apt-get install -y libglib2.0-dev
```

### 打包錯誤：Linux 套件名稱含中文

Linux 的 `.deb` / `.rpm` 套件名稱只能用英數字、連字號與底線。本專案產品名稱是 `what are you doing`，打包後的套件名為 `what-are-you-doing`。

### Bundle identifier 錯誤

## 授權

本專案採用 **MIT License** 授權 - 詳見 [LICENSE](LICENSE) 文件。

Copyright (c) 2026 jeby-228

### 圖標版權說明

**重要提示**：應用程式圖標（`app-icon.png` 及衍生圖標）為粉絲藝術作品，**不適用** MIT 授權。  
該圖標僅供個人使用，不得用於商業用途或再分發。

## 開發者

由 Cursor AI Agent 協助開發。

---

**開發狀態**：目前版本見 `src-tauri/tauri.conf.json`。發布時在 GitHub 推上對應的 `v*` 標籤。
