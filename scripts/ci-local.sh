#!/bin/bash
# 本地 CI 測試腳本 - 模擬 GitHub Actions CI 流程

set -e

echo "🚀 開始本地 CI 檢查..."
echo ""

# 顏色定義
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 成功/失敗計數
PASSED=0
FAILED=0

run_check() {
    local name=$1
    local command=$2
    
    echo -e "${YELLOW}▶ $name${NC}"
    if eval "$command"; then
        echo -e "${GREEN}✅ $name - PASSED${NC}"
        PASSED=$((PASSED + 1))
    else
        echo -e "${RED}❌ $name - FAILED${NC}"
        FAILED=$((FAILED + 1))
        return 1
    fi
    echo ""
}

echo "═══════════════════════════════════════"
echo "  Rust Check & Test"
echo "═══════════════════════════════════════"
echo ""

run_check "Rust 格式檢查" \
    "cargo fmt --manifest-path=src-tauri/Cargo.toml --all -- --check"

run_check "Clippy 靜態分析" \
    "cargo clippy --manifest-path=src-tauri/Cargo.toml --all-targets -- -D warnings"

run_check "Rust 單元測試" \
    "cargo test --manifest-path=src-tauri/Cargo.toml --lib"

run_check "Rust Release 建置" \
    "cargo build --manifest-path=src-tauri/Cargo.toml --release"

echo "═══════════════════════════════════════"
echo "  Frontend Lint & Build"
echo "═══════════════════════════════════════"
echo ""

run_check "前端 Lint (oxlint)" \
    "npm run lint"

run_check "前端格式檢查 (oxlint)" \
    "npm run format:check"

run_check "前端建置 (Astro)" \
    "npm run build"

echo "═══════════════════════════════════════"
echo "  Summary"
echo "═══════════════════════════════════════"
echo ""
echo -e "${GREEN}✅ Passed: $PASSED${NC}"
echo -e "${RED}❌ Failed: $FAILED${NC}"
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}🎉 所有檢查通過！可以安全提交。${NC}"
    exit 0
else
    echo -e "${RED}💥 有 $FAILED 項檢查失敗，請修復後再提交。${NC}"
    exit 1
fi
