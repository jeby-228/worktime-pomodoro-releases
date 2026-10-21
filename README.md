# what are you doing

離線優先的桌面工時記錄工具，基於 Tauri 2 + Rust + Astro 打造。

## 功能特色

- ✅ **離線優先**：所有數據儲存在本機 SQLite，支援 WAL 模式
- 🍅 **番茄鐘設計**：定時提醒（預設 25 分鐘）確認工作任務
- 🔄 **自動恢復**：支援崩潰恢復、過夜關機恢復、休眠/鎖定檢測
- 📊 **統計報表**：每日工時統計、CSV 與 Excel（.xlsx）匯出
- 🎯 **系統整合**：系統匣圖示、全域快捷鍵（Ctrl+Alt+T）、開機自啟動
- 🔐 **資料庫遷移**：內建 SQL 遷移系統，自動備份，保護資料安全
- 🔄 **自動更新**：支援簽名更新套件（配置後啟用）

## 技術架構

### 後端（Rust）
- **Tauri 2.x**：跨平台桌面應用框架
- **SQLite + rusqlite**：本地資料庫，WAL 模式，bundled 靜態連結
- **事件溯源**：append-only `events` 表作為 source of truth
- **狀態機**：Idle → Working ⇄ Paused 狀態管理
- **tokio**：非同步執行環境

### 前端（Astro）
- **Astro 7.x**：靜態網站生成器，輸出純 HTML/JS
- **TypeScript**：型別安全的前端邏輯
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
├── src/                    # Astro 前端
│   └── pages/              # 頁面路由
│       ├── index.astro     # 主頁（應用程式說明）
│       ├── ping.astro      # 任務記錄頁
│       ├── stats.astro     # 統計頁
│       ├── resolve.astro   # 待確認時段
│       └── settings.astro  # 設定頁
├── src-tauri/              # Rust 後端
│   ├── src/
│   │   ├── lib.rs          # 核心函式庫入口
│   │   ├── main.rs         # Tauri 應用程式入口
│   │   ├── commands.rs     # Tauri commands（IPC）
│   │   ├── scheduler.rs    # 番茄鐘排程器
│   │   ├── updater.rs      # 更新系統
│   │   ├── store.rs        # SQLite 資料存取層
│   │   ├── state.rs        # 狀態機實作
│   │   ├── derive.rs       # 片段衍生器
│   │   ├── recovery.rs     # 恢復邏輯
│   │   ├── sync.rs         # 同步介面（未完成）
│   │   └── migrations.rs   # 資料庫遷移
│   ├── migrations/         # SQL 遷移檔案
│   │   └── 0001_init.sql
│   └── tests/              # 整合測試
└── dist/                   # 建置輸出（Astro 靜態檔案）
```

## 核心模組說明

### M1：Rust 核心函式庫
- **store.rs**：SQLite WAL 模式、事件與片段資料模型
- **state.rs**：`Idle / Working / Paused` 狀態機，處理 start/end/pause/ping 操作
- **derive.rs**：從事件重建不重疊的工作片段，計算每日統計
- **migrations.rs**：嵌入式 SQL 遷移，自動備份（保留最近 5 份），拒絕降級

### M2：Tauri 2 整合
- **main.rs**：系統匣、視窗管理、插件初始化（通知、快捷鍵、自啟動、單例）
- **scheduler.rs**：每 N 分鐘觸發 ping，顯示 `/ping` 視窗並發送通知
- **commands.rs**：Tauri IPC 指令（start_task, end_task, get_stats, etc.）

### M3：恢復機制
- **recovery.rs**：
  - 啟動時檢查崩潰（結束於最後 ping + 標記未知時段至今）
  - 過夜關機恢復（結束於下班時間）
  - 休眠/喚醒檢測（Linux: zbus/D-Bus，Windows: 存根）
  - 時鐘跳躍檢測（單調時鐘 vs 掛鐘時間）

### M4：Astro 前端頁面
- **/ping**：記錄當前任務、切換任務、暫停/繼續
- **/stats**：今日工時統計、任務分佈、CSV 匯出
- **/resolve**：待確認時段列表與解決
- **/settings**：使用者名稱、間隔時間、工作時間範圍、同步/更新網址

## 資料庫遷移

### 遷移系統
- 使用 `rusqlite_migration` + `PRAGMA user_version`
- 遷移檔案存放於 `src-tauri/migrations/XXXX_name.sql`
- 每次遷移前自動備份資料庫（格式：`worktime.db.backup_YYYYMMDD_HHMMSS`）
- 保留最近 5 份備份檔案
- **拒絕降級**：當資料庫版本高於程式版本時拒絕啟動

### 新增遷移步驟
1. 在 `src-tauri/migrations/` 新增 `XXXX_description.sql`
2. 更新 `src-tauri/src/migrations.rs` 的 `MigrationRunner::new()` 中的陣列
3. 更新 `latest_version()` 回傳值
4. 運行測試：`cargo test migrations::tests`

## 自動更新系統

### 功能
- 基於 `tauri-plugin-updater`（Tauri 2.x 內建）
- 支援自訂更新伺服器網址（在設定頁面配置）
- 更新前自動：
  1. 暫停當前任務
  2. 執行 `PRAGMA wal_checkpoint(TRUNCATE)`
  3. 下載並安裝更新
  4. 重啟應用程式

### 簽名設定（生產環境）
1. **生成簽名金鑰對**：
   ```bash
   npm run tauri signer generate -- -w ~/.tauri/myapp.key
   ```
   此指令會生成私鑰（`~/.tauri/myapp.key`）和公鑰（顯示於終端）

2. **配置公鑰**：
   在 `src-tauri/tauri.conf.json` 的 `updater.pubkey` 填入公鑰

3. **建置簽名套件**：
   ```bash
   export TAURI_SIGNING_PRIVATE_KEY=$(cat ~/.tauri/myapp.key)
   npm run tauri:build
   ```

4. **發佈 latest.json**：
   建置後產生的 `latest.json` 與更新套件（.AppImage / .exe 等）需放到更新伺服器上

5. **設定更新網址**：
   在應用程式的設定頁面填入更新伺服器網址（例如：`https://example.com/updates/`）

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
# 運行所有測試（單元 + 整合）
cd src-tauri
cargo test

# 運行特定測試模組
cargo test migrations::tests
cargo test recovery::tests
```

### 測試涵蓋範圍
- ✅ 狀態機轉換（idle/working/paused）
- ✅ 事件插入與查詢
- ✅ 片段衍生與重建
- ✅ 資料庫遷移與備份
- ✅ 恢復場景（崩潰、過夜、時鐘跳躍）
- ⚠️ 部分測試需要修正（23/26 通過）

## 快捷鍵

- **Ctrl+Alt+T**：開啟任務記錄視窗（全域快捷鍵）

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

## 已知限制與待辦

- [ ] Recovery 整合至 Tauri 啟動流程（✅ 已完成）
- [ ] 完善 Linux 休眠/喚醒檢測（✅ 已完成 - D-Bus PrepareForSleep）
- [ ] Windows 休眠檢測（架構完成，需實作 unsafe Win32 API）
- [ ] 完整的同步功能（`SyncTarget` trait 已定義，實作留給後續）
- [ ] 完善 Tauri updater 整合（✅ 已完成 - 參考 UPDATER_SETUP.md）
- [ ] Bundle identifier 已更新為 `com.worktime.pomodoro`

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

（待定義授權條款）

## 開發者

由 Cursor AI Agent 協助開發。

---

**開發狀態**：M1-M4 核心功能已完成，可進行本地開發與測試。生產部署需額外配置簽名金鑰與更新伺服器。
