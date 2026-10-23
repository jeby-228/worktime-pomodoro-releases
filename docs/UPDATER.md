# Tauri Updater Setup Guide

## 概述

本應用使用 Tauri 的自動更新功能，通過公開的 GitHub releases 倉庫分發更新。

## 密鑰生成（僅需執行一次）

```bash
# 生成密鑰對
npx @tauri-apps/cli signer generate -w ~/.tauri/worktime-pomodoro.key

# 查看公鑰
cat ~/.tauri/worktime-pomodoro.key.pub
```

將公鑰添加到 `src-tauri/tauri.conf.json` 的 `plugins.updater.pubkey` 欄位。

**⚠️ 重要：**

- **公鑰（.pub）**：提交到 git，放在 `tauri.conf.json` 中
- **私鑰（.key）**：**絕對不要提交**，只通過環境變數傳遞

## 本地構建（帶簽名）

```bash
# 設置私鑰環境變數
export TAURI_SIGNING_PRIVATE_KEY=$(cat ~/.tauri/worktime-pomodoro.key)
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""  # 如果設置了密碼

# 構建（自動簽名）
npm run tauri:build
```

## CI/CD 構建

在 GitHub Actions secrets 中添加：

- `TAURI_SIGNING_PRIVATE_KEY`：私鑰內容
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：密碼（可選）

```yaml
- name: Build with signing
  env:
    TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
  run: npm run tauri:build
```

## 發布更新

### 1. 構建並簽名

```bash
npm run tauri:build
```

這會在 `src-tauri/target/release/bundle/` 生成：

- Windows: `.msi`, `.msi.zip`, `.msi.zip.sig`
- Linux: `.deb`, `.AppImage`, `.AppImage.tar.gz`, `.AppImage.tar.gz.sig`

### 2. 生成 `latest.json`

```bash
# 使用附帶的腳本
node scripts/generate-update-manifest.mjs
```

或手動創建 `latest.json`：

```json
{
  "version": "0.1.0",
  "notes": "Version 0.1.0 release notes",
  "pub_date": "2026-10-22T18:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "<.msi.zip.sig 內容>",
      "url": "https://github.com/jeby-228/worktime-pomodoro-releases/releases/download/v0.1.0/worktime-pomodoro_0.1.0_x64_en-US.msi.zip"
    },
    "linux-x86_64": {
      "signature": "<.AppImage.tar.gz.sig 內容>",
      "url": "https://github.com/jeby-228/worktime-pomodoro-releases/releases/download/v0.1.0/worktime-pomodoro_0.1.0_amd64.AppImage.tar.gz"
    }
  }
}
```

### 3. 上傳到 GitHub Releases

上傳以下檔案到 `https://github.com/jeby-228/worktime-pomodoro-releases/releases`：

1. 安裝包：`.msi.zip`, `.AppImage.tar.gz`
2. 簽名檔：`.msi.zip.sig`, `.AppImage.tar.gz.sig`
3. 更新清單：`latest.json`

## 更新端點

應用配置的更新端點：

```
https://github.com/jeby-228/worktime-pomodoro-releases/releases/latest/download/latest.json
```

## 離線處理

- 應用會定期檢查更新（可在設定中關閉）
- 如果網路不可達，靜默失敗，不中斷使用
- 公司內網限制時，更新功能自動跳過

## 用戶操作

1. 設定頁面有「檢查更新」按鈕
2. 發現新版本時，顯示版本號和更新說明
3. 用戶確認後下載並安裝
4. 安裝完成後提示重啟應用

## 測試更新

```bash
# 修改版本號觸發更新檢測
# src-tauri/tauri.conf.json: "version": "0.2.0"

# 構建新版本
npm run tauri:build

# 生成 latest.json 並上傳到 releases

# 在舊版本應用中點擊「檢查更新」
```

## 安全說明

- 所有更新包都經過 Ed25519 簽名驗證
- 只接受匹配公鑰的簽名
- 下載過程使用 HTTPS
- 私鑰永遠不離開開發者機器/CI secrets
