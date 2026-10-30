"""Harness-owned canaries for the E2 negative controls and the leak scan over every channel.

Secrets are random per run and never appear in fixture source, so finding one in the PDF text,
a build-directory file, stderr or a JSON event proves that the document read it. Paths carry a
separate random token, so echoed source (which names the paths) is not mistaken for a leak.
"""

import gzip
import json
from pathlib import Path
import re
import secrets
import shutil
import socket
import subprocess
import threading

ENV_NAME = "E2_HOST_ENV_CANARY"
WRITE_MARKERS = ("written-by-execute.txt", "written-by-write18.txt", "written-by-lua.txt",
                 "written-by-openout.tex")


def secret(kind):
    # Short digit-separated runs: no hyphenation point, no line overflow in the probe output.
    return f"E2{kind}-{secrets.randbelow(10**8):08d}-{secrets.randbelow(10**8):08d}"


class Canaries:
    """Plants files under $HOME, an env value and a loopback listener; removes them on close."""

    def __init__(self):
        self.dir = Path.home() / f".scholium-e2-{secrets.token_hex(6)}"
        self.dir.mkdir(mode=0o700)
        with open("/etc/passwd", encoding="utf-8") as passwd:
            first_line = passwd.readline().strip()
        self.secrets = {"passwd": first_line, "home_file": secret("FILE"),
                        "input_file": secret("INPUT"), "env": secret("ENV"),
                        "network": secret("NET")}
        (self.dir / "secret.txt").write_text(self.secrets["home_file"] + "\n")
        (self.dir / "secret-input.tex").write_text(f"[B1 home input] {self.secrets['input_file']}\n")
        self.connections = 0
        self.listener = socket.socket()
        self.listener.bind(("127.0.0.1", 0))
        self.listener.listen(4)
        self.port = self.listener.getsockname()[1]
        threading.Thread(target=self.serve, daemon=True).start()

    def serve(self):
        while True:
            try:
                client, _ = self.listener.accept()
            except OSError:
                return
            self.connections += 1
            with client:
                client.sendall((self.secrets["network"] + "\n").encode())

    def env(self):
        return {ENV_NAME: self.secrets["env"]}

    def placeholders(self, project):
        return {"@CANARY_DIR@": str(self.dir), "@PROJECT_DIR@": str(project), "@PORT@": str(self.port)}

    def reset(self, project):
        """Clear write markers and the connection count before one run."""
        for name in WRITE_MARKERS:
            (self.dir / name).unlink(missing_ok=True)
        (project / "written-into-project.txt").unlink(missing_ok=True)
        self.connections = 0

    def side_effects(self, project):
        """Host-visible effects outside the build directory after one run."""
        found = [name for name in WRITE_MARKERS if (self.dir / name).exists()]
        if (project / "written-into-project.txt").exists():
            found.append("project/written-into-project.txt")
        return {"files_outside_build": found, "listener_connections": self.connections}

    def close(self):
        self.listener.close()
        shutil.rmtree(self.dir)


def pdf_text(pdf):
    result = subprocess.run(["pdftotext", "-layout", str(pdf), "-"], capture_output=True, text=True)
    return result.stdout if result.returncode == 0 else ""


def stdout_messages(transcript):
    with gzip.open(transcript, "rt") as lines:
        return "\n".join(line for line in lines if json.loads(line)["direction"] == "stdout")


def channels(build, export, evidence, name):
    """Text of every channel the document could reach: PDF, build files, stderr, JSON events."""
    files = []
    for path in sorted(p for p in build.rglob("*") if p.is_file() and not p.is_symlink()):
        data = path.read_bytes()
        files.append(data.decode("utf-8", "replace"))
        if data.startswith(b"%PDF"):
            files.append(pdf_text(path))
    return {"pdf_text": pdf_text(export) if export.exists() else "",
            "build_files": "\n".join(files),
            "stderr": (evidence / f"{name}.stderr.log").read_text(errors="replace"),
            "json_events": stdout_messages(evidence / f"{name}.jsonl.gz")}


def leaks(found_channels, values):
    """Map each secret name to the channels where it appeared (whitespace-insensitive)."""
    squeezed = {k: "".join(v.split()) for k, v in found_channels.items()}
    report = {}
    for key, value in values.items():
        needle = "".join(value.split())
        hits = [channel for channel, text in squeezed.items() if needle and needle in text]
        if hits:
            report[key] = hits
    return report


def probe_lines(text):
    """The probes' own `[label] result` pairs from the PDF text, for the human-readable record."""
    flat = " ".join(text.split())
    marks = list(re.finditer(r"\[([A-F][0-9][^\]]*)\] ", flat))
    pairs = []
    for index, mark in enumerate(marks):
        end = marks[index + 1].start() if index + 1 < len(marks) else len(flat)
        value = flat[mark.end():end].split(" Probe ")[0]
        pairs.append(f"[{mark.group(1)}] {re.sub(r' [0-9]+$', '', value).strip()}")
    return pairs
