#!/usr/bin/env bash
# Real kernel limits, including descendants; do not count a compile failure as proof.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir "$work/input" "$work/output"
launcher="$root/spikes/toolchain-sandbox.sh"
bash "$launcher" "$work/input" "$work/output" 10 /bin/sh -ec '
  dd if=/dev/zero of=/tmp/a bs=1048576 count=60 status=none
  dd if=/dev/zero of=/tmp/b bs=1048576 count=60 status=none
  dd if=/dev/zero of=/tmp/c bs=1048576 count=60 status=none
  dd if=/dev/zero of=/tmp/d bs=1048576 count=60 status=none
  if dd if=/dev/zero of=/tmp/e bs=1048576 count=60 2>/work/quota-error; then exit 1; fi
  printf quota-enforced > /work/quota
'
test "$(cat "$work/output/quota")" = quota-enforced
grep -q 'No space left on device' "$work/output/quota-error"
printf 'PASS: aggregate temporary storage stops at 256 MiB across multiple files\n'
# Trusted test worker reads the host cgroup before isolation; the child itself
# cannot access the user manager or alter the resource policy.
bash "$launcher" "$work/input" "$work/output" 10 /bin/sh -ec 'sleep 3' &
job=$!
for attempt in {1..30}; do
  scope="$(awk -F: '$1 == 0 {print $3}' "/proc/$job/cgroup" 2>/dev/null || true)"
  if [[ "$scope" == *run-*.scope ]]; then break; fi
  sleep 0.1
done
control="/sys/fs/cgroup$scope"
test "$(cat "$control/memory.max")" = 2147483648
test "$(cat "$control/memory.swap.max")" = 0
test "$(cat "$control/pids.max")" = 64
test "$(cat "$control/memory.oom.group")" = 1
wait "$job"
printf 'PASS: kernel cgroup enforces 2 GiB, no swap, 64 tasks, grouped OOM\n'
mkdir "$work/tools"
rustc --edition 2024 "$root/spikes/tests/resource-worker.rs" -o "$work/tools/resource-worker"
SCHOLIUM_SPIKE_TOOLS="$work/tools" bash "$launcher" "$work/input" "$work/output" 10 \
  /toolchain/resource-worker tasks > "$work/tasks.log"
grep -q 'task quota enforced' "$work/tasks.log"
cat "$work/tasks.log"
status=0
SCHOLIUM_SPIKE_TOOLS="$work/tools" bash "$launcher" "$work/input" "$work/output" 20 \
  /toolchain/resource-worker memory > "$work/memory.log" 2>&1 || status=$?
test "$status" = 137
printf 'PASS: three 768 MiB children terminated as a group before wall timeout (status 137)\n'
