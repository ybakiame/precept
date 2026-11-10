"""Exercise custom chrome on one caller-verified owned HWND, never another app."""

import ctypes
from ctypes import wintypes as W
import time


def exercise_custom_chrome(hwnd):
    u = ctypes.WinDLL("user32", use_last_error=True)
    u.SetProcessDPIAware()
    u.GetForegroundWindow.restype = W.HWND
    u.SetForegroundWindow.argtypes = [W.HWND]
    u.GetWindowRect.argtypes = [W.HWND, ctypes.POINTER(W.RECT)]
    u.IsZoomed.argtypes = [W.HWND]
    u.IsIconic.argtypes = [W.HWND]
    u.IsWindow.argtypes = [W.HWND]
    u.ShowWindow.argtypes = [W.HWND, ctypes.c_int]
    u.WindowFromPoint.argtypes = [W.POINT]
    u.WindowFromPoint.restype = W.HWND
    u.GetAncestor.argtypes = [W.HWND, W.UINT]
    u.GetAncestor.restype = W.HWND
    u.SetCursorPos.argtypes = [ctypes.c_int, ctypes.c_int]
    u.mouse_event.argtypes = [W.DWORD, W.DWORD, W.DWORD, W.DWORD, ctypes.c_size_t]

    def wait(predicate):
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            if predicate():
                return
            time.sleep(0.03)
        raise AssertionError("owned custom window operation did not complete")

    def bounds():
        r = W.RECT()
        assert u.GetWindowRect(hwnd, ctypes.byref(r))
        return r

    def point(x, y):
        u.SetForegroundWindow(hwnd)
        time.sleep(0.08)
        assert u.GetForegroundWindow() == hwnd, "owned foreground unavailable"
        r = bounds()
        p = W.POINT(r.left + x, r.top + y)
        assert u.GetAncestor(u.WindowFromPoint(p), 2) == hwnd, "owned target occluded"
        assert u.SetCursorPos(p.x, p.y)
        return p

    def click(distance_from_right):
        r = bounds()
        point(r.right - r.left - distance_from_right, 18)
        u.mouse_event(2, 0, 0, 0, 0)
        u.mouse_event(4, 0, 0, 0, 0)
        time.sleep(0.15)

    original = bounds()
    p = point(180, 18)
    u.mouse_event(2, 0, 0, 0, 0)
    try:
        for i in range(1, 11):
            u.SetCursorPos(p.x + i * 4, p.y + i * 2)
            time.sleep(0.025)
    finally:
        u.mouse_event(4, 0, 0, 0, 0)
    wait(lambda: (bounds().left, bounds().top) != (original.left, original.top))
    click(60)
    wait(lambda: u.IsZoomed(hwnd))
    click(60)
    wait(lambda: not u.IsZoomed(hwnd))
    click(100)
    wait(lambda: u.IsIconic(hwnd))
    u.ShowWindow(hwnd, 9)  # Restore only the owned synthetic window.
    wait(lambda: not u.IsIconic(hwnd))
    click(20)
    wait(lambda: not u.IsWindow(hwnd))
    return dict(
        status="PASS",
        custom_drag=True,
        maximize_restore=True,
        minimize_restore=True,
        custom_close=True,
        real_pointer=True,
    )
