"""Check for a solid probe color in the preview half of mdedit's 8-bit RGB shot."""
import pathlib
import struct
import sys
import zlib


def probe_count(path):
    png = pathlib.Path(path).read_bytes()
    assert png[:8] == b"\x89PNG\r\n\x1a\n"
    w, h = struct.unpack_from(">II", png, 16)
    assert png[24:29] == bytes([8, 2, 0, 0, 0]), "expected non-interlaced RGB"
    at, compressed = 8, bytearray()
    while at < len(png):
        size, kind = struct.unpack_from(">I4s", png, at)
        if kind == b"IDAT":
            compressed.extend(png[at + 8 : at + 8 + size])
        at += size + 12
    data = zlib.decompress(compressed)
    stride = 3 * w
    assert len(data) == h * (stride + 1)
    previous = bytearray(stride)
    count = 0
    for at in range(0, len(data), stride + 1):
        mode = data[at]
        assert mode in range(5)
        row = bytearray(data[at + 1 : at + stride + 1])
        if mode:
            for x in range(stride):
                a = row[x - 3] if x >= 3 else 0
                b = previous[x]
                c = previous[x - 3] if x >= 3 else 0
                if mode == 1:
                    predictor = a
                elif mode == 2:
                    predictor = b
                elif mode == 3:
                    predictor = (a + b) // 2
                else:
                    pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                    predictor = a if pa <= min(pb, pc) else b if pb <= pc else c
                row[x] = (row[x] + predictor) & 255
        count += sum(row[x : x + 3] == b"\xdf\x15\xac" for x in range(3 * (w // 2), stride, 3))
        previous = row
    return count


with_engine, without_engine = map(probe_count, sys.argv[1:])
assert with_engine >= 100, f"the installed engine did not draw the SVG: {with_engine} probe pixels"
assert without_engine == 0, f"SVG rendered without its sibling helper: {without_engine} probe pixels"
print(f"Installed preview engine: {with_engine} probe pixels; without sibling: zero")
