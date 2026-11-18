"""Capture every page of the running V2 window, including the part below the fold.

The app scrolls inside `.page`, so `captureBeyondViewport` cannot reach the rest.
Instead each page is captured twice: at the top, and scrolled to the bottom when
its content is taller than the viewport.

    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222 rtx-spark-ai-control-v2.exe
    python tools/shots.py screenshots
"""

from __future__ import annotations

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from cdp import CDP, click_nav  # noqa: E402

PAGES = ["Home", "Server", "Blackout", "Settings", "Logs"]
SETTLE = float(1.6)


def scroll_metrics(cdp: CDP) -> dict:
    return cdp.eval(
        "(()=>{const p=document.querySelector('.page');"
        "return {sh:p.scrollHeight,ch:p.clientHeight,st:p.scrollTop}})()"
    )


def scroll_to(cdp: CDP, top: int) -> None:
    cdp.eval(
        "(()=>{const p=document.querySelector('.page');"
        f"p.scrollTop={top};return p.scrollTop}})()"
    )


def main(argv: list[str]) -> int:
    out = Path(argv[1] if len(argv) > 1 else "screenshots")
    out.mkdir(parents=True, exist_ok=True)
    cdp = CDP()
    try:
        for index, label in enumerate(PAGES, start=1):
            click_nav(cdp, label)
            time.sleep(SETTLE)
            scroll_to(cdp, 0)
            time.sleep(0.4)
            slug = label.lower()
            w, h = cdp.shot(str(out / f"phase2-0{index}-{slug}.png"))
            print(f"{slug:9s} top      {w}x{h}")
            metrics = scroll_metrics(cdp)
            if metrics["sh"] > metrics["ch"] + 2:
                scroll_to(cdp, metrics["sh"])
                time.sleep(0.8)
                w, h = cdp.shot(str(out / f"phase2-0{index}-{slug}-b.png"))
                print(f"{slug:9s} bottom   {w}x{h}")
                scroll_to(cdp, 0)
                time.sleep(0.3)
    finally:
        cdp.ws.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
