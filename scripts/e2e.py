#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# ///
"""End-to-end: real DeepSeek generations, shown in the real app.

Builds target/GenUI.app, starts it through LaunchServices (so it reads the key the user saved in its settings,
in the login keychain) with the development controls (src/dev.rs), and for each case: asks for a UI, waits for
the generation, watches the sandbox for errors, and takes a screenshot. Prints a summary and writes
<out>/report.json, each case's TSX and PNG. Exit 1 if a case fails.

    scripts/e2e.py [--model deepseek-flash] [--timeout 300] [--settle 4] [--only 1,3] [--out DIR]
"""

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# The shell's examples, the second a change to the first's result ("edit": the model gets the current module).
CASES = [
    ("new", "番茄钟：开始/暂停/重置，显示今天完成了几个"),
    ("edit", "加一个设置：可以把专注时长改成 15、25 或 50 分钟"),
    ("new", "记账本：添加收支、分类筛选，顶部显示本月结余"),
    ("new", "汇率换算：人民币、美元、日元、欧元互换，可交换方向"),
    ("new", "一周天气卡片，带温度折线图"),
    ("new", "待办看板：三列（待办/进行中/完成），可以把卡片往右移"),
]

EVENT = re.compile(r"^event (\S+) (.*)$")
LOG_ERROR = re.compile(r"\b(ERROR|panicked)\b")


class App:
    """GenUI started with `open`, driven through a FIFO on its stdin, read through its log."""

    def __init__(self, log: Path):
        self.log = log
        log.write_text("")
        self.fifo = Path(tempfile.mktemp(prefix="genui-e2e-"))
        os.mkfifo(self.fifo)
        self.proc = subprocess.Popen(
            ["open", "-n", "-W", "--env", "GENUI_DEV=1", "--stdin", str(self.fifo),
             "--stdout", str(log), "--stderr", str(log), str(ROOT / "target/GenUI.app")]
        )
        # Read-write, so opening never blocks on a reader that is not there yet.
        self.fd = os.open(self.fifo, os.O_RDWR)
        self.pos = 0

    def send(self, line: str):
        os.write(self.fd, (line + "\n").encode())

    def lines(self):
        """New log lines since the last call."""
        with self.log.open("rb") as f:
            f.seek(self.pos)
            data = f.read()
        cut = data.rfind(b"\n") + 1
        self.pos += cut
        return data[:cut].decode(errors="replace").splitlines()

    def wait(self, want, timeout: float, seen: list | None = None):
        """Waits for a line `want(line)` returns a value for; `seen` collects every line on the way."""
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            for line in self.lines():
                if seen is not None:
                    seen.append(line)
                if (found := want(line)) is not None:
                    return found
            time.sleep(0.2)
        return None

    def command(self, line: str, timeout: float = 15):
        self.send(line)
        result = self.wait(lambda l: l if l in (f"ok {line}",) or l.startswith(f"err {line}") else None, timeout)
        if result is None or result.startswith("err"):
            raise RuntimeError(f"{line!r}: {result or 'no answer'}")

    def close(self):
        subprocess.run(["pkill", "-x", "nsh-genui"])
        os.close(self.fd)
        self.fifo.unlink(missing_ok=True)
        self.proc.wait(timeout=10)


def event(names):
    def match(line):
        m = EVENT.match(line)
        if m and m[1] in names:
            return m[1], json.loads(m[2])
        return None
    return match


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", help="deepseek-v4-pro (the app's default) or deepseek-flash")
    ap.add_argument("--timeout", type=float, default=300, help="seconds per generation")
    ap.add_argument("--settle", type=float, default=4, help="seconds to watch the sandbox after a load")
    ap.add_argument("--only", help="case numbers, 1-based, e.g. 1,6")
    ap.add_argument("--out", type=Path, default=ROOT / ".shots" / time.strftime("e2e-%Y%m%d-%H%M%S"))
    args = ap.parse_args()

    subprocess.run(["pkill", "-x", "nsh-genui"])
    subprocess.run([str(ROOT / "scripts/bundle.sh")], check=True, stdout=subprocess.DEVNULL)
    args.out.mkdir(parents=True, exist_ok=True)
    picked = {int(n) for n in args.only.split(",")} if args.only else None

    app = App(args.out / "app.log")
    results = []
    try:
        key = app.wait(event({"key"}), 8)
        if key is None and subprocess.run(["pgrep", "-x", "SecurityAgent"], capture_output=True).returncode == 0:
            # An ad hoc signature changes with the code, so the keychain asks again whether GenUI may read its key.
            print("waiting (up to 180s) for the keychain prompt on the Mac's screen: click 始终允许 / Always Allow", flush=True)
            key = app.wait(event({"key"}), 180)
        if key is None:
            print("FAIL: the app gave no `key` event (not started, or the keychain prompt was not answered)", file=sys.stderr)
            return 1
        if not key[1]["hasKey"]:
            print(f"FAIL: no DeepSeek key in the app's settings ({key[1].get('error') or 'not set'})", file=sys.stderr)
            return 1
        app.command("settings close")
        if args.model:
            app.command(f"model {args.model}")

        for n, (mode, prompt) in enumerate(CASES, 1):
            if picked and n not in picked:
                continue
            print(f"[{n}] {mode}: {prompt}", flush=True)
            r = {"case": n, "mode": mode, "prompt": prompt, "ok": False}
            started = time.monotonic()
            app.command(f"prompt {prompt}")
            app.command(f"generate {mode}")
            seen: list[str] = []
            got = app.wait(event({"generated", "failed"}), args.timeout, seen)
            r["wallSeconds"] = round(time.monotonic() - started, 1)
            r["rejected"] = [json.loads(EVENT.match(l)[2])["error"] for l in seen if l.startswith("event rejected ")]
            if got is None:
                r["error"] = f"no answer in {args.timeout:.0f}s"
            elif got[0] == "failed":
                r["error"] = got[1]["error"]
            else:
                g = got[1]
                r.update({k: g[k] for k in ("model", "seconds", "attempts", "promptTokens", "completionTokens", "summary")})
                (args.out / f"{n:02d}.tsx").write_text(g["tsx"])
                # Watch the sandbox: errors its boundary posts, and errors the engines log (a failed load).
                after: list[str] = []
                app.wait(lambda l: None, args.settle, after)
                errors = [json.loads(EVENT.match(l)[2]).get("message", "") for l in after if l.startswith("event sandbox-error ")]
                errors += [l for l in seen + after if LOG_ERROR.search(l)]
                r["runtimeErrors"] = errors
                app.command("tab preview")
                shot = args.out / f"{n:02d}.png"
                app.command(f"shot {shot}")
                r["shot"] = str(shot)
                r["ok"] = not errors
            results.append(r)
            status = "PASS" if r["ok"] else "FAIL"
            detail = (f"{r.get('seconds', 0):.1f}s, {r.get('attempts', '-')} attempt(s), "
                      f"{r.get('completionTokens', '-')} tokens") if "seconds" in r else r.get("error", "")
            print(f"    {status} {detail}" + (f"; runtime: {r['runtimeErrors'][0][:200]}" if r.get("runtimeErrors") else ""), flush=True)
    finally:
        app.close()

    (args.out / "report.json").write_text(json.dumps(results, ensure_ascii=False, indent=2))
    passed = sum(r["ok"] for r in results)
    print(f"\n{passed}/{len(results)} passed; report and screenshots in {args.out}")
    return 0 if passed == len(results) else 1


if __name__ == "__main__":
    sys.exit(main())
