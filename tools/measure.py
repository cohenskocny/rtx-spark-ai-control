#!/usr/bin/env python
"""Measure RTX Spark AI Control V2 (or V1) while it sits idle.

The webview runs in child processes, so the process tree is walked and every
descendant counts — including msedgewebview2.exe renderers. CPU is reported as
"percent of one core", which is directly comparable with the V1 measurement.

    python tools/measure.py --label v2-visible --seconds 60
    python tools/measure.py --label v2-minimized --seconds 60 --minimize
    python tools/measure.py --label v1-idle --match pythonw.exe
"""
from __future__ import annotations

import argparse
import ctypes
import json
import statistics
import subprocess
import time
from ctypes import wintypes
from pathlib import Path

import psutil

user32 = ctypes.WinDLL("user32", use_last_error=True)
shcore = ctypes.WinDLL("shcore", use_last_error=True)

SW_MINIMIZE = 6
SW_RESTORE = 9

EXE = "rtx-spark-ai-control-v2.exe"


class RECT(ctypes.Structure):
    _fields_ = [("left", wintypes.LONG), ("top", wintypes.LONG),
                ("right", wintypes.LONG), ("bottom", wintypes.LONG)]


def dpi_aware() -> None:
    try:
        shcore.SetProcessDpiAwareness(2)
    except OSError:
        user32.SetProcessDPIAware()


def find_window(title: str) -> int | None:
    found: list[int] = []

    @ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    def callback(hwnd, _param):
        if not user32.IsWindowVisible(hwnd):
            return True
        length = user32.GetWindowTextLengthW(hwnd)
        if length == 0:
            return True
        buf = ctypes.create_unicode_buffer(length + 1)
        user32.GetWindowTextW(hwnd, buf, length + 1)
        if title.lower() in buf.value.lower() and "shell" not in buf.value.lower():
            found.append(hwnd)
        return True

    user32.EnumWindows(callback, 0)
    return found[0] if found else None


def tree(pid: int) -> list[psutil.Process]:
    try:
        root = psutil.Process(pid)
    except psutil.NoSuchProcess:
        return []
    return [root] + root.children(recursive=True)


def gpu_sample() -> dict:
    try:
        out = subprocess.run(
            ["nvidia-smi",
             "--query-gpu=utilization.gpu,memory.used,power.draw,temperature.gpu,clocks.sm",
             "--format=csv,noheader,nounits"],
            capture_output=True, text=True, timeout=8,
            creationflags=0x08000000,
        ).stdout
    except (OSError, subprocess.SubprocessError):
        return {}
    for line in out.splitlines():
        parts = [p.strip() for p in line.split(",")]
        if len(parts) < 5:
            continue
        try:
            return {
                "gpu_util": float(parts[0]),
                "mem_mib": float(parts[1]),
                "power_w": float(parts[2]),
                "temp_c": float(parts[3]),
                "clock_mhz": float(parts[4]),
            }
        except ValueError:
            continue
    return {}


def health_latency() -> float | None:
    import urllib.request

    began = time.perf_counter()
    try:
        with urllib.request.urlopen("http://127.0.0.1:8000/v1/models", timeout=3) as resp:
            resp.read()
    except OSError:
        return None
    return (time.perf_counter() - began) * 1000.0


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--label", required=True)
    parser.add_argument("--seconds", type=float, default=60)
    parser.add_argument("--pid", type=int)
    parser.add_argument("--match", default=EXE, help="executable name to measure")
    parser.add_argument("--minimize", action="store_true")
    parser.add_argument("--restore", action="store_true")
    parser.add_argument("--out", default="")
    args = parser.parse_args()

    dpi_aware()

    pids = [args.pid] if args.pid else [
        p.pid for p in psutil.process_iter(["pid", "name"])
        if (p.info["name"] or "").lower() == args.match.lower()
    ]
    if not pids:
        raise SystemExit(f"no process matching {args.match}")

    hwnd = find_window("RTX Spark AI Control")
    if hwnd and args.minimize:
        user32.ShowWindow(hwnd, SW_MINIMIZE)
        time.sleep(2.0)
    if hwnd and args.restore:
        user32.ShowWindow(hwnd, SW_RESTORE)
        time.sleep(1.5)

    procs = [p for pid in pids for p in tree(pid)]
    for proc in procs:
        try:
            proc.cpu_percent(None)
        except psutil.Error:
            pass

    cpu_samples: list[float] = []
    rss_samples: list[float] = []
    gpu_samples: list[dict] = []
    latencies: list[float] = []
    proc_counts: list[int] = []

    deadline = time.time() + args.seconds
    while time.time() < deadline:
        time.sleep(1.0)
        total_cpu = 0.0
        total_rss = 0.0
        alive = 0
        for proc in procs:
            try:
                total_cpu += proc.cpu_percent(None)
                total_rss += proc.memory_info().rss
                alive += 1
            except psutil.Error:
                continue
        cpu_samples.append(total_cpu)
        rss_samples.append(total_rss / 2**20)
        proc_counts.append(alive)
        gpu_samples.append(gpu_sample())
        latency = health_latency()
        if latency is not None:
            latencies.append(latency)

    gpu_util = [g["gpu_util"] for g in gpu_samples if "gpu_util" in g]
    power = [g["power_w"] for g in gpu_samples if "power_w" in g]
    report = {
        "label": args.label,
        "seconds": args.seconds,
        "pids": pids,
        "processes_in_tree": statistics.median(proc_counts) if proc_counts else 0,
        "cpu_pct_one_core": {
            "mean": round(statistics.fmean(cpu_samples), 2) if cpu_samples else None,
            "median": round(statistics.median(cpu_samples), 2) if cpu_samples else None,
            "max": round(max(cpu_samples), 2) if cpu_samples else None,
            "samples": len(cpu_samples),
        },
        "rss_mib": {
            "mean": round(statistics.fmean(rss_samples), 1) if rss_samples else None,
            "max": round(max(rss_samples), 1) if rss_samples else None,
        },
        "gpu_util_pct": {
            "mean": round(statistics.fmean(gpu_util), 2) if gpu_util else None,
            "max": round(max(gpu_util), 1) if gpu_util else None,
        },
        "gpu_power_w": {
            "mean": round(statistics.fmean(power), 2) if power else None,
        },
        "api_latency_ms": {
            "mean": round(statistics.fmean(latencies), 1) if latencies else None,
            "samples": len(latencies),
        },
        "captured_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }

    text = json.dumps(report, indent=2)
    print(text)
    target = Path(args.out) if args.out else Path("results") / f"bench-{args.label}.json"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(text + "\n", encoding="utf-8")
    print(f"written {target}")


if __name__ == "__main__":
    main()
