#!/usr/bin/env bash
# Iced 候选冒烟测试：启动真实窗口、由合成器确认窗口存在、按窗口 ID 截图、干净退出。
#
# 用法：spikes/native-ui/candidate-iced/scripts/smoke.sh [backend]
#   backend 默认 tiny-skia；本机 wgpu 路径存在 ZINK/Vulkan 初始化失败，见验证报告。
#
# 只截取本应用自己的窗口，不采集屏幕上其他内容。

set -uo pipefail

BACKEND="${1:-tiny-skia}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/debug/scholium-spike-iced"
LOG="/tmp/iced-smoke-${BACKEND}.log"
SHOT_DIR="$ROOT/artifacts"

if [ ! -x "$BIN" ]; then
    echo "FAIL: 未找到可执行文件 $BIN；先构建候选。"
    exit 1
fi

mkdir -p "$SHOT_DIR"

# "default" 表示不设置 ICED_BACKEND，观察候选的默认后端选择。
if [ "$BACKEND" = "default" ]; then
    "$BIN" >"$LOG" 2>&1 &
else
    ICED_BACKEND="$BACKEND" "$BIN" >"$LOG" 2>&1 &
fi
PID=$!
sleep 7

if ! kill -0 "$PID" 2>/dev/null; then
    echo "FAIL: 进程提前退出（backend=$BACKEND）"
    cat "$LOG"
    exit 1
fi
echo "PASS: 进程存活 pid=$PID backend=$BACKEND"

WID="$(niri msg --json windows 2>/dev/null | python3 -c '
import json, sys
for window in json.load(sys.stdin):
    if "Scholium spike" in window.get("title", ""):
        print(window["id"])
        break
')"

if [ -z "$WID" ]; then
    echo "FAIL: 合成器未列出应用窗口"
    cat "$LOG"
    kill "$PID" 2>/dev/null
    exit 1
fi
echo "PASS: 合成器已列出窗口 id=$WID"
echo "INFO: stderr 中 error 行数 = $(grep -ci error "$LOG")"

if command -v niri >/dev/null 2>&1; then
    niri msg action screenshot-window --id "$WID" -d true -p false >/dev/null 2>&1
    sleep 2
    # niri 按 screenshot-path 配置写入，可能是子目录，因此递归查找最近 2 分钟内生成的文件。
    NEWEST="$(find "$HOME/Pictures/Screenshots" -name '*.png' -newermt '-2 minutes' \
        -printf '%T@ %p\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2-)"
    if [ -n "$NEWEST" ]; then
        cp "$NEWEST" "$SHOT_DIR/iced-window-${BACKEND}.png"
        echo "PASS: 已保存窗口截图 $SHOT_DIR/iced-window-${BACKEND}.png"
    else
        echo "WARN: 未取得截图（检查 niri 的 screenshot-path 配置）"
    fi
fi

kill "$PID" 2>/dev/null
sleep 1
if kill -0 "$PID" 2>/dev/null; then
    kill -9 "$PID" 2>/dev/null
    echo "INFO: 强制终止"
else
    echo "PASS: 收到 SIGTERM 后干净退出"
fi
