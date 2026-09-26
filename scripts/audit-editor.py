#!/usr/bin/env python3
"""Run current-product checks and opt-in usability contracts without hiding failures.

python3 scripts/audit-editor.py [--native] [--out DIR] [--quick]
Uses only Python stdlib and Cargo. --native adds isolated Xvfb/X11 UI checks.
Exit 1 means failed checks/contracts; 2 means infrastructure/runner failure.
The default output is a fresh /tmp directory; personal sessions are never opened.
"""
import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
TEST_RESULT = re.compile(r"^test (\S+) \.\.\. (ok|FAILED|ignored[^\n]*)$", re.MULTILINE)


def execute(name, argv, out, env, timeout=600):
    print(f"[{name}] {' '.join(argv)}", flush=True)
    started = time.monotonic()
    log = out / f"{name}.log"
    error = None
    with log.open("w") as stream:
        try:
            proc = subprocess.run(argv, cwd=ROOT, env=env, stdout=stream,
                                  stderr=subprocess.STDOUT, timeout=timeout, check=False)
            code = proc.returncode
        except (OSError, subprocess.TimeoutExpired) as exc:
            error, code = str(exc), 2
            stream.write(f"\nRUNNER ERROR: {exc}\n")
    text = log.read_text(errors="replace")
    cases = [{"name": name, "status": status.split(",")[0]}
             for name, status in TEST_RESULT.findall(text)]
    if name == "native" and code == 2:
        error = "Native test infrastructure failed; see native/results.json and native.log"
    result = {"name": name, "command": argv, "exit_code": code,
              "seconds": round(time.monotonic() - started, 3), "log": log.name,
              "tests": cases, "error": error}
    print(f"  exit={code}; {len(cases)} test results; {log}", flush=True)
    return result


def metadata():
    def read(*argv):
        return subprocess.check_output(argv, cwd=ROOT, text=True).strip()
    return {"utc": datetime.now(timezone.utc).isoformat(), "commit": read("git", "rev-parse", "HEAD"),
            "working_tree": read("git", "status", "--short"), "rust": read("rustc", "--version")}


def report(out, info, stages):
    audit = next(s for s in stages if s["name"] == "usability")
    cases = audit["tests"]
    passed = sum(c["status"] == "ok" for c in cases)
    failed = sum(c["status"] == "FAILED" for c in cases)
    if not cases or any(c["status"].startswith("ignored") for c in cases):
        audit["error"] = "Usability tests missing or ignored; audit is incomplete"
        audit["exit_code"] = 2
    (out / "summary.json").write_text(json.dumps({"environment": info, "stages": stages},
                                                ensure_ascii=False, indent=2) + "\n")
    lines = ["# 当前编辑器自动验收", "", f"基线提交：`{info['commit']}`", "",
             f"体验用例：{passed} 通过，{failed} 失败，共 {len(cases)} 项。", "",
             "失败是未满足的操作契约，不做 expected-failure 豁免。", "",
             "| 检查 | 退出码 | 秒 | 日志 |", "|---|---:|---:|---|"]
    for stage in stages:
        lines.append(f"| {stage['name']} | {stage['exit_code']} | {stage['seconds']} | [{stage['log']}]({stage['log']}) |")
    native_file = out / "native/results.json"
    if native_file.exists():
        native = json.loads(native_file.read_text())
        lines += ["", "## 原生窗口", "", "| 场景 | 结果 |", "|---|---|"]
        lines += [f"| {case['name']} | {case['status']} |" for case in native]
    metrics = re.findall(r"^METRIC .+$", (out / audit["log"]).read_text(), re.MULTILINE)
    if metrics:
        lines += ["", "## Debug 性能观测（毫秒）", "", "```text", *metrics, "```"]
    lines += ["", "## 体验明细", "", "| 用例 | 结果 |", "|---|---|"]
    lines += [f"| `{c['name'].split('usability_audit::')[-1]}` | {c['status']} |" for c in cases]
    lines += ["", "无窗口测试驱动真实 egui 输入、SessionBridge 和 LocalSession；排版用例使用真实 Typst 字形。",
              "它不等同系统 IME、GPU 像素或跨平台验收。`--native` 另测隔离 X11 原生窗口、剪贴板及 SQLite 保存。",
              "原生测试证据见 native/results.json；没有该文件表示未运行或未完成。",
              "性能采样是 debug 构建下的页面输入/会话处理和编译等待，不是完整屏幕呈现延迟。", ""]
    (out / "summary.md").write_text("\n".join(lines))
    print(f"\n体验：{passed} passed / {failed} failed\n报告：{out / 'summary.md'}", flush=True)
    return 2 if any(s["error"] for s in stages) else int(any(s["exit_code"] for s in stages))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--native", action="store_true")
    parser.add_argument("--quick", action="store_true", help="only run opt-in usability contracts")
    args = parser.parse_args()
    out = args.out.resolve() if args.out else Path(tempfile.mkdtemp(prefix="scholium-editor-audit-"))
    out.mkdir(parents=True, exist_ok=True)
    # Do not silently overwrite evidence from another run.
    if any(out.iterdir()):
        parser.error("output directory must be empty; use a fresh directory")
    env = dict(os.environ, CARGO_TERM_COLOR="never", RUST_BACKTRACE="0",
               SCHOLIUM_SESSION_FILE=str(out / "isolated-session.sqlite"))
    info = metadata()
    stages = []
    if not args.quick:
        checks = [("format", ["cargo", "fmt", "--all", "--", "--check"]),
                  ("size", ["python3", "scripts/check-rust-size.py"]),
                  ("clippy", ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"]),
                  ("baseline", ["cargo", "test", "--workspace", "--", "--test-threads=2"]),
                  ("rustdoc", ["cargo", "doc", "--workspace", "--no-deps"])]
        for name, argv in checks:
            check_env = dict(env, RUSTDOCFLAGS="-D warnings") if name == "rustdoc" else env
            stages.append(execute(name, argv, out, check_env))
    stages.append(execute("usability", ["cargo", "test", "-p", "scholium-app", "usability_audit", "--",
                                        "--include-ignored", "--test-threads=1", "--show-output"], out, env))
    if args.native:
        build = execute("native-build", ["cargo", "build", "-p", "scholium-app"], out, env)
        stages.append(build)
        if build["exit_code"] == 0:
            stages.append(execute("native", ["xvfb-run", "-a", "-s", "-screen 0 1280x1024x24",
                                              "python3", "scripts/audit-editor-native.py", str(out / "native")], out, env))
    return report(out, info, stages)


if __name__ == "__main__":
    raise SystemExit(main())
