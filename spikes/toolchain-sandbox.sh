#!/usr/bin/env bash
# Linux spike profile. Arguments are supplied by the trusted harness, never a project manifest.
# Usage: bash spikes/toolchain-sandbox.sh INPUT OUTPUT SECONDS /usr/bin/TOOL ARGS...
set -euo pipefail
[ "$#" -ge 4 ] || { echo 'expected INPUT OUTPUT SECONDS TOOL [ARGS...]' >&2; exit 2; }
input="$(realpath -e "$1")"
output="$(realpath -e "$2")"
seconds="$3"
shift 3
[[ "$seconds" =~ ^[1-9][0-9]*$ ]] || exit 2
[ "$input" != "$output" ] || { echo 'input and output must be separate' >&2; exit 2; }
# Runtime resources are trusted, read-only exceptions to the project-only rule.
mounts=()
for runtime in /usr /lib /lib64 /bin /etc/fonts /etc/texmf /etc/paperspecs /etc/papersize /var/lib/texmf; do
  [ ! -e "$runtime" ] || mounts+=(--ro-bind "$runtime" "$runtime")
done
# Optional trusted toolchain directory, supplied by the harness, never package data.
if [ -n "${SCHOLIUM_SPIKE_TOOLS:-}" ]; then
  mounts+=(--ro-bind "$(realpath -e "$SCHOLIUM_SPIKE_TOOLS")" /toolchain)
fi
exec /usr/bin/timeout --kill-after=2s "${seconds}s" \
  /usr/bin/bwrap --unshare-all --die-with-parent --new-session --cap-drop ALL \
  --clearenv --setenv PATH /usr/bin --setenv HOME /tmp \
  --setenv LANG C.UTF-8 --setenv TEXMFVAR /tmp/texmf-var \
  --setenv TEXMFCONFIG /tmp/texmf-config --setenv TEXMFHOME /tmp/texmf-home \
  --setenv TEXINPUTS /project: --setenv TEXPICTS /project: \
  "${mounts[@]}" --proc /proc --dev /dev --tmpfs /tmp \
  --ro-bind "$input" /project --bind "$output" /work --chdir /work \
  /usr/bin/prlimit --as=4294967296 --fsize=67108864 --cpu=30 --nofile=128 -- "$@"
