#!/usr/bin/env bash
# GPUI 候选冒烟测试：启动真实窗口、由合成器确认窗口存在、按窗口 ID 截图、干净退出。
#
# 用法：spikes/native-ui/candidate-gpui/scripts/smoke.sh
# 只截取本应用自己的窗口，不采集屏幕上其他内容。

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/debug/scholium-spike-egui"
LOG="/tmp/egui-smoke.log"
SHOT_DIR="$ROOT/artifacts"

if [ ! -x "$BIN" ]; then
    echo "FAIL: 未找到可执行文件 $BIN；先构建候选。"
    exit 1
fi

mkdir -p "$SHOT_DIR"

"$BIN" >"$LOG" 2>&1 &
PID=$!
sleep 7

if ! kill -0 "$PID" 2>/dev/null; then
    echo "FAIL: 进程提前退出"
    cat "$LOG"
    exit 1
fi
echo "PASS: 进程存活 pid=$PID"

WID="$(niri msg --json windows 2>/dev/null | python3 -c '
import json, sys
target = int(sys.argv[1])
for window in json.load(sys.stdin):
    title = window.get("title") or ""
    if "Scholium spike" in title or window.get("pid") == target:
        print(window["id"])
        break
' "$PID")"

if [ -z "$WID" ]; then
    echo "FAIL: 合成器未列出应用窗口（按标题或 pid=$PID 都未匹配）"
    niri msg windows 2>/dev/null | grep -E 'Window ID|Title|PID' | head -20
    cat "$LOG"
    kill "$PID" 2>/dev/null
    exit 1
fi
echo "PASS: 合成器已列出窗口 id=$WID"
echo "INFO: stderr 中 error 行数 = $(grep -ci error "$LOG" | head -1)"

if command -v niri >/dev/null 2>&1; then
    niri msg action screenshot-window --id "$WID" -d true -p false >/dev/null 2>&1
    sleep 2
    NEWEST="$(find "$HOME/Pictures/Screenshots" -name '*.png' -newermt '-2 minutes' \
        -printf '%T@ %p\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2-)"
    if [ -n "$NEWEST" ]; then
        cp "$NEWEST" "$SHOT_DIR/egui-window.png"
        echo "PASS: 已保存窗口截图 $SHOT_DIR/egui-window.png"
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
