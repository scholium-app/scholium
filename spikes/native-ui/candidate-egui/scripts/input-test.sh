#!/usr/bin/env bash
# egui 候选输入注入测试：验证键盘与输入法（预编辑 / 提交）是否到达应用。
#
# 用法：spikes/native-ui/candidate-egui/scripts/input-test.sh
# 安全约束：每次注入前复核焦点在本应用窗口，焦点不符立即中止。
# 依赖：ydotool + 运行中的 ydotoold。

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/debug/scholium-spike-egui"
LOG="/tmp/egui-input-test.log"
SHOT_DIR="$ROOT/artifacts"

[ -x "$BIN" ] || { echo "FAIL: 未构建 $BIN"; exit 1; }
command -v ydotool >/dev/null || { echo "FAIL: 未安装 ydotool"; exit 1; }
systemctl --user is-active --quiet ydotool || { echo "FAIL: ydotoold 未运行"; exit 1; }

window_id() {
    niri msg --json windows 2>/dev/null | python3 -c '
import json, sys
target = int(sys.argv[1])
for window in json.load(sys.stdin):
    if window.get("pid") == target or "Scholium spike" in (window.get("title") or ""):
        print(window["id"]); break
' "$1"
}

focused_id() {
    niri msg --json windows 2>/dev/null | python3 -c '
import json, sys
for window in json.load(sys.stdin):
    if window.get("is_focused"):
        print(window["id"]); break
'
}

ensure_focus() {
    local want="$1" attempt got=""
    for attempt in 1 2 3 4 5; do
        niri msg action focus-window --id "$want" >/dev/null 2>&1
        sleep 0.4
        got="$(focused_id)"
        [ "$got" = "$want" ] && return 0
    done
    echo "ABORT: 焦点不在本应用（want=$want got=${got:-none}），停止注入"
    return 1
}

"$BIN" >"$LOG" 2>&1 &
PID=$!
sleep 6

WID="$(window_id "$PID")"
if [ -z "$WID" ]; then
    echo "FAIL: 未找到应用窗口"
    kill "$PID" 2>/dev/null
    exit 1
fi
echo "PASS: 应用窗口 id=$WID"

ensure_focus "$WID" || { kill "$PID" 2>/dev/null; exit 1; }
echo "--- 注入拼音：nihao + 空格 + 回车 ---"
ydotool type "nihao"
sleep 1
ydotool key 57:1 57:0   # KEY_SPACE
sleep 1
ydotool key 28:1 28:0   # KEY_ENTER
sleep 1

ensure_focus "$WID" || { kill "$PID" 2>/dev/null; exit 1; }
echo "--- 注入方向键（结构导航） ---"
ydotool key 108:1 108:0  # KEY_DOWN
sleep 0.5

echo
echo "--- 应用 stderr ---"
grep -vE '^MESA|^libEGL' "$LOG" | tail -10

niri msg action screenshot-window --id "$WID" -d true -p false >/dev/null 2>&1
sleep 2
NEWEST="$(find "$HOME/Pictures/Screenshots" -name '*.png' -newermt '-2 minutes' \
    -printf '%T@ %p\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2-)"
if [ -n "$NEWEST" ]; then
    cp "$NEWEST" "$SHOT_DIR/egui-ime-test.png"
    echo "PASS: 截图 $SHOT_DIR/egui-ime-test.png"
fi

kill "$PID" 2>/dev/null
sleep 1
kill -0 "$PID" 2>/dev/null && kill -9 "$PID" 2>/dev/null
echo "已结束应用"
