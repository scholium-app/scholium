#!/usr/bin/env bash
# 输入注入测试：验证键盘事件与输入法（预编辑 / 提交）是否真正到达应用。
#
# 用法：spikes/native-ui/candidate-iced/scripts/input-test.sh
#
# 安全约束：**每次注入前都确认焦点是本应用窗口**，焦点不符立即中止。
# 注入走 /dev/uinput（ydotool），是内核级虚拟键盘，会经过输入法，因此可用于 IME 验证。
# 依赖：ydotool（extra/ydotool）+ 正在运行的 ydotoold + niri。

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/debug/scholium-spike-iced"
LOG="/tmp/input-test.log"
SHOT_DIR="$ROOT/artifacts"

[ -x "$BIN" ] || { echo "FAIL: 未构建 $BIN"; exit 1; }
command -v ydotool >/dev/null || { echo "FAIL: 未安装 ydotool"; exit 1; }
systemctl --user is-active --quiet ydotool || { echo "FAIL: ydotoold 未运行"; exit 1; }

window_id() {
    niri msg --json windows 2>/dev/null | python3 -c '
import json, sys
for w in json.load(sys.stdin):
    if "Scholium spike" in w.get("title", ""):
        print(w["id"]); break
'
}

focused_id() {
    niri msg --json windows 2>/dev/null | python3 -c '
import json, sys
for w in json.load(sys.stdin):
    if w.get("is_focused"):
        print(w["id"]); break
'
}

# 把焦点切到目标窗口并复核；复核失败则拒绝注入。
ensure_focus() {
    local want="$1"
    local want="$1" attempt got=""
    for attempt in 1 2 3 4 5; do
        niri msg action focus-window --id "$want" >/dev/null 2>&1
        sleep 0.4
        got="$(focused_id)"
        [ "$got" = "$want" ] && return 0
    done
    echo "ABORT: 焦点不在本应用（want=$want got=${got:-none}），停止注入以免影响其他窗口"
    return 1
}

"$BIN" >"$LOG" 2>&1 &
PID=$!
sleep 6

WID="$(window_id)"
if [ -z "$WID" ]; then
    echo "FAIL: 未找到应用窗口"
    kill "$PID" 2>/dev/null
    exit 1
fi
echo "PASS: 应用窗口 id=$WID"

ensure_focus "$WID" || { kill "$PID" 2>/dev/null; exit 1; }
echo "--- 注入 ASCII：ab ---"
ydotool type "ab"
sleep 1

ensure_focus "$WID" || { kill "$PID" 2>/dev/null; exit 1; }
echo "--- 注入拼音：nihao + 空格 + 回车（检验输入法是否介入） ---"
ydotool type "nihao"
sleep 1
ydotool key 57:1 57:0   # KEY_SPACE
sleep 1
ydotool key 28:1 28:0   # KEY_ENTER
sleep 1

echo
echo "--- 应用事件时间线 ---"
grep -vE '^MESA|^libEGL|^Warning' "$LOG" | tail -25

niri msg action screenshot-window --id "$WID" -d true -p false >/dev/null 2>&1
sleep 2
NEWEST="$(find "$HOME/Pictures/Screenshots" -name '*.png' -newermt '-2 minutes' \
    -printf '%T@ %p\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2-)"
if [ -n "$NEWEST" ]; then
    cp "$NEWEST" "$SHOT_DIR/iced-ime-test.png"
    echo "PASS: 截图 $SHOT_DIR/iced-ime-test.png"
fi

kill "$PID" 2>/dev/null
sleep 1
kill -0 "$PID" 2>/dev/null && kill -9 "$PID" 2>/dev/null
echo "已结束应用"
