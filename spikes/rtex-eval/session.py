"""Bounded JSON-lines driver; full worker messages are retained as gzip JSONL."""

import gzip
import json
import os
from pathlib import Path
import queue
import signal
import subprocess
import threading
import time


class Session:
    def __init__(self, command, env, cwd, evidence, name):
        self.command = command
        self.events = []
        self.messages = queue.Queue(maxsize=64)
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
                line = self.process.stdout.readline(16 * 1024 * 1024 + 1)
                total += len(line)
                if not line:
                    break
                if len(line) > 16 * 1024 * 1024 or total > 64 * 1024 * 1024:
                    raise ValueError("worker output exceeded harness budget")
                self.messages.put(json.loads(line))
        except Exception as error:
            self.messages.put({"harness_error": str(error)})
        finally:
            self.messages.put({"harness_eof": True})

    def record(self, direction, value):
        self.transcript.write(json.dumps({"direction": direction,
                                         "monotonic": time.monotonic(), "value": value}) + "\n")
        self.transcript.flush()

    def send(self, value):
        self.record("stdin", value)
        self.process.stdin.write((json.dumps(value) + "\n").encode())
        self.process.stdin.flush()

    def wait(self, predicate, seconds=45):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            try:
                message = self.messages.get(timeout=max(0.01, deadline - time.monotonic()))
            except queue.Empty:
                break
            self.record("stdout", message)
            self.events.append({k: message[k] for k in (
                "event", "reply", "status", "state", "compile", "pages_total", "reason",
                "versions", "result", "items", "source") if k in message})
            if message.get("harness_eof") or message.get("harness_error"):
                raise RuntimeError(str(message))
            if predicate(message):
                return message
        raise TimeoutError(f"no matching worker message within {seconds}s")

    def close(self):
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
            self.record("exit", {"status": self.process.returncode})
            self.process.stdout.close()
            self.process.stdin.close()
            self.stderr.close()
            self.transcript.close()


def launch(clone, project, build, evidence, name, sandbox, extra_env=None, texbin=None, seconds=150):
    """Start `rtex serve`; `texbin` replaces the TeX binary directory (evaluation launcher)."""
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
                   str(seconds), "/rtex/bin/rtex", "serve", "--project", "/project", "--build", "/work/build"]
    else:
        if texbin:
            env["RTEX_TEXLIVE_BIN"] = str(texbin)
        command = [str(clone / "target/release/rtex"), "serve", "--project", str(project),
                   "--build", str(build / "build")]
    return Session(command, env, root, evidence, name)
