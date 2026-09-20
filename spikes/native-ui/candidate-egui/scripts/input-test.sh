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
sleep 8

WID="$(window_id "$PID")"
if [ -z "$WID" ]; then
    echo "FAIL: 未找到应用窗口"
    kill "$PID" 2>/dev/null
    exit 1
fi
echo "PASS: 应用窗口 id=$WID"

inject_pinyin() {
    ydotool type "nihao"
    sleep 1
    ydotool key 57:1 57:0   # KEY_SPACE：选字
    sleep 0.6
    ydotool key 28:1 28:0   # KEY_ENTER：提交
    sleep 1
}

# 输入法是否真的介入了：看应用是否报告过预编辑事件。
ime_engaged() {
    grep -q '预编辑「' "$LOG"
}

ensure_focus "$WID" || { kill "$PID" 2>/dev/null; exit 1; }
echo "--- 注入拼音：nihao + 空格 + 回车 ---"
inject_pinyin

# fcitx5 对新建窗口的激活可能滞后；首次没合成时等待后重试一次，
# 不要一次没触发就断言"没有输入法"（这正是本脚本早先的错）。
if ! ime_engaged; then
    echo "INFO: 首次注入未观察到合成，等待 5s 后重试（fcitx5 激活可能滞后）"
    sleep 5
    ensure_focus "$WID" || { kill "$PID" 2>/dev/null; exit 1; }
    inject_pinyin
fi

if ime_engaged; then
    echo "PASS: 输入法已介入（观察到预编辑事件）"
else
    echo "WARN: 本次注入未观察到预编辑；可能是 fcitx5 状态或焦点时序，不能据此断定框架不支持"
fi

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
