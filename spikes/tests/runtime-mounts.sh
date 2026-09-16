#!/usr/bin/env bash
# A runtime probe must distinguish omitted host resources from a dead sandbox.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir "$work/input" "$work/output"
printf 'CONTROL' > "$work/input/control"
bash "$root/spikes/toolchain-sandbox.sh" "$work/input" "$work/output" 10 /bin/sh -ec '
  test "$(cat /project/control)" = CONTROL
  printf OK > /work/control
  test -x /usr/bin/xelatex
  test -d /usr/share/texmf-dist
  test -d /usr/share/fonts
  test ! -e /usr/share/doc
  test ! -e /usr/bin/curl
  test ! -e /usr/lib/chromium
  test ! -e /etc/hostname
'
test "$(cat "$work/output/control")" = OK
printf 'PASS: runtime allowlist exposes tools/resources but omits unrelated host trees\n'
