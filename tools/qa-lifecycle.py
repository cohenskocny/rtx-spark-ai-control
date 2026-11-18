"""Exercise the V2 server lifecycle end to end against the mock server.

Requires V2 to be running with the QA hooks pointed at tools/mock-vllm.py:

    V2_TEST_PORT=8011
    V2_TEST_VLLM_EXE=<venv>\\Scripts\\python.exe
    V2_TEST_VLLM_ARGS=<repo>\\tools\\mock-vllm.py {port}
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222

Steps: Start, Restart, Stop, external detection, Adopt, Stop again. Every step
prints the rows the Server page actually renders, so the report can quote the
application rather than the test harness.

    python tools/qa-lifecycle.py
"""

from __future__ import annotations

import os
import socket
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import psutil  # noqa: E402

from cdp import CDP, click_nav  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
PYTHON = Path(sys.executable)
MOCK = ROOT / "tools" / "mock-vllm.py"
PORT = 8011


def _pid_from(row: str) -> int:
    tail = row.rsplit("pid ", 1)
    return int(tail[-1]) if len(tail) == 2 and tail[-1].strip().isdigit() else 0


def port_open(port: int = PORT) -> bool:
    with socket.socket() as sock:
        sock.settimeout(0.4)
        return sock.connect_ex(("127.0.0.1", port)) == 0


def rows(cdp: CDP) -> dict[str, str]:
    """Read every key/value row the Server page renders."""
    return cdp.eval(
        "(()=>{const out={};document.querySelectorAll('.row').forEach(r=>{"
        "const k=r.querySelector('.k'),v=r.querySelector('.v');"
        "if(k&&v)out[k.textContent.trim()]=v.textContent.trim()});return out})()"
    )


def click(cdp: CDP, label: str) -> str:
    return cdp.eval(
        "(()=>{const b=[...document.querySelectorAll('button')]"
        f".find(x=>x.textContent.trim()==='{label}');if(!b)return 'missing';"
        "if(b.disabled)return 'disabled';b.click();return 'clicked'})()"
    )


def wait_for(cdp: CDP, key: str, wanted: str, timeout: float, absent: str | None = None):
    deadline = time.time() + timeout
    seen = ""
    while time.time() < deadline:
        seen = rows(cdp).get(key, "")
        if wanted in seen and (absent is None or absent not in seen):
            return True, seen
        time.sleep(0.5)
    return False, seen


def main() -> int:
    cdp = CDP()
    results: list[tuple[str, bool, str]] = []
    try:
        click_nav(cdp, "Server")
        time.sleep(1.0)
        print("initial  ", rows(cdp))

        # 1. Start
        t0 = time.time()
        print("start    ", click(cdp, "Start"))
        ok, state = wait_for(cdp, "Health", "200", 90)
        info = rows(cdp)
        elapsed = time.time() - t0
        pid_one = info.get("Process", "")
        results.append(("Start reaches ready", ok, f"{elapsed:.2f}s {info.get('Stage')} {pid_one}"))
        print(f"  ready  {elapsed:.2f}s  {state}  {pid_one}")

        # 2. Restart
        t0 = time.time()
        print("restart  ", click(cdp, "Restart"))
        time.sleep(1.0)
        ok, _ = wait_for(cdp, "Health", "200", 90)
        info = rows(cdp)
        elapsed = time.time() - t0
        pid_two = info.get("Process", "")
        results.append(
            ("Restart replaces the process", ok and pid_two != pid_one, f"{elapsed:.2f}s {pid_two}")
        )
        print(f"  ready  {elapsed:.2f}s  {pid_two}")

        # 3. Stop
        print("stop     ", click(cdp, "Stop"))
        ok, state = wait_for(cdp, "Health", "no response", 60, absent="200")
        time.sleep(1.5)
        results.append(
            ("Stop releases the port", ok and not port_open(), f"port_open={port_open()} state={state}")
        )
        print(f"  stopped  port_open={port_open()}  {state}")

        # 4. External server is detected, never terminated
        external = subprocess.Popen(
            [str(PYTHON), str(MOCK), str(PORT), "--anchor"],
            cwd=str(ROOT),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        ok, state = wait_for(cdp, "Ownership", "left alone", 60)
        info = rows(cdp)
        results.append(
            ("External server detected", ok, f"{info.get('Ownership')} {info.get('Process')}")
        )
        print(f"  external {info.get('Ownership')}  {info.get('Process')}")

        # 5. Adopt, then Stop takes the adopted process and nothing else
        listener_pid = _pid_from(info.get("Process", ""))
        print("adopt    ", click(cdp, "Take ownership"))
        ok, state = wait_for(cdp, "Ownership", "adopted from outside", 30)
        info = rows(cdp)
        results.append(("Adopt takes ownership", ok, info.get("Ownership", "")))
        print(f"  adopted  {info.get('Ownership')}  listener={listener_pid}")
        print("  stop   ", click(cdp, "Stop"))
        wait_for(cdp, "Health", "no response", 60, absent="200")
        deadline = time.time() + 20
        while time.time() < deadline and psutil.pid_exists(listener_pid):
            time.sleep(0.3)
        still_there = psutil.pid_exists(listener_pid)
        results.append(
            (
                "Adopted process can be stopped",
                not still_there and not port_open(),
                f"listener_alive={still_there} port_open={port_open()}",
            )
        )
        results.append(
            (
                "Stop leaves the test harness alone",
                True,
                f"this process (pid {os.getpid()}) survived the stop",
            )
        )
        print(f"  listener_alive={still_there} port_open={port_open()}")
    finally:
        cdp.ws.close()

    print("\n== summary ==")
    bad = 0
    for name, ok, detail in results:
        print(f"{'PASS' if ok else 'FAIL':4s} {name:38s} {detail}")
        bad += 0 if ok else 1
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
