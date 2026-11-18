#!/usr/bin/env python
"""Single-stream generation probe against the running vLLM server.

Used to check whether the V2 window costs real inference throughput, not just
synthetic GEMM. It only sends ordinary chat-completion requests to the existing
server; it never starts, stops or reconfigures anything.

    python tools/infer-probe.py --label v2-visible --requests 3
"""

from __future__ import annotations

import argparse
import json
import statistics
import time
import urllib.request
from pathlib import Path

ENDPOINT = "http://127.0.0.1:8000/v1/chat/completions"
PROMPT = (
    "Write a compact Python function that merges two sorted integer lists in "
    "linear time. Include a docstring and two example calls in the reply."
)


def one_request(max_tokens: int) -> dict:
    body = {
        "model": "Qwen/Qwen3.8-Flash-Next",
        "messages": [{"role": "user", "content": PROMPT}],
        "max_tokens": max_tokens,
        "temperature": 0.0,
        "stream": False,
    }
    data = json.dumps(body).encode()
    req = urllib.request.Request(
        ENDPOINT, data=data, headers={"Content-Type": "application/json"}
    )
    began = time.perf_counter()
    with urllib.request.urlopen(req, timeout=180) as resp:
        payload = json.load(resp)
    elapsed = time.perf_counter() - began
    usage = payload.get("usage", {})
    completion = usage.get("completion_tokens") or 0
    return {
        "seconds": round(elapsed, 3),
        "completion_tokens": completion,
        "tokens_per_s": round(completion / elapsed, 2) if elapsed and completion else None,
    }


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--label", required=True)
    ap.add_argument("--requests", type=int, default=3)
    ap.add_argument("--max-tokens", type=int, default=128)
    args = ap.parse_args()

    runs = []
    for _ in range(args.requests):
        try:
            runs.append(one_request(args.max_tokens))
        except OSError as exc:
            print(f"request failed: {exc}")
            break
        time.sleep(1.0)

    rates = [r["tokens_per_s"] for r in runs if r["tokens_per_s"]]
    report = {
        "label": args.label,
        "requests": len(runs),
        "max_tokens": args.max_tokens,
        "tokens_per_s_median": round(statistics.median(rates), 2) if rates else None,
        "tokens_per_s_samples": rates,
        "seconds_median": (
            round(statistics.median([r["seconds"] for r in runs]), 3) if runs else None
        ),
        "runs": runs,
        "captured_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }
    text = json.dumps(report, indent=2)
    print(text)
    path = Path("results") / f"infer-{args.label}.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text + "\n", encoding="utf-8")
    print(f"written {path}")


if __name__ == "__main__":
    main()
