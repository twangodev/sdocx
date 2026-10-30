#!/usr/bin/env python3
"""Verify a native isolated-stamp capture against the compact quad receipt."""

import hashlib
import json
import struct
import sys
import zlib
from pathlib import Path

root = Path(__file__).resolve().parents[2]
paint = json.loads(Path(sys.argv[1]).read_text())["paint"]
vector = zlib.decompress(
    (root / "tools/crayon2-independent-control/native-vector.z").read_bytes()
)
assert bytes.fromhex(paint["native_vector_hex"]) == vector[100 * 24 : 101 * 24]
assert paint["target_size"] == [192, 192]
draw = paint["draw_calls"][-1]["values"]
assert draw == [5, 0, 4, 1]
quad_uploads = [
    call
    for call in paint["paint_gl_calls"]
    if call["call"] == "glBufferData" and call["size"] == 64 and call.get("dataHex")
]
assert len(quad_uploads) == 1
quad = bytes.fromhex(quad_uploads[0]["dataHex"])
assert hashlib.sha256(quad).hexdigest() == quad_uploads[0]["dataSha256"]
receipt = {
    "scope": "Isolated native Crayon2 stamp 100 host GLES upload and draw",
    "stamp": 100,
    "target_size": paint["target_size"],
    "native_vector_sha256": hashlib.sha256(vector[100 * 24 : 101 * 24]).hexdigest(),
    "draw_values": draw,
    "quad_sha256": hashlib.sha256(quad).hexdigest(),
    "quad_hex": quad.hex(),
    "quad_f32": list(struct.unpack("<16f", quad)),
}
assert receipt == json.loads(
    (root / "tools/crayon2-precision-model/quad-receipt.json").read_text()
)
print("Verified stamp vector, 64-byte quad upload, and triangle-strip draw")
