#!/usr/bin/env python3
"""Verify and compact six separately captured diagnostic shader frames."""

import base64
import hashlib
import json
import sys
import zlib
from pathlib import Path

root = Path(__file__).resolve().parents[2]
directory = Path(sys.argv[1])
vector = zlib.decompress(
    (root / "tools/crayon2-independent-control/native-vector.z").read_bytes()
)
receipt = {
    "scope": "Recompiled host shader; high float bytes inferred by expected coordinate.",
    "stamps": {},
}
for index in (20, 200, 350):
    frames, hashes = {}, {}
    for mode in ("xuv", "yuv"):
        paint = json.loads((directory / f"{mode}{index}.json").read_text())["paint"]
        assert paint["target_size"] == [192, 192]
        assert (
            bytes.fromhex(paint["native_vector_hex"])
            == vector[24 * index : 24 * (index + 1)]
        )
        assert paint["draw_calls"][-1]["values"] == [5, 0, 4, 1]
        frame = zlib.decompress(
            base64.b64decode(paint["readback"]["rgba8_zlib_base64"], validate=True)
        )
        assert len(frame) == 192 * 192 * 4
        frames[mode] = frame
        hashes[mode] = hashlib.sha256(frame).hexdigest()
    masks = {
        mode: {
            i // 4
            for i in range(0, len(frame), 4)
            if frame[i : i + 4] != bytes((64, 128, 192, 255))
        }
        for mode, frame in frames.items()
    }
    assert masks["xuv"] == masks["yuv"]
    rows = []
    for pos in sorted(masks["xuv"]):
        y, x = divmod(pos, 192)
        rows.append(
            {
                "xy": [x, 191 - y],
                "encoded_rgba": {
                    mode: list(frame[pos * 4 : pos * 4 + 4])
                    for mode, frame in frames.items()
                },
            }
        )
    receipt["stamps"][str(index)] = {"frame_sha256": hashes, "observations": rows}
assert receipt == json.loads(
    (root / "tools/crayon2-precision-model/triangle-plane-holdout.json").read_text()
)
print("Verified six diagnostic frames and 24 UV axis observations")
