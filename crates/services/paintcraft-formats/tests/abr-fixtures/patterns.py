"""Owned pattern records with proper VMAL extents and exact source channels."""
import struct


def pattern(mode, name, identifier, slots, bounds=(5, 7, 7, 9), extra=b"\0" * 4):
    encoded = (name + "\0").encode("utf-16be")
    identifier = identifier.encode("ascii")
    body = struct.pack(">IIhhI", 1, mode, -3, 4, len(encoded) // 2) + encoded
    body += bytes([len(identifier)]) + identifier
    if mode == 2:
        palette = [(11, 22, 33), (200, 100, 50)] + [(0, 0, 0)] * 254
        body += b"".join(bytes(color) for color in palette) + extra
    arrays = struct.pack(">4II", *bounds, len(slots) - 2)
    for slot in slots:
        if slot is None:
            arrays += struct.pack(">I", 0)
            continue
        if slot == "empty":
            arrays += struct.pack(">II", 1, 0)
            continue
        depth, compression, region, values = slot
        code = {8: "B", 16: "H", 32: "f"}[depth]
        raw = struct.pack(">" + code * len(values), *values)
        width = region[3] - region[1]
        height = region[2] - region[0]
        assert len(raw) == width * height * (depth // 8)
        data = raw
        if compression == 1:
            rows = []
            for y in range(height):
                row = raw[y * width * (depth // 8):(y + 1) * width * (depth // 8)]
                assert 0 < len(row) <= 128
                rows.append(b"\x80" + bytes([len(row) - 1]) + row + b"\x80")
            data = struct.pack(">" + "H" * height, *(len(row) for row in rows)) + b"".join(rows)
        channel = struct.pack(">I4IHB", depth, *region, depth, compression) + data
        arrays += struct.pack(">II", 1, len(channel)) + channel
    body += struct.pack(">II", 3, len(arrays)) + arrays
    return struct.pack(">I", len(body)) + body + b"\0" * (-len(body) % 4)


def fixtures():
    box = (5, 7, 7, 9)
    rgb = pattern(3, "Pattern 色 🚀", "rgb8", [
        (8, 0, box, [10, 20, 30, 40]), (8, 1, box, [100, 110, 120, 130]),
        (8, 0, box, [200, 210, 220, 230]), None, (8, 1, box, [0, 64, 128, 255])])
    gray = pattern(1, "Gray16", "gray16", [(16, 1, box, [0, 255, 32769, 65535]),
        "empty", (16, 0, box, [0, 1, 32768, 65535])])
    indexed_slots = [(8, 1, box, [0, 1, 1, 0]), None, (8, 0, box, [255, 128, 64, 0])]
    indexed = pattern(2, "Indexed", "indexed8", indexed_slots)
    floating = pattern(3, "Float32", "float32", [
        (32, 0, box, [-0.0, 0.5, 1.5, -0.25]), (32, 1, box, [0.25, 0.75, 1.0, 0.0]),
        (32, 0, box, [1.0, 0.0, 0.125, 0.875]), None, (8, 1, box, [255, 128, 64, 0])])
    partial = pattern(3, "Partial", "partial", [
        (8, 0, (5, 8, 7, 10), [1, 2, 3, 4]), (8, 0, (5, 7, 7, 10), [10, 20, 30, 40, 50, 60]),
        (8, 1, (5, 7, 7, 10), [100, 110, 120, 130, 140, 150]), None,
        (8, 0, (5, 8, 7, 10), [0, 64, 128, 255])], bounds=(5, 7, 7, 10))
    return {
        "patterns": rgb + gray + indexed + floating + partial,
        "indexed-no-extra": pattern(2, "Indexed no extra", "indexed-no-extra", indexed_slots, extra=b""),
        "indexed-extra3": pattern(2, "Indexed extra three", "indexed-extra3", indexed_slots, extra=b"\0\0\0\3"),
        "auxiliary": pattern(3, "Auxiliary retained", "auxiliary", [
            (8, 0, box, [10, 20, 30, 40]), (8, 0, box, [100, 110, 120, 130]),
            (8, 0, box, [200, 210, 220, 230]), (16, 1, box, [1, 255, 32769, 65535]),
            None, (8, 0, box, [0, 64, 128, 255])]),
        "missing-color": pattern(3, "Missing red", "missing-color", [None,
            (8, 0, box, [100, 110, 120, 130]), (8, 0, box, [200, 210, 220, 230]),
            None, (8, 0, box, [0, 64, 128, 255])]),
        "empty-color": pattern(3, "Empty red", "empty-color", ["empty",
            (8, 0, box, [100, 110, 120, 130]), (8, 0, box, [200, 210, 220, 230]),
            None, (8, 0, box, [0, 64, 128, 255])]),
        "user-mask": pattern(3, "User mask retained", "user-mask", [
            (8, 0, box, [10, 20, 30, 40]), (8, 0, box, [100, 110, 120, 130]),
            (8, 0, box, [200, 210, 220, 230]), (8, 0, box, [255, 128, 64, 0]),
            (8, 0, box, [0, 64, 128, 255])]),
    }
