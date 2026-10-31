#!/usr/bin/env bash
# What the X server delivers for `xdotool key ctrl+F1` versus `ctrl+<F1 keycode>`.
# Runs on a private Xvfb :97 only (never a real desktop). Usage, on an Ubuntu image:
#   podman run --rm -v "$PWD:/s:ro" ubuntu:26.04 /s/repro-ctrl-f1.sh
# or on a GitHub runner: bash repro-ctrl-f1.sh
set -u
SUDO=$([ "$(id -u)" = 0 ] && echo || echo sudo)
export DEBIAN_FRONTEND=noninteractive
$SUDO apt-get update -qq >/dev/null 2>&1
$SUDO apt-get install -y -qq xvfb xdotool x11-utils x11-xkb-utils python3 >/dev/null 2>&1
Xvfb :97 -screen 0 1280x900x24 >/dev/null 2>&1 &
XVFB=$!
sleep 2
export DISPLAY=:97
grep PRETTY_NAME /etc/os-release
dpkg-query -W xdotool xvfb xkb-data
echo "== key type of <FK01> (F1) in the server keymap"
xkbcomp -xkb :97 /tmp/repro-keymap.xkb 2>/dev/null
awk '/key <FK01>/,/};/' /tmp/repro-keymap.xkb
awk '/type "CTRL\+ALT"/,/};/' /tmp/repro-keymap.xkb
F1=$(python3 - <<'PY'
import ctypes, os
x11 = ctypes.CDLL("libX11.so.6")
x11.XOpenDisplay.restype = ctypes.c_void_p
x11.XStringToKeysym.restype = ctypes.c_ulong
x11.XKeysymToKeycode.restype = ctypes.c_ubyte
x11.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
display = x11.XOpenDisplay(os.environ["DISPLAY"].encode())
print(x11.XKeysymToKeycode(display, x11.XStringToKeysym(b"F1")))
PY
)
for chord in "ctrl+F1" "ctrl+$F1"; do
  xev -geometry 400x300 -name repro -event keyboard >/tmp/repro-xev.log 2>&1 &
  XEV=$!
  sleep 1
  W=$(xdotool search --name repro | head -1)
  xdotool windowfocus "$W"
  sleep 0.3
  echo "== xdotool key --clearmodifiers $chord  (events seen by the focused client)"
  xdotool key --clearmodifiers "$chord"
  sleep 0.5
  grep -A2 -E '^Key(Press|Release)' /tmp/repro-xev.log |
    awk '/^Key/{t=$1} /keycode/{sub(/^ +/, ""); print t, $0}'
  kill "$XEV"
  sleep 0.3
done
kill "$XVFB"
