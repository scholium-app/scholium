#!/usr/bin/env bash
# 阶段 0 七项否决性验证 + 出口条件检查的一键复现脚本。
#
# 用法：bash spikes/verify-stage0.sh            # 全部
#       bash spikes/verify-stage0.sh license    # 只跑许可门禁
#
# 每一项都会打印 PASS/FAIL，并在结尾给出汇总。退出码非零表示有项目失败。
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1
export CARGO_HOME="$PWD/spikes/native-ui/.cargo-home"

ONLY="${1:-all}"
case "$ONLY" in
  all|core|ui|typst|reconcile|items|security|license|web) ;;
  *) printf '未知验证分段：%s\n' "$ONLY" >&2; exit 2 ;;
esac
LOG_DIR="$(mktemp -d "${TMPDIR:-/tmp}/scholium-stage0.XXXXXX")" || exit 1
printf '本次日志：%s\n' "$LOG_DIR"
pass=0
fail=0
declare -a FAILED

section() { printf '\n=== %s ===\n' "$1"; }

record() { # name exit_code
  if [ "$2" -eq 0 ]; then pass=$((pass+1)); printf '  PASS  %s\n' "$1";
  else fail=$((fail+1)); FAILED+=("$1"); printf '  FAIL  %s\n' "$1"; fi
}

run() { # name command...
  local name="$1"; shift
  local log="$LOG_DIR/$((pass+fail+1)).log"
  local status=0
  "$@" >"$log" 2>&1 || status=$?
  # Some exploratory binaries print assertions without propagating an exit code.
  # Treat explicit failed evidence as failure until every runner has a typed result.
  if [ "$status" -eq 0 ] && grep -Eq '(^|[^[:alnum:]_])FAIL([^[:alnum:]_]|$)' "$log"; then
    status=1
  fi
  record "$name" "$status"
  if [ "$status" -ne 0 ]; then
    tail -10 "$log" | sed 's/^/        /'
    printf '        完整日志：%s\n' "$log"
  fi
}

want() { [ "$ONLY" = "all" ] || [ "$ONLY" = "$1" ]; }

# ---------- 共享核心测试 ----------
if want core; then
  section "共享核心（语义图 / 光标 / 语义编辑 / 布局）"
  run "核心测试" cargo test --offline --manifest-path spikes/native-ui/core/Cargo.toml
fi

# ---------- 第 1 项：原生 UI ----------
if want ui; then
  section "第 1 项 原生 UI（egui 选定；iced 对照）"
  run "egui 输入契约与性能测试" cargo test --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml
  run "iced 输入契约测试" cargo test --offline --manifest-path spikes/native-ui/candidate-iced/Cargo.toml
  for c in iced gpui egui; do
    run "候选 $c 构建" cargo build --offline --manifest-path "spikes/native-ui/candidate-$c/Cargo.toml"
  done
fi

# ---------- 第 2 项：Typst 映射 ----------
if want typst; then
  section "第 2 项 Typst 映射（生成 / 定位 / 渲染比对 / 时延）"
  run "typst-mapping 主流程" cargo run --release --offline --manifest-path spikes/typst-mapping/Cargo.toml
  run "typst-mapping 字符级映射" cargo run --release --offline --manifest-path spikes/typst-mapping/Cargo.toml -- charmap
  run "typst-mapping 渲染比对" cargo run --release --offline --manifest-path spikes/typst-mapping/Cargo.toml -- render
  run "typst-mapping 时延与结果门" cargo run --release --offline --manifest-path spikes/typst-mapping/Cargo.toml -- latency
fi

# ---------- 第 3 项：源码 reconcile ----------
if want reconcile; then
  section "第 3 项 源码 reconcile"
  run "source-reconcile" cargo run --release --offline --manifest-path spikes/source-reconcile/Cargo.toml
fi

# ---------- 第 4–7 项：各 spike ----------
if want items; then
  section "第 4–7 项"
  for s in crdt-race crdt-engines recovery language-coordination mixed-build; do
    run "$s" cargo run --release --offline --manifest-path "spikes/$s/Cargo.toml"
  done
fi

# ---------- 出口条件：安全 ----------
if want security; then
  section "出口条件 安全（不受信源码隔离）"
  run "untrusted-input" cargo run --release --offline --manifest-path spikes/untrusted-input/Cargo.toml
fi

# ---------- 出口条件：许可与来源 ----------
if want license; then
  section "出口条件 许可与来源（cargo deny：licenses / bans / sources）"
  # 注意：advisories 需要联网获取 RustSec 数据库，离线环境下无法运行——这里明确跳过而不是假装通过。
  for d in spikes/native-ui/core \
           spikes/native-ui/candidate-iced spikes/native-ui/candidate-gpui spikes/native-ui/candidate-egui \
           spikes/typst-mapping spikes/source-reconcile spikes/untrusted-input \
           spikes/crdt-race spikes/crdt-engines spikes/recovery spikes/language-coordination spikes/mixed-build; do
    [ -f "$d/Cargo.lock" ] || continue
    ok=0
    for check in licenses bans sources; do
      ( cd "$d" && cargo deny --offline check "$check" >"$LOG_DIR/deny-$(basename "$d")-$check.log" 2>&1 ) || {
        ok=1
        printf '        %s / %s 失败：\n' "$(basename "$d")" "$check"
        tail -3 "$LOG_DIR/deny-$(basename "$d")-$check.log" | sed 's/^/          /'
      }
    done
    record "deny $(basename "$d")" $ok
  done
  printf '  提示：advisories 需要联网，本脚本在离线环境跳过该项（CI 必须跑）。\n'
fi

# ---------- 出口条件：无 npm/Node/WebView ----------
if want web; then
  section "出口条件 无 npm / Node.js / WebView"
  ok=0
  for f in package.json package-lock.json yarn.lock pnpm-lock.yaml; do
    find . -name "$f" -not -path '*/.cargo-home/*' -not -path '*/target/*' | grep -q . && ok=1
  done
  [ -d node_modules ] && ok=1
  record "仓库内无 npm/Node 工程" $ok
fi

section "汇总"
printf '  通过 %d 项，失败 %d 项\n' "$pass" "$fail"
if [ "$fail" -gt 0 ]; then
  printf '  失败项：\n'
  for name in "${FAILED[@]}"; do printf '    - %s\n' "$name"; done
  exit 1
fi
printf '  本次所选检查通过；阶段出口仍以报告中的未完成项为准。\n'
