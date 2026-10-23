# 建置指南 Build Guide

## ⚠️ 重要：建置順序

此專案需要**先建置前端**，再建置後端。Tauri 在編譯時需要 `dist/` 目錄存在。

## 💡 使用 Just 指令執行器

本專案使用 [Just](https://github.com/casey/just) 來管理常用指令，簡化開發流程。

### 安裝 Just

```bash
# 使用 Cargo 安裝
cargo install just

# 或使用系統套件管理器
# macOS
brew install just

# Ubuntu/Debian
wget -qO - 'https://proget.makedeb.org/debian-feeds/prebuilt-mpr.pub' | gpg --dearmor | sudo tee /usr/share/keyrings/prebuilt-mpr-archive-keyring.gpg 1> /dev/null
echo "deb [arch=all,$(dpkg --print-architecture) signed-by=/usr/share/keyrings/prebuilt-mpr-archive-keyring.gpg] https://proget.makedeb.org prebuilt-mpr $(lsb_release -cs)" | sudo tee /etc/apt/sources.list.d/prebuilt-mpr.list
sudo apt update
sudo apt install just

# Arch Linux
sudo pacman -S just
```

更多安裝方式請參考：https://github.com/casey/just#installation

### 正確的建置方式

#### 方式 1：使用 Just（推薦）

```bash
# 開發模式（自動建置前端 + 後端）
just dev

# 生產建置（自動建置前端 + 後端）
just build-release

# 或分步建置
just build-frontend  # 先建置前端
just build-rust      # 再建置後端
```

#### 方式 2：使用 npm 腳本

```bash
# 開發模式（自動建置前端 + 後端）
npm run tauri:dev

# 生產建置（自動建置前端 + 後端）
npm run tauri:build
```

這些指令會自動執行 `beforeDevCommand` / `beforeBuildCommand`，確保前端先建置完成。

#### 方式 3：手動分步建置

```bash
# 1. 先建置前端（生成 dist/ 目錄）
npm run build

# 2. 再建置後端
cargo build --manifest-path=src-tauri/Cargo.toml

# 或直接在 src-tauri/ 目錄內
cd src-tauri
cargo build
```

### ❌ 常見錯誤

**錯誤訊息：**

```
error: proc macro panicked
   --> src/main.rs:167:16
    |
167 |         .build(tauri::generate_context!())
    |                ^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = help: message: The `frontendDist` configuration is set to `"../dist"` but this path doesn't exist
```

**原因：**  
直接執行 `cargo build` 或 `npm run tauri:build` 時，`dist/` 目錄不存在。

**解決方法：**

```bash
# 先建置前端
npm run build

# 然後再建置
cargo build --manifest-path=src-tauri/Cargo.toml
```

---

## 📦 完整建置流程

### 初次設置

**使用 Just（最簡單）：**

```bash
# 一鍵完成所有設置（安裝依賴 + 建置）
just init
```

**手動設置：**

```bash
# 1. 安裝系統依賴（僅 Linux）
./check-dependencies.sh
# 或使用 just
just install-system-deps

# 2. 安裝 Node.js 依賴
npm install
# 或使用 just
just install-node

# 3. 建置前端靜態檔案
npm run build
# 或使用 just
just build-frontend

# 4. 驗證後端編譯
cargo build --manifest-path=src-tauri/Cargo.toml
# 或使用 just
just build-rust
```

### 開發流程

**使用 Just：**

```bash
# 啟動開發伺服器（熱重載）
just dev
```

**使用 npm：**

```bash
# 啟動開發伺服器（熱重載）
npm run tauri:dev
```

Tauri 會自動：

1. 執行 `npm run dev` 啟動 Astro 開發伺服器
2. 啟動 Rust 後端並連接到前端
3. 監聽檔案變更並自動重載

### 生產建置

**使用 Just：**

```bash
# 一鍵建置（推薦）
just build-release
```

**使用 npm：**

```bash
# 一鍵建置（推薦）
npm run tauri:build
```

Tauri 會自動：

1. 執行 `npm run build` 建置前端靜態檔案到 `dist/`
2. 編譯 Rust 後端（release 模式）
3. 打包為平台專屬安裝檔（.deb / .AppImage / .exe 等）

輸出位置：`src-tauri/target/release/bundle/`

---

## 🔧 進階：僅建置後端

如果你已經建置過前端，只想重新編譯 Rust 部分：

```bash
# 從專案根目錄
cargo build --manifest-path=src-tauri/Cargo.toml --release

# 或進入 src-tauri/ 目錄
cd src-tauri
cargo build --release
```

**前提：** `dist/` 目錄必須存在（至少執行過一次 `npm run build`）。

---

## 🎨 程式碼格式化與檢查

**使用 Just（推薦）：**

```bash
# 格式化所有程式碼（Rust + 前端）
just fmt

# 執行所有檢查（格式、Clippy、測試、Lint）
just check

# 僅檢查 Rust 格式（不修改）
just check-fmt-rust

# 格式化 Rust 程式碼
just fmt-rust

# 執行 Clippy 檢查
just clippy

# 執行測試
just test

# 前端檢查
just lint-frontend
just fmt-frontend
```

**手動執行：**

```bash
# 檢查 JavaScript/TypeScript 程式碼（不修改）
npm run lint

# 檢查並顯示建議的修改（不實際修改）
npm run format:check

# 自動修正可修復的問題
npm run fmt

# 檢查 Rust 程式碼格式
cd src-tauri
cargo fmt --check

# 自動格式化 Rust 程式碼
cargo fmt

# 執行 Clippy 檢查
cargo clippy -- -D warnings
```

---

## 📝 CI/CD 建置

本專案的 CI/CD 已整合 Just 指令。

GitHub Actions 工作流程範例：

```yaml
- name: Install Just
  uses: extractions/setup-just@v2

- name: Install Node dependencies
  run: npm install

- name: Install system dependencies
  run: just install-system-deps

- name: Check Rust formatting
  run: just check-fmt-rust

- name: Run Clippy
  run: just clippy

- name: Run tests
  run: just test-rust

- name: Build frontend
  run: just build-frontend

- name: Lint frontend
  run: just lint-frontend

- name: Build Tauri app
  run: npm run tauri:build
```

**使用 Just 的 CI 簡化版：**

```yaml
- name: Install Just
  uses: extractions/setup-just@v2

- name: Install dependencies
  run: |
    npm install
    just install-system-deps

- name: Run CI checks
  run: just ci-check

- name: Build
  run: just ci-build
```

---

## 🐛 疑難排解

### 問題：`dist/` 目錄被誤刪或清理

```bash
# 重新生成
npm run build
```

### 問題：`cargo clean` 後無法建置

```bash
# cargo clean 不會刪除 dist/，但如果你手動清理了專案：
npm run build
cargo build --manifest-path=src-tauri/Cargo.toml
```

### 問題：修改前端後 Tauri 沒有更新

```bash
# 重新建置前端
npm run build

# 重新啟動 Tauri
npm run tauri:dev
```

### 問題：Linux `.deb` / `.rpm` 套件名稱不合法

Debian / RPM 套件名稱必須以英數字開頭。本專案 `productName` 為 `what are you doing`，打包後檔名會類似：

- `what-are-you-doing_0.1.0_amd64.deb`
- `what-are-you-doing-0.1.0-1.x86_64.rpm`

### 問題：Astro 7.x 升級後出現錯誤

目前專案使用 Astro 7.4.2。如果遇到相容性問題：

```bash
# 清除快取並重新建置
rm -rf node_modules/.astro dist
npm install
npm run build
```

---

## 📚 相關文件

- [README.md](./README.md) - 專案概覽與功能說明
- [CI_CD.md](./CI_CD.md) - CI/CD 設定說明
- [UPDATER_SETUP.md](./UPDATER_SETUP.md) - 更新系統設定指南
- [check-dependencies.sh](./check-dependencies.sh) - 系統依賴檢查腳本
