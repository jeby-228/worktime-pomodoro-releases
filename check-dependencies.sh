#!/bin/bash
# 確保系統依賴已安裝的腳本

echo "檢查並安裝 Tauri 所需的系統依賴..."

# 檢查 glib
if ! pkg-config --exists glib-2.0; then
    echo "安裝 glib-2.0..."
    sudo apt-get update
    sudo apt-get install -y libglib2.0-dev
else
    echo "✓ glib-2.0 已安裝 ($(pkg-config --modversion glib-2.0))"
fi

# 檢查 gobject
if ! pkg-config --exists gobject-2.0; then
    echo "安裝 gobject-2.0..."
    sudo apt-get install -y libglib2.0-dev
else
    echo "✓ gobject-2.0 已安裝 ($(pkg-config --modversion gobject-2.0))"
fi

# 檢查其他 Tauri 依賴
echo ""
echo "檢查其他 Tauri 依賴..."

dependencies=(
    "webkit2gtk-4.1:libwebkit2gtk-4.1-dev"
    "ayatana-appindicator3-0.1:libayatana-appindicator3-dev"
    "librsvg-2.0:librsvg2-dev"
    "libxdo:libxdo-dev"
    "openssl:libssl-dev"
)

missing=()

for dep in "${dependencies[@]}"; do
    IFS=: read -r pkg apt <<< "$dep"
    if pkg-config --exists "$pkg" 2>/dev/null; then
        echo "✓ $pkg 已安裝"
    else
        echo "✗ $pkg 缺失"
        missing+=("$apt")
    fi
done

if [ ${#missing[@]} -gt 0 ]; then
    echo ""
    echo "安裝缺失的依賴: ${missing[*]}"
    sudo apt-get install -y "${missing[@]}"
fi

echo ""
echo "✓ 所有依賴檢查完成！"
echo ""
echo "現在可以運行："
echo "  npm install"
echo "  npm run tauri:dev    # 開發模式"
echo "  npm run tauri:build  # 生產建置"
