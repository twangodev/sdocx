#!/usr/bin/env python3
"""Replay the captured InkPen V4 alpha coverage with scalar fine derivatives.

This checks one committed 128x64 Mesa Drawing capture. It does not claim that
the observed derivative grouping is universal across GLES implementations.
"""

import base64
import hashlib
import json
import math
import struct
import zlib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "conformance" / "inkpen-joined-paint-native.json"
CASE_ID = "inkpen_v4_black_1920x1080_to_128x64"
EXPECTED_FBO3_SHA256 = "de57faa6e7261c4f7c818b01fc5803ce072c30ca7b28d0e5c55fba27a25f75dd"
INKPEN_DSO_SHA256 = "939a7c719838306d693f96701994486b03f5c7eea35f16f25c6a89d84daba401"
ALPHA_SHADER_SHA256 = "25cc41115c3f7f6025151633e50da4c361808a34033db56640163af5d6e5a299"


def _f32_from_le_hex(word):
    raw = bytes.fromhex(word)
    if len(raw) != 4:
        raise ValueError(f"expected one little-endian f32 word, got {word!r}")
    return struct.unpack("<f", raw)[0]


def _load_case():
    document = json.loads(FIXTURE.read_text())
    if document.get("schema") != "inkpen-joined-paint-fixture-v1":
        raise ValueError("unexpected joined-paint fixture schema")
    case = document["cases"][CASE_ID]
    if case["source_caller"]["libraries"].get("InkPen") != INKPEN_DSO_SHA256:
        raise ValueError("joined-paint input is not from the pinned InkPen DSO")
    stream = case["source_caller"]["streams"]["draw_object_stroke"]
    if stream["callback_count"] != 26 or stream["float_count"] != 78:
        raise ValueError("captured Drawing stream is not the expected 26 triples")
    words = stream["values_f32_bits"]
    if len(words) != 78:
        raise ValueError("captured Drawing stream has an incomplete f32 vector")
    raw = b"".join(bytes.fromhex(word) for word in words)
    if hashlib.sha256(raw).hexdigest() != stream["sha256"]:
        raise ValueError("captured Drawing stream SHA-256 mismatch")

    target = case["target"]
    draw = target["draws"][0]
    if draw["kind"] != "instanced" or draw["args"] != [5, 0, 4, 26]:
        raise ValueError("unexpected AlphaShader instanced draw")
    if draw["framebuffer"] != 3 or draw["viewport"] != [0, 0, 128, 64]:
        raise ValueError("unexpected native coverage framebuffer or viewport")
    matrix = next(row[4] for row in target["native_shader_uniform_trace"]
                  if row[0] == "glUniformMatrix4fv" and row[2] == 1)
    expected_matrix = [float.fromhex(value) for value in (
        "0x1.0000000000000p-6", "0x0.0p+0", "0x0.0p+0", "0x0.0p+0",
        "0x0.0p+0", "0x1.0000000000000p-5", "0x0.0p+0", "0x0.0p+0",
        "0x0.0p+0", "0x0.0p+0", "-0x1.0002000000000p-15", "0x0.0p+0",
        "-0x1.0000000000000p+0", "-0x1.0000000000000p+0", "0x0.0p+0", "0x1.0000000000000p+0")]
    if [float.fromhex(value) for value in matrix] != expected_matrix:
        raise ValueError("native AlphaShader projection changed")
    for row in target["native_shader_uniform_trace"]:
        if row[0] == "glShaderSource" and "fwidth(dist)" in row[3][0]:
            source = row[3][0]
            if "FragColor = vec4(smoothAlpha);" not in source:
                raise ValueError("native AlphaShader coverage expression changed")
            if hashlib.sha256(source.encode()).hexdigest() != ALPHA_SHADER_SHA256:
                raise ValueError("native AlphaShader source hash mismatch")
            break
    else:
        raise ValueError("fixture does not contain the captured AlphaShader source")

    readback = target["readback"]
    rgba = zlib.decompress(base64.b64decode(readback["fbo3_rgba8_zlib_base64"]))
    if hashlib.sha256(rgba).hexdigest() != EXPECTED_FBO3_SHA256:
        raise ValueError("captured FBO3 readback SHA-256 mismatch")
    if readback["fbo3_rgba8_sha256"] != EXPECTED_FBO3_SHA256:
        raise ValueError("fixture FBO3 readback metadata changed")
    if len(rgba) != 128 * 64 * 4:
        raise ValueError("captured FBO3 readback has the wrong byte length")
    return case, [tuple(_f32_from_le_hex(words[i + j]) for j in range(3))
                  for i in range(0, len(words), 3)], rgba


def _dist(px, py, extent, x, y):
    # Screen position is `point + quad.xy * extent`. The joined Drawing
    # projection maps it to the 128x64 pixel grid; pixel centers are half-integers.
    u = (x + 0.5 - (px - extent)) / (2.0 * extent)
    v = (py + extent - (y + 0.5)) / (2.0 * extent)
    return math.hypot(u - 0.5, v - 0.5)


def scalar_mask(points, width=128, height=64):
    """Return RGBA8 coverage using the observed fine 2x2 derivative pairs."""
    coverage = bytearray(width * height)
    for px, py, extent in points:
        if not all(math.isfinite(value) for value in (px, py, extent)) or extent <= 0.0:
            continue
        x0 = max(0, math.ceil(px - extent - 0.5))
        x1 = min(width - 1, math.floor(px + extent - 0.5))
        y0 = max(0, math.ceil(py - extent - 0.5))
        y1 = min(height - 1, math.floor(py + extent - 0.5))
        for y in range(y0, y1 + 1):
            y_neighbor = y + (1 if y % 2 == 0 else -1)
            for x in range(x0, x1 + 1):
                x_neighbor = x + (1 if x % 2 == 0 else -1)
                distance = _dist(px, py, extent, x, y)
                fwidth = (abs(_dist(px, py, extent, x_neighbor, y) - distance)
                          + abs(_dist(px, py, extent, x, y_neighbor) - distance))
                if fwidth == 0.0:
                    alpha = 1.0 if distance < 0.5 else 0.0
                else:
                    alpha = 1.0 - min(1.0, max(0.0,
                        (distance - (0.5 - fwidth)) / fwidth))
                value = min(255, max(0, math.floor(alpha * 255.0 + 0.5)))
                offset = y * width + x
                if value > coverage[offset]:
                    coverage[offset] = value
    return bytes(channel for value in coverage for channel in (value,) * 4)


def main():
    _, points, expected = _load_case()
    actual = scalar_mask(points)
    digest = hashlib.sha256(actual).hexdigest()
    if actual != expected:
        differing = sum(left != right for left, right in zip(actual, expected))
        max_error = max((abs(left - right) for left, right in zip(actual, expected)), default=0)
        raise SystemExit(f"InkPen scalar mask mismatch: {differing} bytes differ, max error {max_error}")
    print(f"InkPen scalar mask matches captured native FBO3: 32768 bytes, sha256={digest}")


if __name__ == "__main__":
    main()
