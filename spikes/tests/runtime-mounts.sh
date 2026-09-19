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
SCHOLIUM_SANDBOX_PROFILE=pdf-probe SCHOLIUM_HOST_SENTINEL=private \
  bash "$root/spikes/toolchain-sandbox.sh" "$work/input" "$work/output" 10 /bin/sh -ec '
  read -r value < /project/control || test "$value" = CONTROL
  test "$value" = CONTROL
  test -z "${SCHOLIUM_HOST_SENTINEL:-}"
  for tool in pdfinfo pdftotext pdftohtml pdfimages; do test -x /usr/bin/$tool; done
  for tool in xelatex xetex kpsewhich bash bwrap curl cp; do test ! -e /usr/bin/$tool; done
  test ! -e /usr/share/texmf-dist
  test ! -e /etc/hostname
  test ! -e /usr/share/doc
  test -d /usr/share/fonts
  if (printf changed > /project/control) 2>/dev/null; then exit 1; fi
  printf PDF-PROBE-OK > /work/pdf-probe-control
'
test "$(cat "$work/output/pdf-probe-control")" = PDF-PROBE-OK
printf 'PASS: PDF probe exposes inspection tools, read-only input and writable output; omits TeX and unrelated tools\n'
SCHOLIUM_SANDBOX_PROFILE=recovery \
  bash "$root/spikes/toolchain-sandbox.sh" "$work/input" "$work/output" 10 /bin/sh -ec '
  test -x /usr/bin/latexmk
  test -x /usr/bin/perl
  test ! -e /etc/hostname
  test ! -e /usr/bin/curl
  latexmk -norc -v > /work/latexmk-version
  if (printf changed > /project/control) 2>/dev/null; then exit 1; fi
'
test -s "$work/output/latexmk-version"
printf 'PASS: recovery profile runs latexmk and preserves input/host boundaries\n'
