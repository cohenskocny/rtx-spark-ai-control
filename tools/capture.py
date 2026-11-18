"""Capture the RTX Spark AI Control V2 window to a PNG.

Uses PrintWindow(PW_RENDERFULLCONTENT) rather than a screen grab: it renders the
window's own surface, so captures stay valid when the panel is powered down or
the window is occluded, and it is DPI-correct because the window rect is read in
physical pixels from a per-monitor-DPI-aware process.

    python tools/capture.py --out screenshots/01-home.png
    python tools/capture.py --out screenshots/05-compact.png --width 980 --height 660

--width/--height are logical pixels (the window uses 150 % scaling here), so the
resulting image is 1.5x larger.
"""

import argparse
import ctypes
import ctypes.wintypes as wt
import struct
import time
from pathlib import Path

from PIL import Image

user32 = ctypes.windll.user32
gdi32 = ctypes.windll.gdi32

TITLE = "RTX Spark AI Control"
PW_RENDERFULLCONTENT = 0x00000002
SWP_NOMOVE = 0x0002
SWP_NOZORDER = 0x0004
SWP_NOACTIVATE = 0x0010
DWMWA_EXTENDED_FRAME_BOUNDS = 9


class BITMAPINFOHEADER(ctypes.Structure):
    _fields_ = [
        ("biSize", wt.DWORD),
        ("biWidth", wt.LONG),
        ("biHeight", wt.LONG),
        ("biPlanes", wt.WORD),
        ("biBitCount", wt.WORD),
        ("biCompression", wt.DWORD),
        ("biSizeImage", wt.DWORD),
        ("biXPelsPerMeter", wt.LONG),
        ("biYPelsPerMeter", wt.LONG),
        ("biClrUsed", wt.DWORD),
        ("biClrImportant", wt.DWORD),
    ]


class BITMAPINFO(ctypes.Structure):
    _fields_ = [("bmiHeader", BITMAPINFOHEADER), ("bmiColors", wt.DWORD * 3)]


def find_window(title: str) -> int:
    hwnd = user32.FindWindowW(None, title)
    if not hwnd:
        raise SystemExit(f"window not found: {title!r} (is the app running?)")
    return hwnd


def frame_bounds(hwnd: int) -> tuple[int, int, int, int]:
    rect = wt.RECT()
    ctypes.windll.dwmapi.DwmGetWindowAttribute(
        hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, ctypes.byref(rect), ctypes.sizeof(rect)
    )
    return rect.left, rect.top, rect.right, rect.bottom


def client_origin(hwnd: int) -> tuple[int, int]:
    """Physical screen offset of the client area, needed to map PrintWindow
    output (client area only on this platform) onto the frame-bounds rect."""
    pt = wt.POINT(0, 0)
    user32.ClientToScreen(hwnd, ctypes.byref(pt))
    return pt.x, pt.y


def capture_window(hwnd: int) -> Image.Image:
    left, top, right, bottom = frame_bounds(hwnd)
    frame_w, frame_h = right - left, bottom - top
    cx, cy = client_origin(hwnd)

    client = wt.RECT()
    user32.GetClientRect(hwnd, ctypes.byref(client))
    w, h = client.right, client.bottom

    hdc = user32.GetWindowDC(hwnd)
    mem = gdi32.CreateCompatibleDC(hdc)
    bmp = gdi32.CreateCompatibleBitmap(hdc, w, h)
    gdi32.SelectObject(mem, bmp)
    try:
        if not user32.PrintWindow(hwnd, mem, PW_RENDERFULLCONTENT):
            raise SystemExit("PrintWindow failed")

        bi = BITMAPINFO()
        bi.bmiHeader.biSize = ctypes.sizeof(BITMAPINFOHEADER)
        bi.bmiHeader.biWidth = w
        bi.bmiHeader.biHeight = -h  # top-down rows
        bi.bmiHeader.biPlanes = 1
        bi.bmiHeader.biBitCount = 32
        buf = ctypes.create_string_buffer(w * h * 4)
        lines = gdi32.GetDIBits(mem, bmp, 0, h, buf, ctypes.byref(bi), 0)
        if lines != h:
            raise SystemExit(f"GetDIBits returned {lines} of {h} lines")
        content = Image.frombuffer("RGBA", (w, h), buf, "raw", "BGRA", 0, 1).convert("RGB")
    finally:
        gdi32.DeleteObject(bmp)
        gdi32.DeleteDC(mem)
        user32.ReleaseDC(hwnd, hdc)

    # Compose onto a frame-sized canvas so the caption bar is included when the
    # window manager draws one, and nothing is cropped at the edges.
    canvas = Image.new("RGB", (frame_w, frame_h), (4, 6, 13))
    canvas.paste(content, (cx - left, cy - top))
    return canvas


def dpi_scale(hwnd: int) -> float:
    try:
        return user32.GetDpiForWindow(hwnd) / 96.0
    except AttributeError:
        return 1.0


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--width", type=int, default=None, help="logical client width")
    ap.add_argument("--height", type=int, default=None, help="logical client height")
    ap.add_argument("--settle", type=float, default=1.2, help="seconds to wait after resize")
    args = ap.parse_args()

    # Per-monitor DPI awareness v2 is required or the rects come back scaled.
    user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))
    hwnd = find_window(TITLE)

    if args.width and args.height:
        scale = dpi_scale(hwnd)
        phys_w, phys_h = int(args.width * scale), int(args.height * scale)
        user32.SetWindowPos(
            hwnd, 0, 0, 0, phys_w, phys_h, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE
        )
        time.sleep(args.settle)

    img = capture_window(hwnd)
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    img.save(out)
    left, top, _, _ = frame_bounds(hwnd)
    print(f"{out}  {img.width}x{img.height}  at ({left},{top})  dpi x{dpi_scale(hwnd)}")


if __name__ == "__main__":
    main()
