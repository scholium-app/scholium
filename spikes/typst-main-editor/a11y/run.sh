#!/usr/bin/env bash
# Isolated AT-SPI session: private a11y bus + registry + org.a11y.Status, never the desktop's.
# Usage (repo root): xvfb-run -a -s '-screen 0 1280x900x24' dbus-run-session -- \
#   bash spikes/typst-main-editor/a11y/run.sh target/debug/scholium-app /tmp/out
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
logs="${2%/}-helpers"
mkdir -p "$logs"
export AT_SPI_BUS_ADDRESS="$(dbus-daemon --config-file=/usr/share/defaults/at-spi2/accessibility.conf --fork --print-address)"
registry=""
for candidate in /usr/lib/at-spi2-registryd /usr/libexec/at-spi2-registryd \
    /usr/libexec/at-spi2-core/at-spi2-registryd /usr/lib/at-spi2-core/at-spi2-registryd; do
    [ -x "$candidate" ] && registry="$candidate" && break
done
[ -n "$registry" ] || { echo "at-spi2-registryd not found" >&2; exit 2; }
"$registry" --use-gnome-session=false >"$logs/registry.log" 2>&1 &
registry_pid=$!
# at-spi2-core 2.62 no longer exposes ScreenReaderEnabled, which AccessKit waits for.
python3 "$here/a11y_status.py" "$AT_SPI_BUS_ADDRESS" >"$logs/status.log" 2>&1 &
status_pid=$!
sleep 1
for pid in "$registry_pid" "$status_pid"; do
    kill -0 "$pid" 2>/dev/null || { echo "AT-SPI helper exited; see $logs" >&2; cat "$logs"/*.log >&2; exit 2; }
done
exec python3 "$here/a11y_check.py" "$@"
