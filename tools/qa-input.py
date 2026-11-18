#!/usr/bin/env python3
"""qa-input.py — inject a tiny mouse nudge (and optional key press) via SendInput.

Used to prove the blackout guard releases on real user input, and to query the
system idle time so we can prove it *arms* after the configured threshold.

  qa-input.py idle        -> print "IDLE_S <seconds since last input>"
  qa-input.py nudge       -> move the mouse 1px back and forth (real input event)
  qa-input.py key         -> send a harmless VK_CONTROL tap
"""
import ctypes
import sys
import time
from ctypes import wintypes

user32 = ctypes.windll.user32

INPUT_MOUSE = 0
INPUT_KEYBOARD = 1
MOUSEEVENTF_MOVE = 0x0001

KEYEVENTF_KEYUP = 0x0002
VK_CONTROL = 0x11


class MOUSEINPUT(ctypes.Structure):
    _fields_ = [
        ("dx", wintypes.LONG),
        ("dy", wintypes.LONG),
        ("mouseData", wintypes.DWORD),
        ("dwFlags", wintypes.DWORD),
        ("time", wintypes.DWORD),
        ("dwExtraInfo", ctypes.POINTER(ctypes.c_ulong)),
    ]


class KEYBDINPUT(ctypes.Structure):
    _fields_ = [
        ("wVk", wintypes.WORD),
        ("wScan", wintypes.WORD),
        ("dwFlags", wintypes.DWORD),
        ("time", wintypes.DWORD),
        ("dwExtraInfo", ctypes.POINTER(ctypes.c_ulong)),
    ]


class INPUT(ctypes.Structure):
    class _I(ctypes.Union):
        _fields_ = [("mi", MOUSEINPUT), ("ki", KEYBDINPUT)]

    _fields_ = [("type", wintypes.DWORD), ("i", _I)]


LASTINPUTINFO = None


class LASTINPUTINFO(ctypes.Structure):
    _fields_ = [("cbSize", wintypes.UINT), ("dwTime", wintypes.DWORD)]


def idle_seconds() -> float:
    info = LASTINPUTINFO()
    info.cbSize = ctypes.sizeof(info)
    if not user32.GetLastInputInfo(ctypes.byref(info)):
        raise ctypes.WinError()
    millis = ctypes.windll.kernel32.GetTickCount() - info.dwTime
    return millis / 1000.0


def nudge_mouse() -> None:
    for dy in (-1, 1):
        inp = INPUT()
        inp.type = INPUT_MOUSE
        inp.i.mi = MOUSEINPUT(0, dy, 0, MOUSEEVENTF_MOVE, 0, None)
        if user32.SendInput(1, ctypes.byref(inp), ctypes.sizeof(INPUT)) != 1:
            raise ctypes.WinError()
        time.sleep(0.05)


def tap_key() -> None:
    for flags in (0, KEYEVENTF_KEYUP):
        inp = INPUT()
        inp.type = INPUT_KEYBOARD
        inp.i.ki = KEYBDINPUT(VK_CONTROL, 0, flags, 0, None)
        user32.SendInput(1, ctypes.byref(inp), ctypes.sizeof(INPUT))
        time.sleep(0.03)


def main() -> int:
    cmd = sys.argv[1] if len(sys.argv) > 1 else "idle"
    if cmd == "idle":
        print(f"IDLE_S {idle_seconds():.1f}")
    elif cmd == "nudge":
        nudge_mouse()
        print(f"NUDGED ok idle={idle_seconds():.2f}")
    elif cmd == "key":
        tap_key()
        print(f"KEYED ok idle={idle_seconds():.2f}")
    else:
        print(__doc__)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
