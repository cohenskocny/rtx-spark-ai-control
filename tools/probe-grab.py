"""Diagnostic: full-screen grab cropped to the app window rect, to tell a blank
window apart from a failed window-only capture."""
import ctypes, ctypes.wintypes as wt
from PIL import Image, ImageGrab

user32 = ctypes.windll.user32
user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))

hwnd = user32.FindWindowW(None, "RTX Spark AI Control")
print("hwnd", hwnd)
rect = wt.RECT()
user32.GetWindowRect(hwnd, ctypes.byref(rect))
print("rect", rect.left, rect.top, rect.right, rect.bottom)

img = ImageGrab.grab(all_screens=True)
print("screen", img.size, "mode", img.mode)
print("extrema", img.convert("RGB").getextrema())
crop = img.crop((rect.left, rect.top, rect.right, rect.bottom))
print("crop size", crop.size, "extrema", crop.convert("RGB").getextrema())
crop.save("results/probe-01-fullscreen-crop.png")
img.resize((img.width // 3, img.height // 3)).save("results/probe-02-screen-third.png")
