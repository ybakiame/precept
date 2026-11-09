"""Capture pixels from one owned HWND via PrintWindow, never the desktop DC.

Callers must verify ownership (process ID or a nonce-bound synthetic title).
An unsupported/blank target fails; there is deliberately no screen-copy fallback.
"""

import ctypes
from ctypes import wintypes as W
import struct
import zlib
from pathlib import Path


def capture_owned_window(hwnd, output, *, inset=(0, 0, 0, 0)):
    u = ctypes.WinDLL("user32", use_last_error=True)
    g = ctypes.WinDLL("gdi32", use_last_error=True)
    u.GetWindowRect.argtypes = [W.HWND, ctypes.POINTER(W.RECT)]
    u.GetWindowDC.argtypes = [W.HWND]
    u.GetWindowDC.restype = W.HDC
    u.ReleaseDC.argtypes = [W.HWND, W.HDC]
    u.PrintWindow.argtypes = [W.HWND, W.HDC, W.UINT]
    g.CreateCompatibleDC.argtypes = [W.HDC]
    g.CreateCompatibleDC.restype = W.HDC
    g.CreateCompatibleBitmap.argtypes = [W.HDC, ctypes.c_int, ctypes.c_int]
    g.CreateCompatibleBitmap.restype = W.HBITMAP
    g.SelectObject.argtypes = [W.HDC, W.HANDLE]
    g.SelectObject.restype = W.HANDLE
    g.DeleteObject.argtypes = [W.HANDLE]
    g.DeleteDC.argtypes = [W.HDC]

    class Header(ctypes.Structure):
        _fields_ = [
            ("size", W.DWORD),
            ("width", W.LONG),
            ("height", W.LONG),
            ("planes", W.WORD),
            ("bits", W.WORD),
            ("compression", W.DWORD),
            ("image_size", W.DWORD),
            ("xppm", W.LONG),
            ("yppm", W.LONG),
            ("colors", W.DWORD),
            ("important", W.DWORD),
        ]

    g.GetDIBits.argtypes = [
        W.HDC,
        W.HBITMAP,
        W.UINT,
        W.UINT,
        W.LPVOID,
        ctypes.POINTER(Header),
        W.UINT,
    ]
    bounds = W.RECT()
    assert u.GetWindowRect(
        hwnd, ctypes.byref(bounds)
    ), "owned window bounds unavailable"
    width, height = bounds.right - bounds.left, bounds.bottom - bounds.top
    assert 64 <= width <= 8192 and 64 <= height <= 8192
    dc = u.GetWindowDC(hwnd)
    memory = g.CreateCompatibleDC(dc)
    bitmap = g.CreateCompatibleBitmap(dc, width, height)
    assert dc and memory and bitmap
    previous = g.SelectObject(memory, bitmap)
    try:
        assert u.PrintWindow(
            hwnd, memory, 2
        ), "target PrintWindow unsupported; no desktop fallback"
        g.SelectObject(memory, previous)
        previous = None
        pixels = ctypes.create_string_buffer(width * height * 4)
        header = Header(ctypes.sizeof(Header), width, -height, 1, 32, 0, 0, 0, 0, 0, 0)
        assert (
            g.GetDIBits(memory, bitmap, 0, height, pixels, ctypes.byref(header), 0)
            == height
        )
        left, top, right, bottom = inset
        cw, ch = width - left - right, height - top - bottom
        assert cw > 0 and ch > 0
        bgra = pixels.raw
        rgb = bytearray()
        for y in range(top, height - bottom):
            row = bgra[(y * width + left) * 4 : (y * width + width - right) * 4]
            rgb.extend(b"\0")
            for i in range(0, len(row), 4):
                rgb.extend((row[i + 2], row[i + 1], row[i]))
        assert len(set(rgb)) > 16, "blank target window; no desktop fallback"

        def chunk(kind, data):
            return (
                struct.pack(">I", len(data))
                + kind
                + data
                + struct.pack(">I", zlib.crc32(kind + data))
            )

        png = b"\x89PNG\r\n\x1a\n" + chunk(
            b"IHDR", struct.pack(">IIBBBBB", cw, ch, 8, 2, 0, 0, 0)
        )
        png += chunk(b"IDAT", zlib.compress(rgb)) + chunk(b"IEND", b"")
        Path(output).write_bytes(png)
        return dict(
            status="PASS",
            width=cw,
            height=ch,
            nonblank=True,
            capture="owned HWND PrintWindow; no desktop pixels",
        )
    finally:
        if previous:
            g.SelectObject(memory, previous)
        g.DeleteObject(bitmap)
        g.DeleteDC(memory)
        u.ReleaseDC(hwnd, dc)
