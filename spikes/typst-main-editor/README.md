# typst-main-editor window checks

Real-window checks drive the built `scholium-app` under a private Xvfb display and assert on
`SCHOLIUM_EDITOR_AUDIT` output. They never touch the desktop display.

Run all of them exactly as CI does:

    cargo build --locked -p scholium-app -p scholium-storage --bins
    python3 scripts/window-checks.py [--binary target/debug/scholium-app] [--out DIR] [--only NAME...] [--list]

The runner runs checks sequentially, never retries (a retry would hide flakiness), writes
`<out>/<name>/` evidence plus `<out>/<name>.log` per check, prints a summary table and exits
non-zero if any check failed. `DISPLAY`/`WAYLAND_DISPLAY` are stripped from the child environment.

## Adding a check

Name it `spikes/typst-main-editor/<name>-check.py` with the signature `<binary> <outdir>`. It is
discovered automatically; do not edit `.github/workflows/rust.yml`. The AT-SPI check is `a11y/run.sh`
(run inside `dbus-run-session`).

## Exceptions

Defaults: `xvfb-run -a -s '-screen 0 1280x900x24'`, 300 s timeout. Per-check overrides live in the
`EXCEPTIONS` table at the top of `scripts/window-checks.py`: `screen` (e.g. `sharp-raster` uses
1920x1400x24), `extra` (arguments after the binary, e.g. `default-launch` needs `target/debug/hammer`),
`session` (`dbus` for a11y), `timeout`, and `skip` (a one-line reason).

## Lock

The whole run holds an exclusive `flock` on `/tmp/scholium-window-checks.lock`, so parallel tasks on one
machine never run window checks at the same time. A busy lock prints "waiting for another window-check
run" and blocks.
