#!/usr/bin/env bash
# 可访问性验收：启动候选，用 AT-SPI 检查它是否向辅助技术发布对象树。
#
# 用法：spikes/native-ui/candidate-iced/scripts/a11y-test.sh
# 依赖：python-atspi（extra/python-atspi）、运行中的 AT-SPI 总线、niri。

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/debug/scholium-spike-iced"
PROBE="$ROOT/scripts/a11y-probe.py"
LOG="/tmp/a11y-test.log"

[ -x "$BIN" ] || { echo "FAIL: 未构建 $BIN"; exit 1; }
python3 -c 'import pyatspi' 2>/dev/null || { echo "FAIL: 未安装 python-atspi"; exit 1; }

"$BIN" >"$LOG" 2>&1 &
PID=$!
sleep 6

if ! kill -0 "$PID" 2>/dev/null; then
    echo "FAIL: 应用提前退出"; cat "$LOG"; exit 1
fi
echo "应用已启动 pid=$PID"
echo

echo "=== 基线：桌面上的可访问应用 ==="
python3 "$PROBE" "__不存在的窗口__" 0
echo

echo "=== 查找本候选（关键字 Scholium） ==="
python3 "$PROBE" "Scholium" 4

echo
echo "=== 对照：任意一个其他 GUI 应用（关键字 QQ） ==="
python3 "$PROBE" "QQ" 1

kill "$PID" 2>/dev/null
sleep 1
kill -0 "$PID" 2>/dev/null && kill -9 "$PID" 2>/dev/null
echo
echo "已结束应用（日志 $LOG）"
