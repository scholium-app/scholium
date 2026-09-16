#!/usr/bin/env bash
# Exercise the real runner with a fake cargo; no compiler or desktop required.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cat > "$work/cargo" <<'CARGO'
#!/usr/bin/env bash
printf '%s\n' "${FAKE_OUTPUT:-all checks passed}"
exit "${FAKE_STATUS:-0}"
CARGO
chmod +x "$work/cargo"
export PATH="$work:$PATH"
runner="$root/spikes/verify-stage0.sh"

if bash "$runner" typo > "$work/unknown" 2>&1; then
  echo 'FAIL: unknown section passed'; exit 1
fi
if FAKE_STATUS=7 FAKE_OUTPUT='compiler failure detail' bash "$runner" core > "$work/error" 2>&1; then
  echo 'FAIL: command failure passed'; exit 1
fi
grep -q 'compiler failure detail' "$work/error"
if FAKE_OUTPUT='  [FAIL] simulated assertion' bash "$runner" reconcile > "$work/assertion" 2>&1; then
  echo 'FAIL: printed assertion failure passed'; exit 1
fi
grep -q 'simulated assertion' "$work/assertion"
bash "$runner" core > "$work/success" 2>&1
grep -q '通过 1 项，失败 0 项' "$work/success"
echo 'PASS: unknown section, command failure, assertion failure, successful run'
