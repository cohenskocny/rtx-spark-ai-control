"""Minimal Chrome DevTools Protocol client for the V2 capture workflow.

The app is started with WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222,
which exposes the webview on loopback only. Everything here is dev tooling: no
part of this module is shipped with the application.

    python tools/cdp.py eval "document.title"
    python tools/cdp.py shot screenshots/01-home.png
    python tools/cdp.py click Server

Screenshots come from Page.captureScreenshot, i.e. the surface the renderer has
actually composed, at the window's real device scale factor. That is the only
capture path that stays exact while the OLED panel is powered down, where both
screen grabs and PrintWindow return stale or black surfaces.
"""

import base64
import json
import os
import socket
import struct
import sys
import time
import urllib.request


def _list_targets(port: int) -> list:
    with urllib.request.urlopen(f"http://127.0.0.1:{port}/json", timeout=5) as resp:
        return json.load(resp)


class WS:
    def __init__(self, url: str):
        assert url.startswith("ws://"), url
        rest = url[len("ws://"):]
        hostport, _, path = rest.partition("/")
        host, _, port = hostport.partition(":")
        self.sock = socket.create_connection((host, int(port)), timeout=20)
        key = base64.b64encode(os.urandom(16)).decode()
        req = (
            f"GET /{path} HTTP/1.1\r\nHost: {hostport}\r\nUpgrade: websocket\r\n"
            f"Connection: Upgrade\r\nSec-WebSocket-Key: {key}\r\n"
            f"Sec-WebSocket-Version: 13\r\n\r\n"
        )
        self.sock.sendall(req.encode())
        self.buf = b""
        while b"\r\n\r\n" not in self.buf:
            chunk = self.sock.recv(4096)
            if not chunk:
                raise RuntimeError("websocket handshake closed")
            self.buf += chunk
        head, _, self.buf = self.buf.partition(b"\r\n\r\n")
        if b"101" not in head.split(b"\r\n")[0]:
            raise RuntimeError(head.decode(errors="replace"))

    def _read(self, n: int) -> bytes:
        while len(self.buf) < n:
            chunk = self.sock.recv(65536)
            if not chunk:
                raise RuntimeError("websocket closed")
            self.buf += chunk
        out, self.buf = self.buf[:n], self.buf[n:]
        return out

    def send(self, text: str) -> None:
        payload = text.encode()
        header = bytearray([0x81])
        length = len(payload)
        if length < 126:
            header.append(0x80 | length)
        elif length < 65536:
            header.append(0x80 | 126)
            header += struct.pack(">H", length)
        else:
            header.append(0x80 | 127)
            header += struct.pack(">Q", length)
        mask = os.urandom(4)
        header += mask
        self.sock.sendall(bytes(header) + bytes(b ^ mask[i % 4] for i, b in enumerate(payload)))

    def recv(self) -> str:
        while True:
            b0, b1 = self._read(2)
            opcode = b0 & 0x0F
            masked = b1 & 0x80
            length = b1 & 0x7F
            if length == 126:
                length = struct.unpack(">H", self._read(2))[0]
            elif length == 127:
                length = struct.unpack(">Q", self._read(8))[0]
            mask = self._read(4) if masked else None
            data = self._read(length)
            if mask:
                data = bytes(b ^ mask[i % 4] for i, b in enumerate(data))
            if opcode == 0x1:
                return data.decode()
            if opcode == 0x8:
                raise RuntimeError("websocket closed by peer")
            # ping/pong/binary frames are ignored

    def close(self) -> None:
        try:
            self.sock.close()
        except OSError:
            pass


class CDP:
    def __init__(self, port: int = 9222):
        targets = [t for t in _list_targets(port) if t.get("type") == "page"]
        if not targets:
            raise SystemExit("no page target: is the app running with --remote-debugging-port?")
        self.ws = WS(targets[0]["webSocketDebuggerUrl"])
        self.next_id = 0

    def call(self, method: str, params: dict | None = None) -> dict:
        self.next_id += 1
        mid = self.next_id
        self.ws.send(json.dumps({"id": mid, "method": method, "params": params or {}}))
        while True:
            msg = json.loads(self.ws.recv())
            if msg.get("id") == mid:
                if "error" in msg:
                    raise RuntimeError(f"{method}: {msg['error']}")
                return msg.get("result", {})

    def eval(self, expression: str):
        res = self.call(
            "Runtime.evaluate",
            {"expression": expression, "returnByValue": True, "awaitPromise": True},
        )
        if res.get("exceptionDetails"):
            raise RuntimeError(res["exceptionDetails"].get("text", "evaluation failed"))
        return res["result"].get("value")

    def shot(self, path: str, full: bool | None = None) -> tuple[int, int]:
        if full is None:
            full = os.environ.get("SHOT_FULL") == "1"
        params: dict = {"format": "png", "captureBeyondViewport": bool(full)}
        if full:
            metrics = self.call("Page.getLayoutMetrics", {})
            size = metrics.get("cssContentSize") or metrics.get("contentSize") or {}
            params["clip"] = {
                "x": 0,
                "y": 0,
                "width": float(size.get("width", 0)),
                "height": float(size.get("height", 0)),
                "scale": 1,
            }
        data = self.call("Page.captureScreenshot", params)
        blob = base64.b64decode(data["data"])
        os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
        with open(path, "wb") as fh:
            fh.write(blob)
        return _png_size(blob)


def _png_size(blob: bytes) -> tuple[int, int]:
    if blob[:8] != b"\x89PNG\r\n\x1a\n":
        raise RuntimeError("not a png")
    return struct.unpack(">II", blob[16:24])


NAV = (
    "(()=>{const b=[...document.querySelectorAll('nav button')]"
    ".find(x=>x.textContent.trim()===__LABEL__);"
    "if(!b)return 'missing';b.click();return 'clicked'})()"
)


def click_nav(cdp: CDP, label: str) -> str:
    return cdp.eval(NAV.replace("__LABEL__", json.dumps(label)))


def main(argv: list) -> int:
    if len(argv) < 3:
        print(__doc__)
        return 2
    cmd, arg = argv[1], argv[2]
    cdp = CDP()
    try:
        if cmd == "eval":
            print(cdp.eval(arg))
        elif cmd == "click":
            print(click_nav(cdp, arg))
        elif cmd == "shot":
            time.sleep(float(os.environ.get("SHOT_SETTLE", "1.0")))
            w, h = cdp.shot(arg)
            print(f"{arg}  {w}x{h}")
        elif cmd == "pages":
            for i, label in enumerate(["Home", "Server", "Blackout", "Settings", "Logs"]):
                click_nav(cdp, label)
                time.sleep(float(os.environ.get("SHOT_SETTLE", "1.0")))
            print("ok")
        else:
            print(__doc__)
            return 2
    finally:
        cdp.ws.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
