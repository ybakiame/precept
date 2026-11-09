"""Windows-side, nonce-bound WSLg window control. Run only against owned fixtures.

No whole-desktop capture. Input requires that exact unique window be foreground.
Stdout contains fixed metadata, never arbitrary window titles or user content.
"""

import ctypes
from ctypes import wintypes as W
import json
import sys
import time


def main():
    request = json.loads(sys.argv[1])
    nonce = request["nonce"]
    assert len(nonce) == 16 and all(c in "0123456789abcdef" for c in nonce)
    title = "Agent Control Plane — read-only console [" + nonce + "]"
    u = ctypes.WinDLL("user32", use_last_error=True)
    u.SetProcessDPIAware()
    u.GetForegroundWindow.restype = W.HWND
    u.IsWindowVisible.argtypes = [W.HWND]
    u.GetWindowTextW.argtypes = [W.HWND, W.LPWSTR, ctypes.c_int]
    u.SetForegroundWindow.argtypes = [W.HWND]
    u.GetWindowRect.argtypes = [W.HWND, ctypes.POINTER(W.RECT)]
    u.SetWindowPos.argtypes = [
        W.HWND,
        W.HWND,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_uint,
    ]
    u.PostMessageW.argtypes = [W.HWND, ctypes.c_uint, W.WPARAM, W.LPARAM]
    found = []
    callback = ctypes.WINFUNCTYPE(W.BOOL, W.HWND, W.LPARAM)

    def visit(hwnd, _):
        value = ctypes.create_unicode_buffer(1024)
        u.GetWindowTextW(hwnd, value, 1024)
        if u.IsWindowVisible(hwnd) and (
            value.value == title
            or (value.value.startswith(title + " (") and value.value.endswith(")"))
        ):
            found.append(hwnd)
        return True

    deadline = time.monotonic() + 8
    while not found and time.monotonic() < deadline:
        u.EnumWindows(callback(visit), 0)
        if not found:
            time.sleep(0.05)
    assert len(found) == 1, "owned WSLg window not uniquely visible"
    hwnd = found[0]
    action = request["action"]
    if action in ("key", "capture", "resize"):
        u.SetForegroundWindow(hwnd)
        time.sleep(0.08)
        assert (
            u.GetForegroundWindow() == hwnd
        ), "owned WSLg window not foreground; no input/capture performed"
    if action == "key":
        u.keybd_event.argtypes = [
            ctypes.c_ubyte,
            ctypes.c_ubyte,
            ctypes.c_uint,
            ctypes.c_size_t,
        ]

        def key(vk, up=False):
            u.keybd_event(vk, 0, 2 if up else 0, 0)

        sequence = request["sequence"]
        # Closed test vocabulary, no arbitrary injection into other applications.
        special = {
            "ctrl+p": ord("P"),
            "ctrl+f": ord("F"),
            "ctrl+k": ord("K"),
            "ctrl+r": ord("R"),
            "ctrl+1": ord("1"),
            "ctrl+2": ord("2"),
            "ctrl+3": ord("3"),
            "ctrl+4": ord("4"),
            "ctrl+5": ord("5"),
        }
        if sequence in special:
            key(0x11)
            key(special[sequence])
            key(special[sequence], True)
            key(0x11, True)
        elif sequence in ("Tab", "Enter", "Escape"):
            vk = {"Tab": 9, "Enter": 13, "Escape": 27}[sequence]
            key(vk)
            key(vk, True)
        else:
            assert sequence in (
                "event-09999",
                "nomatch",
                "shared",
                "missing",
                "quality-task",
            )
            for c in sequence:
                vk = 0xBD if c == "-" else ord(c.upper())
                key(vk)
                key(vk, True)
                time.sleep(0.025)
    elif action == "resize":
        width, height = request["size"]
        assert (width, height) in [(1080, 760), (640, 480), (720, 600)]
        bounds = W.RECT()
        assert u.GetWindowRect(hwnd, ctypes.byref(bounds))
        # WSLg forwards a real interactive sizing transaction to Wayland.
        # SetWindowPos alone merely clips the RAIL proxy's old buffer.
        u.SetCursorPos.argtypes = [ctypes.c_int, ctypes.c_int]
        u.mouse_event.argtypes = [W.DWORD, W.DWORD, W.DWORD, W.DWORD, ctypes.c_size_t]
        u.SetCursorPos(bounds.right - 27, bounds.bottom - 27)
        time.sleep(0.1)
        u.mouse_event(0x0002, 0, 0, 0, 0)
        time.sleep(0.1)
        try:
            start_x, start_y = bounds.right - 27, bounds.bottom - 27
            end_x, end_y = bounds.left + width + 50 - 27, bounds.top + height + 50 - 27
            for step in range(1, 11):
                u.SetCursorPos(
                    start_x + (end_x - start_x) * step // 10,
                    start_y + (end_y - start_y) * step // 10,
                )
                time.sleep(0.025)
            time.sleep(0.1)
        finally:
            u.mouse_event(0x0004, 0, 0, 0, 0)
        time.sleep(0.2)
    elif action == "close":
        assert u.PostMessageW(hwnd, 0x0010, 0, 0)
    elif action == "capture":
        import mss, mss.tools

        bounds = W.RECT()
        assert u.GetWindowRect(hwnd, ctypes.byref(bounds))
        with mss.MSS() as capture:
            # WSLg RAIL bounds include transparent GTK CSD shadows. Capture
            # only the opaque content interior, excluding titlebar/corners/shadows.
            frame = capture.grab(
                dict(
                    left=bounds.left + 32,
                    top=bounds.top + 66,
                    width=bounds.right - bounds.left - 64,
                    height=bounds.bottom - bounds.top - 100,
                )
            )
            assert len(set(frame.rgb[::3])) > 8, "blank WSLg frame"
            mss.tools.to_png(frame.rgb, frame.size, output=request["output"])
        print(
            json.dumps(
                dict(
                    status="PASS", width=frame.width, height=frame.height, nonblank=True
                )
            )
        )
        return
    else:
        assert action == "visible"
    print(json.dumps(dict(status="PASS")))


if __name__ == "__main__":
    main()
