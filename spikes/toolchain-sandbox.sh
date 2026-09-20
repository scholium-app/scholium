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
# Arch Linux runtime profile: mount only explicit tools and their shared libraries.
# The executable list is trusted configuration, never derived from project content.
mounts=(--dir /usr --dir /usr/bin --dir /usr/lib --dir /usr/share
  --symlink usr/bin /bin --symlink usr/lib /lib --symlink usr/lib /lib64
  --symlink lib /usr/lib64)
declare -A mounted=()
mount_file() {
  local target="$1" source
  [ -e "$target" ] || return 0
  [ -z "${mounted[$target]:-}" ] || return 0
  source="$(realpath -e "$target")"
  mounts+=(--ro-bind "$source" "$target")
  mounted[$target]=1
}
mount_libraries() {
  local executable="$1" library
  while IFS= read -r library; do
    [ -z "$library" ] || mount_file "$library"
  done < <(/usr/bin/ldd "$executable" 2>/dev/null | /usr/bin/awk '
    /=> \// {print $3}
    /^[[:space:]]*\// {print $1}
  ')
}
# Profiles are selected by trusted callers, never by document content.
# The editor helper needs fonts and its own libraries, not the TeX runtime.
case "${SCHOLIUM_SANDBOX_PROFILE:-full}" in
  full|recovery)
    runtime_tools=(bash sh xelatex xetex xdvipdfmx kpsewhich paper paperconf prlimit
      bwrap timeout realpath ldd awk cp mv cat touch readlink sleep dd
      pdfinfo pdftotext pdftohtml pdfimages)
    runtime_paths=(/usr/lib/locale /usr/share/fonts /usr/share/fontconfig /usr/share/texmf-dist
      /usr/share/texmf /etc/fonts /etc/texmf /etc/paperspecs /etc/papersize /var/lib/texmf)
    mount_file /usr/lib/localepaper
    mount_libraries /usr/lib/localepaper
    if [ "${SCHOLIUM_SANDBOX_PROFILE:-full}" = recovery ]; then
      runtime_tools+=(env perl latexmk)
      runtime_paths+=(/usr/lib/perl5 /usr/share/perl5)
    fi
    ;;
  typst-editor)
    runtime_tools=(prlimit)
    runtime_paths=(/usr/share/fonts /usr/share/fontconfig /etc/fonts)
    ;;
  pdf-probe)
    runtime_tools=(sh prlimit pdfinfo pdftotext pdftohtml pdfimages)
    runtime_paths=(/usr/share/fonts /usr/share/fontconfig /etc/fonts /usr/share/poppler)
    ;;
  *) echo 'unknown sandbox profile' >&2; exit 2 ;;
esac
for tool in "${runtime_tools[@]}"; do
  mount_file "/usr/bin/$tool"
  mount_libraries "/usr/bin/$tool"
done
for runtime in "${runtime_paths[@]}"; do
  [ ! -e "$runtime" ] || mounts+=(--ro-bind "$runtime" "$runtime")
done
# Optional trusted toolchain directory, supplied by the harness, never package data.
if [ -n "${SCHOLIUM_SPIKE_TOOLS:-}" ]; then
  mounts+=(--ro-bind "$(realpath -e "$SCHOLIUM_SPIKE_TOOLS")" /toolchain)
  for executable in "$SCHOLIUM_SPIKE_TOOLS"/*; do
    [ ! -f "$executable" ] || mount_libraries "$executable"
  done
fi
# Nested package builds inherit the outer cgroup. This marker is a read-only
# mount created here, outside /project and /work; document environment is cleared.
resource_scope=()
if [ -e /run/scholium-resource-policy ] && [ ! -e /usr/bin/systemd-run ]; then
  mounts+=(--ro-bind /run/scholium-resource-policy /run/scholium-resource-policy)
else
  [ -x /usr/bin/systemd-run ] || { echo 'user cgroup supervisor unavailable' >&2; exit 2; }
  resource_scope=(/usr/bin/env "XDG_RUNTIME_DIR=/run/user/$UID"
    /usr/bin/systemd-run --user --scope --quiet
    -p MemoryMax=2G -p MemorySwapMax=0 -p TasksMax=64 -p OOMPolicy=kill --)
  mounts+=(--ro-bind /usr/bin/prlimit /run/scholium-resource-policy)
fi
exec "${resource_scope[@]}" /usr/bin/timeout --kill-after=2s "${seconds}s" \
  /usr/bin/bwrap --unshare-all --die-with-parent --new-session --cap-drop ALL \
  --clearenv --setenv PATH /usr/bin --setenv HOME /tmp \
  --setenv LANG C.UTF-8 --setenv TEXMFVAR /tmp/texmf-var \
  --setenv TEXMFCONFIG /tmp/texmf-config --setenv TEXMFHOME /tmp/texmf-home \
  --setenv TEXINPUTS /project: --setenv TEXPICTS /project: \
  "${mounts[@]}" --proc /proc --dev /dev --size 268435456 --tmpfs /tmp \
  --ro-bind "$input" /project --bind "$output" /work --chdir /work \
  /usr/bin/prlimit --as=4294967296 --fsize=67108864 --cpu=30 --nofile=128 -- "$@"
