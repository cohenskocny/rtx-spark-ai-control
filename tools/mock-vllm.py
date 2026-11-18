#!/usr/bin/env python3
"""A stand-in for `vllm serve`, used to test the V2 lifecycle end to end.

The real server on port 8000 is production inference: starting it twice, or
stopping it to watch what happens, would interrupt whatever is running. This
mock speaks enough of the OpenAI surface to exercise the controller instead:

    python tools/mock-vllm.py 8011
    # then launch V2 with
    #   V2_TEST_VLLM_EXE=<python.exe>
    #   V2_TEST_VLLM_ARGS=<this file> {port}
    #   V2_TEST_PORT=8011

It prints weight-loading progress with percentages (so the progress state can be
checked), announces startup the way uvicorn does, serves GET /v1/models, and holds
the socket until it is terminated. `--fail` makes it exit with a traceback
instead, which is how a failed launch is verified.

Test-only tooling: it is not part of the application bundle.
"""
from __future__ import annotations

import argparse
import json
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


def chatter(delay: float) -> None:
    for pct in (10, 25, 50, 75, 99):
        print(f"Loading safetensors shard {pct}% ({pct}/100)", flush=True)
        time.sleep(delay)
    print("INFO:     Started server process [mock]", flush=True)
    print("INFO:     Uvicorn running on http://127.0.0.1 (Press CTRL+C to quit)", flush=True)
    print("INFO:     Application startup complete.", flush=True)


def main(argv=None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("port", nargs="?", type=int, default=8011)
    ap.add_argument("--fail", action="store_true", help="exit with a traceback")
    ap.add_argument("--model", default="Qwen/Qwen3.8-Flash-Next")
    ap.add_argument("--anchor", action="store_true", help="print the pid and stay quiet")
    args = ap.parse_args(argv)

    if args.fail:
        print("Loading safetensors shard 40%", flush=True)
        print("Traceback (most recent call last):", flush=True)
        print('RuntimeError: failed to load weights from checkpoint', flush=True)
        return 1

    print(f"INFO:     vLLM mock starting on port {args.port} (pid {__import__('os').getpid()})", flush=True)
    if not args.anchor:
        threading.Thread(target=chatter, args=(0.6,), daemon=True).start()

    class Handler(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def do_GET(self):  # noqa: N802
            if self.path.rstrip("/") in ("/v1/models", "/models"):
                body = json.dumps(
                    {"object": "list", "data": [{"id": args.model, "object": "model"}]}
                ).encode()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
                return
            self.send_error(404)

        def log_message(self, *_args):
            pass

    server = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
