#!/usr/bin/env bash
# 渲染后端对比：首窗口时间、稳态 CPU、stderr 噪声、窗口是否创建。
#
# 用法：spikes/native-ui/candidate-iced/scripts/compare-backends.sh [次数]
#   RUNS=3 bash scripts/compare-backends.sh
#
# 只截取/观测本应用自己的窗口；不采集屏幕其他内容。

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/debug/scholium-spike-iced"
RUNS="${1:-3}"
SETTLE_MS=3000

[ -x "$BIN" ] || { echo "FAIL: 未构建 $BIN"; exit 1; }

# 空字符串 = 不设置 ICED_BACKEND，观察默认选择。
BACKENDS=("default" "wgpu" "tiny-skia")

window_present() {
    niri msg --json windows 2>/dev/null | grep -q 'Scholium spike'
}

cpu_seconds() {
    # /proc/<pid>/stat 第 14、15 字段是 utime/stime，单位是时钟滴答。
    local pid="$1"
    local stat
    stat="$(cat "/proc/$pid/stat" 2>/dev/null)" || { echo "n/a"; return; }
    local hz
    hz="$(getconf CLK_TCK)"
    echo "$stat" | awk -v hz="$hz" '{printf "%.3f", ($14 + $15) / hz}'
}

rss_kb() {
    awk '/VmRSS/ {print $2}' "/proc/$1/status" 2>/dev/null || echo "n/a"
}

for backend in "${BACKENDS[@]}"; do
    for run in $(seq 1 "$RUNS"); do
        log="/tmp/backend-${backend}-${run}.log"
        if [ "$backend" = "default" ]; then
            "$BIN" >"$log" 2>&1 &
        else
            ICED_BACKEND="$backend" "$BIN" >"$log" 2>&1 &
        fi
        pid=$!

        start_ns=$(date +%s%N)
        first_ms=""
        for _ in $(seq 1 150); do
            if window_present; then
                first_ms=$(( ($(date +%s%N) - start_ns) / 1000000 ))
                break
            fi
            sleep 0.1
        done

        sleep "$(awk -v ms="$SETTLE_MS" 'BEGIN {print ms/1000}')"
        cpu="$(cpu_seconds "$pid")"
        mem="$(rss_kb "$pid")"
        errors="$(grep -ci error "$log" 2>/dev/null | head -1)"

        kill "$pid" 2>/dev/null
        sleep 1
        kill -0 "$pid" 2>/dev/null && kill -9 "$pid" 2>/dev/null

        printf '%-9s run%s  首窗口=%sms  CPU=%ss  内存=%sKB  stderr_error=%s\n' \
            "$backend" "$run" "${first_ms:-未出现}" "${cpu:-n/a}" "${mem:-n/a}" "$errors"
    done
done

echo
echo "提示：GL 适配器在本机会以 'Parent device is lost' 失败并打印 MESA/ZINK 噪声，"
echo "      这是 wgpu 探测适配器时的正常回退，不代表 Vulkan 路径不可用；用 gpu_probe 复核。"
