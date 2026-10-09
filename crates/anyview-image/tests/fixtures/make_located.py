#!/usr/bin/env python3
"""Writes located.jpg, located.png and located.heic: tiny pictures whose EXIF carries every fact the viewer's
Info panel lists, including a place and the camera's serial numbers (which no pane may show).

Standard library only. Run from this folder: python3 make_located.py
The base picture is plain.jpg (no metadata), so only the EXIF block is made here.
"""
import struct
import zlib

BYTE, ASCII, SHORT, LONG, RATIONAL, SRATIONAL = 1, 2, 3, 4, 5, 10


def ascii_(text):
    return (ASCII, text.encode() + b"\0")


def short(*values):
    return (SHORT, struct.pack("<%dH" % len(values), *values))


def long_(value):
    return (LONG, struct.pack("<I", value))


def rational(*pairs):
    return (RATIONAL, b"".join(struct.pack("<II", n, d) for n, d in pairs))


def srational(*pairs):
    return (SRATIONAL, b"".join(struct.pack("<ii", n, d) for n, d in pairs))


def byte(*values):
    return (BYTE, bytes(values))


SIZE = {BYTE: 1, ASCII: 1, SHORT: 2, LONG: 4, RATIONAL: 8, SRATIONAL: 8}


def build_ifd(entries, offset):
    """The bytes of one IFD placed at `offset`; values over four bytes follow its table."""
    entries = sorted(entries, key=lambda e: e[0])
    table = struct.pack("<H", len(entries))
    data = b""
    data_at = offset + 2 + 12 * len(entries) + 4
    for tag, (kind, payload) in entries:
        count = len(payload) // SIZE[kind]
        if len(payload) <= 4:
            inline = payload.ljust(4, b"\0")
        else:
            inline = struct.pack("<I", data_at + len(data))
            data += payload + (b"\0" if len(payload) % 2 else b"")
        table += struct.pack("<HHI", tag, kind, count) + inline
    return table + struct.pack("<I", 0) + data


def exif_block():
    gps = [
        (0x0000, byte(2, 3, 0, 0)),
        (0x0001, ascii_("N")),
        (0x0002, rational((37, 1), (46, 1), (2964, 100))),
        (0x0003, ascii_("W")),
        (0x0004, rational((122, 1), (25, 1), (984, 100))),
        (0x0005, byte(0)),
        (0x0006, rational((12, 1))),
    ]
    exif = [
        (0x829A, rational((1, 200))),
        (0x829D, rational((28, 10))),
        (0x8827, short(400)),
        (0x9003, ascii_("2024:05:01 12:30:45")),
        (0x9204, srational((-2, 3))),
        (0x9209, short(0x19)),
        (0x920A, rational((35, 1))),
        (0xA431, ascii_("SN-BODY-123456")),
        (0xA434, ascii_("TestLens 35mm f/2")),
        (0xA435, ascii_("SN-LENS-654321")),
    ]
    zero = long_(0)
    ifd0 = lambda exif_at, gps_at: [
        (0x010F, ascii_("TestCam")),
        (0x0110, ascii_("Located One")),
        (0x0112, short(1)),
        (0x011A, rational((300, 1))),
        (0x011B, rational((300, 1))),
        (0x0128, short(2)),
        (0x0131, ascii_("TestSoft 2.1")),
        (0x8298, ascii_("(c) Test Photographer")),
        (0x8769, long_(exif_at)),
        (0x8825, long_(gps_at)),
    ]
    first = len(build_ifd(ifd0(0, 0), 8))
    exif_at = 8 + first
    second = len(build_ifd(exif, exif_at))
    gps_at = exif_at + second
    return b"II*\0" + struct.pack("<I", 8) + build_ifd(ifd0(exif_at, gps_at), 8) + build_ifd(exif, exif_at) + build_ifd(gps, gps_at)


def jpeg(exif):
    base = open("plain.jpg", "rb").read()
    assert base[:2] == b"\xff\xd8" and b"Exif" not in base
    body = b"Exif\0\0" + exif
    return base[:2] + b"\xff\xe1" + struct.pack(">H", len(body) + 2) + body + base[2:]


def chunk(kind, data):
    crc = zlib.crc32(kind + data) & 0xFFFFFFFF
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", crc)


def png(exif):
    header = struct.pack(">IIBBBBB", 2, 2, 8, 2, 0, 0, 0)
    rows = b"".join(b"\0" + bytes([255, 0, 0, 0, 255, 0]) for _ in range(2))
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"pHYs", struct.pack(">IIB", 11811, 11811, 1))
        + chunk(b"eXIf", exif)
        + chunk(b"IDAT", zlib.compress(rows))
        + chunk(b"IEND", b"")
    )


def box(kind, body):
    return struct.pack(">I", len(body) + 8) + kind + body


def heic(exif):
    """A HEIF container with an Exif item and no picture: all the reader of metadata needs."""
    item = struct.pack(">I", 6) + b"Exif\0\0" + exif
    infe = box(b"infe", struct.pack(">I", 2 << 24) + struct.pack(">HH", 1, 0) + b"Exif" + b"\0")
    iinf = box(b"iinf", struct.pack(">I", 0) + struct.pack(">H", 1) + infe)
    iloc = box(
        b"iloc",
        struct.pack(">I", 1 << 24)
        + struct.pack(">H", 0x4400)  # offsets and lengths of four bytes, no base offset
        + struct.pack(">H", 1)  # one item
        + struct.pack(">HHH", 1, 1, 0)  # item 1, built from the item data box, no data reference
        + struct.pack(">H", 1)  # one extent
        + struct.pack(">II", 0, len(item)),
    )
    meta = box(b"meta", struct.pack(">I", 0) + iinf + iloc + box(b"idat", item))
    return box(b"ftyp", b"heic" + struct.pack(">I", 0) + b"mif1heic") + meta


if __name__ == "__main__":
    block = exif_block()
    open("located.jpg", "wb").write(jpeg(block))
    open("located.png", "wb").write(png(block))
    open("located.heic", "wb").write(heic(block))
