# Worktime Pomodoro - Just Commands
# 使用方式：just <command>
# 列出所有指令：just --list

# 預設指令（執行所有檢查）
default: check

# Rust 相關指令
# ============

# 格式化 Rust 程式碼
fmt-rust:
    cargo fmt --manifest-path=src-tauri/Cargo.toml --all

# 檢查 Rust 格式（不修改）
check-fmt-rust:
    cargo fmt --manifest-path=src-tauri/Cargo.toml --all -- --check

# 執行 Clippy 檢查
clippy:
    cargo clippy --manifest-path=src-tauri/Cargo.toml --all-targets -- -D warnings

# 執行 Rust 測試
test-rust:
    cargo test --manifest-path=src-tauri/Cargo.toml --all

# 建置 Rust（debug 模式）
build-rust:
    cargo build --manifest-path=src-tauri/Cargo.toml

# 建置 Rust（release 模式）
build-rust-release:
    cargo build --manifest-path=src-tauri/Cargo.toml --release

# 清理 Rust 建置產物
clean-rust:
    cargo clean --manifest-path=src-tauri/Cargo.toml

# 前端相關指令
# ============

# 格式化前端程式碼
fmt-frontend:
    npm run fmt

# 檢查前端程式碼
lint-frontend:
    npm run lint

# 建置前端
build-frontend:
    npm run build

# 啟動前端開發伺服器
dev-frontend:
    npm run dev

# 整合指令
# ============

# 格式化所有程式碼
fmt: fmt-rust fmt-frontend

# 執行所有檢查（CI 使用）
check: check-fmt-rust clippy test-rust lint-frontend

# 建置所有（前端 + 後端）
build: build-frontend build-rust

# 建置 release 版本
build-release: build-frontend build-rust-release

# 清理所有建置產物
clean: clean-rust
    rm -rf dist/
    rm -rf node_modules/.astro

# Tauri 相關指令
# ============

# 啟動 Tauri 開發模式
dev:
    npm run tauri:dev

# 建置 Tauri 應用程式
build-tauri:
    npm run tauri:build

# CI/CD 相關指令
# ============

# CI 執行的完整檢查流程
ci-check: check-fmt-rust clippy test-rust lint-frontend build-frontend

# CI 建置流程
ci-build: build-frontend
    cargo build --manifest-path=src-tauri/Cargo.toml --release

# 安裝依賴
# ============

# 安裝 Node.js 依賴
install-node:
    npm install

# 安裝系統依賴（Ubuntu/Debian）
install-system-deps:
    sudo apt-get update
    sudo apt-get install -y \
        libwebkit2gtk-4.1-dev \
        libayatana-appindicator3-dev \
        librsvg2-dev \
        libxdo-dev \
        libssl-dev \
        libglib2.0-dev

# 完整初始化（首次設置）
init: install-system-deps install-node build-frontend build-rust
    @echo "✅ 專案初始化完成！"
    @echo "執行 'just dev' 啟動開發模式"

# 測試相關
# ============

# 執行所有測試
test: test-rust
    @echo "✅ 所有測試通過"

# 執行測試並顯示輸出
test-verbose:
    cargo test --manifest-path=src-tauri/Cargo.toml --all -- --nocapture

# 工具指令
# ============

# 顯示專案狀態
status:
    @echo "=== Git Status ==="
    @git status --short
    @echo ""
    @echo "=== Rust Version ==="
    @cargo --version
    @echo ""
    @echo "=== Node Version ==="
    @node --version
    @echo ""
    @echo "=== npm Version ==="
    @npm --version

# 執行 pre-commit 檢查
pre-commit: check-fmt-rust clippy test-rust lint-frontend
    @echo "✅ Pre-commit 檢查通過"

# 更新依賴
update-deps:
    cargo update --manifest-path=src-tauri/Cargo.toml
    npm update
    @echo "✅ 依賴已更新"
