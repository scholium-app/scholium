#!/usr/bin/env bash
# Scholium UI 冒烟测试：启动应用、注入键盘/鼠标输入、按阶段截图到指定目录。
#
# 用法:
#   scripts/ui-smoke.sh <scenario> [输出目录] [应用路径]
#
# 场景:
#   sample         启动示例工作区：默认视觉页 + 源码分屏（静态夹具）
#   blocks         新建文档：输入标题与两段正文、回车分段、下拉切换一级标题
#   source-preview 新建文档：输入内容后切源码模式，等待 Typst 编译并截图
#   all            同一窗口依次执行 sample → blocks → source-preview
#
# 依赖: ydotool/ydotoold、grim、magick、Wayland 会话（niri 验证）。
# 环境变量:
#   WIN_X WIN_Y WIN_W WIN_H  窗口几何（物理像素，默认 600 280 1380 950）
#   COMPILE_WAIT_MS           切源码后等待编译的毫秒数（默认 5000）
#   TYPE_DELAY_MS             逐键注入延迟（默认 35）
#
# 键码为 Linux input-event-codes：Ctrl=29 N=46 数字2=3 Enter=28 Home=102。
# 输入注入在合成器层（uinput），应用无法区分人工与脚本输入。
set -euo pipefail

SCENARIO="${1:-all}"
OUT="${2:-/tmp/scholium-ui-smoke}"
BIN="${3:-target/debug/scholium-app}"

WIN_X="${WIN_X:-600}"; WIN_Y="${WIN_Y:-280}"
WIN_W="${WIN_W:-1380}"; WIN_H="${WIN_H:-950}"
COMPILE_WAIT_MS="${COMPILE_WAIT_MS:-5000}"
TYPE_DELAY_MS="${TYPE_DELAY_MS:-35}"

mkdir -p "$OUT"

say() { printf '\033[1;34m[smoke]\033[0m %s\n' "$*"; }
die() { printf '\033[1;31m[smoke]\033[0m %s\n' "$*" >&2; exit 1; }

command -v ydotool >/dev/null || die "缺少 ydotool"
command -v grim >/dev/null || die "缺少 grim"
command -v magick >/dev/null || die "缺少 magick"
[[ -x "$BIN" ]] || die "应用不存在：$BIN（先 cargo build -p scholium-app）"

if ! pgrep -x ydotoold >/dev/null; then
  ydotoold & sleep 1
  pgrep -x ydotoold >/dev/null || die "ydotoold 启动失败"
fi

# 组合键：combo <修饰键码> <键码>
combo() { ydotool key "$1:1" "$2:1" "$2:0" "$1:0"; }
press() { ydotool key "$1:1" "$1:0"; }
enter() { press 28; }
type_text() { ydotool type -d "$TYPE_DELAY_MS" "$1"; }

# 截图：全屏 + 窗口裁剪两份
shot() {
  local name="$1"
  grim "$OUT/$name.png"
  magick "$OUT/$name.png" -crop "${WIN_W}x${WIN_H}+${WIN_X}+${WIN_Y}" +repage "$OUT/$name-crop.png"
  say "截图 → $OUT/$name-crop.png"
}

# 点击窗口内相对坐标：win_click <dx> <dy>
win_click() {
  ydotool mousemove -a -x $((WIN_X + $1)) -y $((WIN_Y + $2))
  sleep 0.15
  ydotool click 0xC0
  sleep 0.3
}

focus_window() {
  ydotool mousemove -a -x $((WIN_X + WIN_W / 2)) -y $((WIN_Y + WIN_H / 2))
  ydotool click 0xC0
  sleep 0.5
}

launch() {
  ("$BIN" >"$OUT/app.log" 2>&1 &)
  local waited=0
  until pgrep -f "$BIN" >/dev/null; do
    (( waited += 200 )); (( waited > 15000 )) && die "应用未启动，见 $OUT/app.log"
    sleep 0.2
  done
  sleep 3   # 首帧与字体初始化
  focus_window
  say "应用已启动（日志 $OUT/app.log）"
}

stop_app() { pkill -f "$BIN" 2>/dev/null || true; }
trap stop_app EXIT

scenario_sample() {
  say "== 场景 sample：示例工作区 =="
  shot 01-sample-visual
  combo 29 3          # Ctrl+2 → 源码分屏（静态夹具）
  sleep 1
  shot 02-sample-source
  combo 29 2          # Ctrl+1 回视觉
  sleep 0.5
}

scenario_blocks() {
  say "== 场景 blocks：新建多块文档 =="
  combo 29 46         # Ctrl+N 新建
  sleep 1.5
  type_text "Spectrum and Vibration"
  enter; sleep 0.3
  type_text "Let the string have length L, tension T and density rho."
  enter; sleep 0.3
  type_text "Fixed endpoints give the boundary conditions."
  sleep 0.5
  shot 03-blocks-typed
  # 段落样式下拉（窗口内偏移）→ 一级标题
  win_click 140 45
  sleep 0.6
  shot 04-blocks-combo
  win_click 140 60
  sleep 0.8
  shot 05-blocks-heading
}

scenario_source_preview() {
  say "== 场景 source-preview：Typst 实时预览 =="
  combo 29 3          # Ctrl+2 → 源码模式
  sleep 1
  shot 06-source-entered
  say "等待编译 ${COMPILE_WAIT_MS}ms…"
  sleep $(awk "BEGIN{print $COMPILE_WAIT_MS/1000}")
  shot 07-source-compiled
  # 编辑后重编译：回到视觉模式追加一段，再切回源码
  combo 29 2; sleep 0.6
  press 103           # ↑ 跨块到上一块
  press 108           # ↓ 回尾块
  type_text " Rayleigh quotient characterizes the modes."
  sleep 0.4
  combo 29 3
  sleep $(awk "BEGIN{print $COMPILE_WAIT_MS/1000}")
  shot 08-source-recompiled
}

case "$SCENARIO" in
  sample|blocks|source-preview|all) ;;
  *) die "未知场景：$SCENARIO（可选 sample|blocks|source-preview|all）" ;;
esac

say "输出目录：$OUT"
launch
case "$SCENARIO" in
  sample)         scenario_sample ;;
  blocks)         scenario_blocks ;;
  source-preview) scenario_source_preview ;;
  all)            scenario_sample; scenario_blocks; scenario_source_preview ;;
esac
say "完成：$OUT/*-crop.png（窗口裁剪版供核对）"
