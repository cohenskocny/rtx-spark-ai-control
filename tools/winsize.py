"""Resize the running V2 window to a logical size (physical = size x DPI scale).

    python tools/winsize.py 980 660

The window manager keeps the client area at the requested size; the webview
reflows and the CDP capture then reports the new pixel dimensions.
"""

import ctypes
import ctypes.wintypes as wt
import sys

user32 = ctypes.windll.user32
SWP_NOMOVE = 0x0002
SWP_NOZORDER = 0x0004
SWP_NOACTIVATE = 0x0010


def main(argv: list) -> int:
    if len(argv) != 3:
        print(__doc__)
        return 2
    user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))
    hwnd = user32.FindWindowW(None, "RTX Spark AI Control")
    if not hwnd:
        raise SystemExit("window not found")
    scale = user32.GetDpiForWindow(hwnd) / 96.0
    w, h = int(int(argv[1]) * scale), int(int(argv[2]) * scale)
    # AdjustWindowRectExForDpi is unnecessary: Tauri sizes the client area, and
    # SetWindowPos on a resizable window keeps that relationship.
    user32.SetWindowPos(hwnd, 0, 0, 0, w, h, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE)
    rect = wt.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(rect))
    print(f"requested {argv[1]}x{argv[2]} logical · scale {scale} · window {rect.right - rect.left}x{rect.bottom - rect.top}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
