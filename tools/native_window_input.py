"""Scoped test input for an explicitly owned Windows NSH window; no clipboard."""

import ctypes
import time
from ctypes import wintypes as W
from local_console_probe import windows_api


class NativeInput:
    def __init__(self, process, window):
        self.process, self.window = process, window
        self.user = windows_api()
        self.user.SendMessageW.argtypes = [W.HWND, W.UINT, W.WPARAM, W.LPARAM]
        self.user.SendMessageW.restype = ctypes.c_ssize_t
        self.user.GetClientRect.argtypes = [W.HWND, ctypes.POINTER(W.RECT)]
        self.user.GetWindowThreadProcessId.argtypes = [W.HWND, ctypes.POINTER(W.DWORD)]
        self.user.keybd_event.argtypes = [
            ctypes.c_ubyte,
            ctypes.c_ubyte,
            ctypes.c_uint,
            ctypes.c_size_t,
        ]

    def owned(self):
        owner = W.DWORD()
        self.user.GetWindowThreadProcessId(self.window, ctypes.byref(owner))
        assert self.process.poll() is None and owner.value == self.process.pid

    def click(self, x, y):
        self.owned()
        bounds = W.RECT()
        assert self.user.GetClientRect(self.window, ctypes.byref(bounds))
        assert 0 <= x < bounds.right and 0 <= y < bounds.bottom
        point = (y << 16) | x
        self.user.SendMessageW(self.window, 0x201, 1, point)
        self.user.SendMessageW(self.window, 0x202, 0, point)

    def chord(self, *keys):
        self.owned()
        self.user.SetForegroundWindow(self.window)
        time.sleep(0.05)
        assert self.user.GetForegroundWindow() == self.window
        pressed = []
        try:
            for key in keys:
                assert self.user.GetForegroundWindow() == self.window
                self.user.keybd_event(key, self.user.MapVirtualKeyW(key, 0), 0, 0)
                pressed.append(key)
        finally:
            for key in reversed(pressed):
                self.user.keybd_event(key, self.user.MapVirtualKeyW(key, 0), 2, 0)

    def type_at(self, x, y, value):
        self.click(x, y)
        time.sleep(0.1)
        self.chord(0x11, ord("A"))
        time.sleep(0.05)
        for byte in range(0, len(raw := value.encode("utf-16-le")), 2):
            self.owned()
            self.user.SendMessageW(
                self.window, 0x102, int.from_bytes(raw[byte : byte + 2], "little"), 0
            )
            time.sleep(0.01)
        time.sleep(0.2)
