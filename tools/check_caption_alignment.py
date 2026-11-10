"""Measure app-drawn caption icons in our RGB/filter-0 owned-window PNG format."""

import struct
import zlib


def caption_alignment(path):
    raw = path.read_bytes()
    assert raw.startswith(b"\x89PNG\r\n\x1a\n")
    pos, payload = 8, bytearray()
    width = height = None
    while pos < len(raw):
        size = struct.unpack(">I", raw[pos : pos + 4])[0]
        kind, data = raw[pos + 4 : pos + 8], raw[pos + 8 : pos + 8 + size]
        if kind == b"IHDR":
            width, height, bits, mode, _, _, _ = struct.unpack(">IIBBBBB", data)
            assert bits == 8 and mode == 2
        elif kind == b"IDAT":
            payload.extend(data)
        pos += size + 12
    assert width and height and height >= 36
    pixels = zlib.decompress(payload)
    stride = width * 3 + 1
    assert len(pixels) == stride * height

    def pixel(x, y):
        assert pixels[y * stride] == 0
        start = y * stride + 1 + x * 3
        return pixels[start : start + 3]

    results = []
    for name, distance in [("minimize", 100), ("maximize", 60), ("close", 20)]:
        center_x, center_y = width - distance, 18
        background = pixel(center_x - 14, 5)
        foreground = [
            (x, y)
            for y in range(5, 31)
            for x in range(center_x - 14, center_x + 14)
            if max(abs(a - b) for a, b in zip(pixel(x, y), background)) > 70
        ]
        assert foreground, "caption vector was not painted"
        xs, ys = zip(*foreground)
        measured_x, measured_y = (min(xs) + max(xs)) / 2, (min(ys) + max(ys)) / 2
        dx, dy = measured_x - center_x, measured_y - center_y
        assert abs(dx) <= 2 and abs(dy) <= 2, f"{name} icon off center: {dx}/{dy}"
        results.append(dict(icon=name, delta_x=dx, delta_y=dy))
    return dict(status="PASS", tolerance_pixels=2, measured=results)
