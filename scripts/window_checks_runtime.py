"""Machine-wide scheduling and private X11 process lifetimes for window checks."""
from contextlib import contextmanager
import fcntl
import os
from pathlib import Path
import secrets
import selectors
import signal
import struct
import subprocess
import tempfile
import time

# SH is compatible with other new runners; an old runner holds EX for its suite.
LOCK_PREFIX = "/tmp/scholium-window-checks"


@contextmanager
def check_slot(jobs, exclusive=False, prefix=LOCK_PREFIX):
    # The gate makes exclusive checks exclude *every* runner, even if their K differs.
    # A turnstile stops new readers starving an exclusive waiter.
    with open(f"{prefix}.admission.lock", "a") as admission:
        fcntl.flock(admission, fcntl.LOCK_EX)
        with open(f"{prefix}.lock", "a") as gate:
            fcntl.flock(gate, fcntl.LOCK_EX if exclusive else fcntl.LOCK_SH)
            if exclusive:
                fcntl.flock(admission, fcntl.LOCK_UN)
                yield
                return
            while True:
                for index in range(jobs):
                    slot = open(f"{prefix}.{index}.lock", "a")
                    try:
                        fcntl.flock(slot, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    except BlockingIOError:
                        slot.close()
                    else:
                        fcntl.flock(admission, fcntl.LOCK_UN)
                        try:
                            yield
                        finally:
                            slot.close()
                        return
                # Only lock acquisition polls; no UI state or assertion uses a sleep.
                time.sleep(.05)


def stop_group(process):
    """Checks can leave app/helpers alive on failure; terminate the whole private group."""
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        process.wait(timeout=3)
    except subprocess.TimeoutExpired:
        pass
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    process.wait()


@contextmanager
def private_display(screen, env):
    # An explicit high display keeps :0 off limits even on headless CI. Xvfb locks
    # it atomically; -displayfd only publishes it after the server is ready.
    # A FamilyWild cookie authenticates only this server without knowing its number yet.
    with tempfile.TemporaryDirectory(prefix="scholium-x11-") as tmp:
        auth = Path(tmp) / "Xauthority"
        cookie = secrets.token_bytes(16)
        fields = (b"", b"", b"MIT-MAGIC-COOKIE-1", cookie)
        auth.write_bytes(struct.pack(">H", 65535) + b"".join(
            struct.pack(">H", len(field)) + field for field in fields))
        auth.chmod(0o600)
        process, number = start_display(screen, env, auth)
        try:
            yield dict(env, DISPLAY=f":{number}", XAUTHORITY=str(auth), WINIT_UNIX_BACKEND="x11")
        finally:
            stop_group(process)


def start_display(screen, env, auth):
    # Collisions may happen across slots. Only display allocation retries, never a check.
    for _ in range(16):
        number = 100 + secrets.randbelow(30000)
        if Path(f"/tmp/.X11-unix/X{number}").exists() or Path(f"/tmp/.X{number}-lock").exists():
            continue
        read_fd, write_fd = os.pipe()
        try:
            with open(auth.parent / f"Xvfb-{number}.log", "w") as server_log:
                process = subprocess.Popen(
                    ["Xvfb", f":{number}", "-displayfd", str(write_fd), "-screen", "0", screen,
                     "-nolisten", "tcp", "-noreset", "-auth", str(auth)], env=env,
                    pass_fds=(write_fd,), stdout=subprocess.DEVNULL, stderr=server_log,
                    start_new_session=True)
        except BaseException:
            os.close(read_fd)
            raise
        finally:
            os.close(write_fd)
        try:
            with selectors.DefaultSelector() as selector:
                selector.register(read_fd, selectors.EVENT_READ)
                if not selector.select(timeout=15):
                    raise RuntimeError("Xvfb did not publish a ready display within 15s")
                # Xvfb may write the digits and newline separately. Closing on a
                # partial read kills it with "Cannot write display number to fd".
                with os.fdopen(read_fd, "rb", closefd=False) as ready:
                    published = ready.readline(64).decode().strip()
            if published == str(number):
                return process, number
            stop_group(process)
        except BaseException:
            stop_group(process)
            raise
        finally:
            os.close(read_fd)
    raise RuntimeError("Xvfb could not allocate a private display in 16 attempts")
