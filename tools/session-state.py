"""Report whether the interactive session is locked and whether displays are on,
without touching anything."""
import ctypes, ctypes.wintypes as wt, subprocess

user32 = ctypes.windll.user32
kernel32 = ctypes.windll.kernel32

# Windows 10+: lock screen present when LogonUI is running.
out = subprocess.run(["powershell", "-NoProfile", "-Command",
                      "(Get-Process LogonUI -ErrorAction SilentlyContinue | Measure-Object).Count"],
                     capture_output=True, text=True)
print("logonui_count", out.stdout.strip())

# Monitor power state via GetDevicePowerState on the display device is awkward;
# instead report the console session input desktop name.
h = user32.OpenInputDesktop(0, False, 0x0100)
print("input_desktop_handle", h)
if h:
    buf = ctypes.create_unicode_buffer(256)
    need = wt.DWORD()
    user32.GetUserObjectInformationW(h, 2, buf, 512, ctypes.byref(need))
    print("input_desktop_name", buf.value)
    user32.CloseDesktop(h)

# Grab a tiny screen sample to see whether anything renders at all.
from PIL import ImageGrab
img = ImageGrab.grab(all_screens=True)
print("screen", img.size, "extrema", img.convert("RGB").getextrema())
