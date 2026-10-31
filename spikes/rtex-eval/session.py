"""Bounded JSON-lines driver; full worker messages are retained as gzip JSONL.

Round 2 additions: every message carries its receipt time (taken right after the line was read,
before JSON parsing) in `Session.received_at`; output budgets are configurable for 100-page
layouts; `redact=True` replaces display lists in the transcript by their size so latency runs
keep small evidence.
"""

import gzip
import json
import os
from pathlib import Path
import queue
import signal
import subprocess
import threading
import time

MB = 1024 * 1024


def redacted(message):
    """The message with display lists replaced by their JSON size (transcripts only)."""
    def size(value):
        return {"redacted_dl_bytes": len(json.dumps(value))}
    out = dict(message)
    if "dl" in out:
        out["dl"] = size(out["dl"])
    if isinstance(out.get("pages_changed"), list):
        out["pages_changed"] = [{**p, "dl": size(p.get("dl"))} for p in out["pages_changed"]]
    if "placements" in out:
        out["placements"] = {"redacted_placements": len(out["placements"] or [])}
    return out


class Session:
    def __init__(self, command, env, cwd, evidence, name, line_limit=16 * MB, total_limit=64 * MB,
                 redact=False, sandboxed=False):
        self.command = command
        self.sandboxed = sandboxed
        self.events = []
        self.messages = queue.Queue()
        self.line_limit, self.total_limit, self.redact = line_limit, total_limit, redact
        self.received_at = None
        self.started_at = time.monotonic()
        self.transcript = gzip.open(evidence / f"{name}.jsonl.gz", "wt")
        self.stderr = (evidence / f"{name}.stderr.log").open("w")
        self.record("launch", {"argv": command, "cwd": str(cwd),
                               "env": {k: v for k, v in env.items()
                                       if k.startswith(("RTEX_", "SCHOLIUM_"))}})
        self.process = subprocess.Popen(command, env=env, cwd=cwd, stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, stderr=self.stderr,
                                        start_new_session=True)
        threading.Thread(target=self.read, daemon=True).start()

    def read(self):
        try:
            total = 0
            while True:
                line = self.process.stdout.readline(self.line_limit + 1)
                received = time.monotonic()
                total += len(line)
                if not line:
                    break
                if len(line) > self.line_limit or total > self.total_limit:
                    raise ValueError("worker output exceeded harness budget")
                self.messages.put((received, json.loads(line)))
        except Exception as error:
            self.messages.put((time.monotonic(), {"harness_error": str(error)}))
        finally:
            self.messages.put((time.monotonic(), {"harness_eof": True}))

    def record(self, direction, value, at=None):
        self.transcript.write(json.dumps({"direction": direction,
                                         "monotonic": at if at is not None else time.monotonic(),
                                         "value": value}) + "\n")
        self.transcript.flush()

    def send(self, value):
        """Writes one command; returns the monotonic time just before the write."""
        sent = time.monotonic()
        self.record("stdin", value, sent)
        self.process.stdin.write((json.dumps(value) + "\n").encode())
        self.process.stdin.flush()
        return sent

    def wait(self, predicate, seconds=45):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            try:
                received, message = self.messages.get(timeout=max(0.01, deadline - time.monotonic()))
            except queue.Empty:
                break
            self.received_at = received
            self.record("stdout", redacted(message) if self.redact else message, received)
            self.events.append({k: message[k] for k in (
                "event", "reply", "status", "state", "compile", "pages_total", "reason",
                "versions", "result", "items", "source") if k in message})
            if message.get("harness_eof") or message.get("harness_error"):
                raise RuntimeError(str(message))
            if predicate(message):
                return message
        raise TimeoutError(f"no matching worker message within {seconds}s")

    def memory_peak(self):
        """cgroup v2 `memory.peak` of the sandbox scope (systemd-run keeps the launcher's pid)."""
        if not self.sandboxed:
            return None
        try:
            entry = Path(f"/proc/{self.process.pid}/cgroup").read_text().strip().split("::", 1)[1]
            return int(Path(f"/sys/fs/cgroup{entry}/memory.peak").read_text())
        except (OSError, IndexError, ValueError):
            return None

    def close(self):
        self.peak_bytes = self.memory_peak()
        try:
            if self.process.poll() is None:
                self.send({"cmd": "quit"})
                self.process.wait(timeout=3)
        except (BrokenPipeError, subprocess.TimeoutExpired):
            pass
        finally:
            # Terminate descendants too; a stopped supervisor alone is insufficient.
            try:
                os.killpg(self.process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            self.process.wait()
            self.record("exit", {"status": self.process.returncode, "memory_peak_bytes": self.peak_bytes})
            self.process.stdout.close()
            self.process.stdin.close()
            self.stderr.close()
            self.transcript.close()


def launch(clone, project, build, evidence, name, sandbox, extra_env=None, texbin=None, seconds=150,
           serve_args=(), **session_options):
    """Start `rtex serve`; `texbin` replaces the TeX binary directory (evaluation launcher).

    `extra_env` reaches the sandbox script (e.g. `SCHOLIUM_RTEX_FONT_CACHE`), never the document.
    """
    root = Path(__file__).resolve().parents[2]
    env = dict(os.environ, RTEX_TEXLIVE_BIN="/usr/bin", RTEX_TEXDIR=str(clone / "tex"))
    env.update(extra_env or {})
    build.mkdir(parents=True)
    if sandbox:
        env.update(SCHOLIUM_SANDBOX_PROFILE="rtex", SCHOLIUM_RTEX_BIN=str(clone / "target/release/rtex"),
                   SCHOLIUM_RTEX_TEXDIR=str(clone / "tex"))
        if texbin:
            env["SCHOLIUM_RTEX_EVAL_TEXBIN"] = str(texbin)
        command = ["bash", str(root / "spikes/toolchain-sandbox.sh"), str(project), str(build),
                   str(seconds), "/rtex/bin/rtex", "serve", "--project", "/project", "--build",
                   "/work/build", *serve_args]
    else:
        if texbin:
            env["RTEX_TEXLIVE_BIN"] = str(texbin)
        command = [str(clone / "target/release/rtex"), "serve", "--project", str(project),
                   "--build", str(build / "build"), *serve_args]
    return Session(command, env, root, evidence, name, sandboxed=sandbox, **session_options)
