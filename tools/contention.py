#!/usr/bin/env python
"""Short BF16 GEMM probe used to check whether the V2 window costs inference.

Run one probe per condition, back-to-back, and compare medians:

    python tools/contention.py --label v2-closed --reps 6
    python tools/contention.py --label v2-visible --reps 6

Each probe warms the clocks for a few seconds, then times six 8192^3 BF16
matmuls, recording the SM clock around every rep. It deliberately reports the
spread: on this machine the GPU can fall out of its fast state mid-run, so a
single number without spread would be misleading.

Must run with the existing vLLM environment's interpreter, the one that already
has torch installed — on a native-vLLM installation that is the environment's own
`Scripts\python.exe`.
"""
from __future__ import annotations

import argparse
import json
import statistics
import subprocess
import time
from pathlib import Path

import torch


def clock_mhz() -> float | None:
    try:
        out = subprocess.run(
            ["nvidia-smi", "--query-gpu=clocks.sm,power.draw", "--format=csv,noheader,nounits"],
            capture_output=True, text=True, timeout=6, creationflags=0x08000000,
        ).stdout
    except (OSError, subprocess.SubprocessError):
        return None
    for line in out.splitlines():
        parts = [p.strip() for p in line.split(",")]
        try:
            return float(parts[0])
        except (ValueError, IndexError):
            continue
    return None


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--label", required=True)
    ap.add_argument("--reps", type=int, default=6)
    ap.add_argument("--size", type=int, default=8192)
    ap.add_argument("--warmup", type=float, default=4.0)
    ap.add_argument("--out", default="")
    args = ap.parse_args()

    if not torch.cuda.is_available():
        raise SystemExit("CUDA not available")

    size = args.size
    a = torch.randn(size, size, device="cuda", dtype=torch.bfloat16)
    b = torch.randn(size, size, device="cuda", dtype=torch.bfloat16)

    end = time.perf_counter() + args.warmup
    while time.perf_counter() < end:
        torch.mm(a, b)
    torch.cuda.synchronize()

    reps = []
    for _ in range(args.reps):
        torch.cuda.synchronize()
        t0 = time.perf_counter()
        torch.mm(a, b)
        torch.cuda.synchronize()
        elapsed = time.perf_counter() - t0
        reps.append({
            "ms": round(elapsed * 1e3, 2),
            "tflops": round(2.0 * size ** 3 / elapsed / 1e12, 2),
            "clock_mhz": clock_mhz(),
        })
        time.sleep(0.2)

    tflops = [r["tflops"] for r in reps]
    clocks = [r["clock_mhz"] for r in reps if r["clock_mhz"]]
    report = {
        "label": args.label,
        "size": size,
        "torch": torch.__version__,
        "device": torch.cuda.get_device_name(0),
        "reps": reps,
        "tflops_median": round(statistics.median(tflops), 2),
        "tflops_min": round(min(tflops), 2),
        "tflops_max": round(max(tflops), 2),
        "clock_median_mhz": round(statistics.median(clocks), 0) if clocks else None,
        "captured_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }
    text = json.dumps(report, indent=2)
    print(text)
    target = Path(args.out) if args.out else Path("results") / f"contention-{args.label}.json"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(text + "\n", encoding="utf-8")
    print(f"written {target}")


if __name__ == "__main__":
    main()
