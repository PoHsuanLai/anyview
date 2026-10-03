#!/usr/bin/env python3
"""Generate the TrueType fixtures of `anyview-font`'s tests.

Every font is synthesized from scratch: a fixed skeleton (head, hhea, hmtx, maxp, loca, glyf, post,
cmap, name) whose glyphs are plain rectangles, so there is nothing to license. The skeleton follows
the generator of pdfrum's `pdfrum-font` fixtures (same authors, MIT OR Apache-2.0).

    python3 crates/anyview-font/tests/fixtures/make.py [OUT_DIR]

Output is deterministic. `blocks.ttf` maps Latin letters, digits and a few signs, each to a
rectangle of its own width and height; `circled.ttf` maps only U+2460..U+2469, so none of the
specimen's sample letters.
"""

from __future__ import annotations

import struct
import sys
from pathlib import Path

UNITS_PER_EM = 1000
SAMPLE = (
    "ABCDEFGHIJKLMNOPQRSTUVWXYZ" "abcdefghijklmnopqrstuvwxyz" "0123456789" " &?!@"
)


def pad4(data: bytes) -> bytes:
    return data + b"\0" * (-len(data) % 4)


def checksum(data: bytes) -> int:
    total = 0
    for (word,) in struct.iter_unpack(">I", pad4(data)):
        total = (total + word) & 0xFFFFFFFF
    return total


def rectangle(width: int, height: int) -> bytes:
    """One contour of four on-curve points: a filled rectangle with a 40-unit side bearing."""
    xs = [40, 40 + width, 40 + width, 40]
    ys = [0, 0, height, height]
    header = struct.pack(">hhhhh", 1, min(xs), min(ys), max(xs), max(ys))
    body = struct.pack(">H", 3) + struct.pack(">H", 0) + bytes([1, 1, 1, 1])
    prev = 0
    for x in xs:
        body += struct.pack(">h", x - prev)
        prev = x
    prev = 0
    for y in ys:
        body += struct.pack(">h", y - prev)
        prev = y
    return pad4(header + body)


def shape(index: int) -> tuple[int, int, int]:
    """Width, height and advance of glyph `index` (0 is .notdef)."""
    width = 300 + (index % 5) * 60
    height = 500 + (index % 7) * 30
    return width, height, width + 120


def cmap_format4(mapping: dict[int, int]) -> bytes:
    segments: list[tuple[int, int, int]] = []
    for code in sorted(mapping):
        delta = (mapping[code] - code) & 0xFFFF
        if segments and segments[-1][1] == code - 1 and segments[-1][2] == delta:
            segments[-1] = (segments[-1][0], code, delta)
        else:
            segments.append((code, code, delta))
    segments.append((0xFFFF, 0xFFFF, 1))
    seg_x2 = len(segments) * 2
    search, selector = 2, 0
    while search * 2 <= seg_x2:
        search *= 2
        selector += 1
    arrays = struct.pack(">HHHH", seg_x2, search, selector, seg_x2 - search)
    arrays += b"".join(struct.pack(">H", end) for _, end, _ in segments)
    arrays += struct.pack(">H", 0)
    arrays += b"".join(struct.pack(">H", start) for start, _, _ in segments)
    arrays += b"".join(
        struct.pack(">h", d - 0x10000 if d > 0x7FFF else d) for _, _, d in segments
    )
    arrays += b"".join(struct.pack(">H", 0) for _ in segments)
    return struct.pack(">HHH", 4, 6 + len(arrays), 0) + arrays


def make_cmap(subtable: bytes) -> bytes:
    return struct.pack(">HH", 0, 1) + struct.pack(">HHI", 3, 1, 12) + subtable


def make_name(family: str, style: str) -> bytes:
    """Family (id 1) and subfamily (id 2) as Windows Unicode BMP English records."""
    records = [(1, family.encode("utf-16-be")), (2, style.encode("utf-16-be"))]
    storage = 6 + 12 * len(records)
    out = struct.pack(">HHH", 0, len(records), storage)
    offset = 0
    for name_id, value in records:
        out += struct.pack(">HHHHHH", 3, 1, 0x0409, name_id, len(value), offset)
        offset += len(value)
    return pad4(out + b"".join(value for _, value in records))


def make_font(chars: list[int], family: str) -> bytes:
    count = len(chars) + 1  # glyph 0 is .notdef
    glyf = bytearray()
    offsets = [0]
    hmtx = b""
    for index in range(count):
        width, height, advance = shape(index)
        glyf += rectangle(width, height)
        offsets.append(len(glyf))
        hmtx += struct.pack(">Hh", advance, 40)
    loca = b"".join(struct.pack(">H", off // 2) for off in offsets)
    head = struct.pack(
        ">IIIIHHqqhhhhHHhhh",
        0x00010000, 0x00010000, 0, 0x5F0F3CF5, 0b11, UNITS_PER_EM, 0, 0,
        0, 0, 900, 700, 0, 8, 2, 0, 0,
    )
    hhea = struct.pack(
        ">IhhhHhhhhhhhhhhhH", 0x00010000, 800, -200, 0, 900, 0, 0, 900, 1, 0, 0, 0, 0, 0, 0, 0, count
    )
    maxp = struct.pack(">IHHHHHHHHHHHHHH", 0x00010000, count, 4, 1, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0)
    post = struct.pack(">IIhhIIIII", 0x00030000, 0, -100, 50, 0, 0, 0, 0, 0)
    mapping = {code: gid + 1 for gid, code in enumerate(chars)}
    tables = {
        "head": head, "hhea": hhea, "hmtx": hmtx, "maxp": maxp, "loca": loca,
        "glyf": bytes(glyf), "post": post, "cmap": make_cmap(cmap_format4(mapping)),
        "name": make_name(family, "Regular"),
    }
    return build_sfnt(tables)


def build_sfnt(tables: dict[str, bytes]) -> bytes:
    tags = sorted(tables)
    n = len(tags)
    search, selector = 16, 0
    while search * 2 <= n * 16:
        search *= 2
        selector += 1
    header = struct.pack(">IHHHH", 0x00010000, n, search, selector, n * 16 - search)
    directory, body = bytearray(), bytearray()
    offset = len(header) + 16 * n
    head_at = 0
    for tag in tags:
        data = tables[tag]
        if tag == "head":
            head_at = len(body)
        directory += struct.pack(">4sIII", tag.encode(), checksum(data), offset + len(body), len(data))
        body += pad4(data)
    font = bytearray(header + bytes(directory) + bytes(body))
    adjust = len(header) + 16 * n + head_at + 8
    struct.pack_into(">I", font, adjust, 0)
    struct.pack_into(">I", font, adjust, (0xB1B0AFBA - checksum(bytes(font))) & 0xFFFFFFFF)
    return bytes(font)


def main(argv: list[str]) -> int:
    out = Path(argv[1]) if len(argv) > 1 else Path(__file__).resolve().parent
    (out / "blocks.ttf").write_bytes(make_font([ord(c) for c in SAMPLE], "Anyview Blocks"))
    (out / "circled.ttf").write_bytes(make_font(list(range(0x2460, 0x246A)), "Anyview Circled"))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
