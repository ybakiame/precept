"""Windows-side, nonce-bound WSLg window control. Run only against owned fixtures.

Only PrintWindow of the nonce-bound HWND is permitted. Input requires that exact unique window be foreground.
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
    if action in ("key", "click", "resize", "chrome"):
        u.SetForegroundWindow(hwnd)
        time.sleep(0.08)
        assert (
            u.GetForegroundWindow() == hwnd
        ), "owned WSLg window not foreground; no input/capture performed"
    if action == "chrome":
        print(json.dumps(exercise_custom_chrome(hwnd)))
        return
    if action == "key":
        u.keybd_event.argtypes = [
            ctypes.c_ubyte,
            ctypes.c_ubyte,
            ctypes.c_uint,
            ctypes.c_size_t,
        ]

        def key(vk, up=False):
            if not up:
                assert (
                    u.GetForegroundWindow() == hwnd
                ), "owned foreground lost; input stopped"
            u.keybd_event(
                vk,
                u.MapVirtualKeyW(vk, 0),
                (2 if up else 0) | (1 if 0x25 <= vk <= 0x28 else 0),
                0,
            )

        sequence = request["sequence"]
        # Closed test vocabulary, no arbitrary injection into other applications.
        special = {
            "ctrl+p": ord("P"),
            "ctrl+d": ord("D"),
            "ctrl+shift+h": ord("H"),
            "alt+Right": 0x27,
            "alt+Left": 0x25,
            "ctrl+f": ord("F"),
            "ctrl+k": ord("K"),
            "ctrl+l": ord("L"),
            "ctrl+r": ord("R"),
            "alt+1": ord("1"),
            "alt+0": ord("0"),
            "alt+a": ord("A"),
            "alt+b": ord("B"),
            "alt+c": ord("C"),
            "alt+2": ord("2"),
            "alt+3": ord("3"),
            "alt+4": ord("4"),
            "alt+5": ord("5"),
        }
        if sequence in special:
            modifier = 0x12 if sequence.startswith("alt+") else 0x11
            key(modifier)
            if "shift+" in sequence:
                key(0x10)
            try:
                key(special[sequence])
            finally:
                key(special[sequence], True)
                key(0x10, True) if "shift+" in sequence else None
                key(modifier, True)
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
    elif action == "click":
        x, y = request["position"]
        bounds = W.RECT()
        assert u.GetWindowRect(hwnd, ctypes.byref(bounds))
        assert (
            0 <= x < bounds.right - bounds.left - 64
            and 0 <= y < bounds.bottom - bounds.top - 100
        )
        point = W.POINT(bounds.left + 32 + x, bounds.top + 66 + y)
        u.WindowFromPoint.argtypes = [W.POINT]
        u.WindowFromPoint.restype = W.HWND
        u.GetAncestor.argtypes = [W.HWND, W.UINT]
        u.GetAncestor.restype = W.HWND
        assert (
            u.GetAncestor(u.WindowFromPoint(point), 2) == hwnd
        ), "owned target occluded; no pointer input"
        u.SetCursorPos(point.x, point.y)
        u.mouse_event(0x0002, 0, 0, 0, 0)
        u.mouse_event(0x0004, 0, 0, 0, 0)
    elif action == "resize":
        width, height = request["size"]
        assert (width, height) in [(1080, 760), (640, 480), (720, 600)]
        bounds = W.RECT()
        assert u.GetWindowRect(hwnd, ctypes.byref(bounds))
        # WSLg forwards a real interactive sizing transaction to Wayland.
        # SetWindowPos alone merely clips the RAIL proxy's old buffer.
        u.SetCursorPos.argtypes = [ctypes.c_int, ctypes.c_int]
        u.mouse_event.argtypes = [W.DWORD, W.DWORD, W.DWORD, W.DWORD, ctypes.c_size_t]
        frameless = request.get("frameless", False)
        edge_x, edge_y = (8, 5) if frameless else (27, 27)
        decoration = 0 if frameless else 50
        u.SetCursorPos(bounds.right - edge_x, bounds.bottom - edge_y)
        time.sleep(0.1)
        u.mouse_event(0x0002, 0, 0, 0, 0)
        time.sleep(0.1)
        try:
            start_x, start_y = bounds.right - edge_x, bounds.bottom - edge_y
            end_x, end_y = (
                bounds.left + width + decoration - edge_x,
                bounds.top + height + decoration - edge_y,
            )
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
        result = capture_owned_window(
            hwnd,
            request["output"],
            inset=(0, 0, 0, 0) if request.get("frameless") else (32, 66, 32, 34),
        )
        print(json.dumps(result))
        return
    else:
        assert action == "visible"
    print(json.dumps(dict(status="PASS")))


if __name__ == "__main__":
    main()
