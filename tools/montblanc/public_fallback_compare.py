#!/usr/bin/env python3
"""Reproduce synthetic public Stroke fallback versus retained native host frames."""
import base64
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import zlib

import pymupdf
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
CASES = ("baseline", "pressure", "size", "far_endpoint")
FIXTURE = ROOT / "tmp/legacy-marker-source-controls/serialized-marker.sdocx"
FIXTURE_SHA256 = "b0233b505f42a2a875e60ee3d2aee0d6e0f4e0db3ce8e5536a8f2b68e75d91e1"
RECEIPT = ROOT / "conformance/montblanc-public-fallback-host-comparison.json"


def composite_native_frame(frame, size, paper=252):
    """Composite a premultiplied native RGBA8 readback over opaque gray."""
    if len(frame) != size[0] * size[1] * 4:
        raise ValueError("native framebuffer size mismatch")
    rgb = bytes(min(255, frame[i + channel] + (paper * (255 - frame[i + 3]) + 127) // 255)
                for i in range(0, len(frame), 4) for channel in range(3))
    return Image.frombytes("RGB", size, rgb).transpose(Image.Transpose.FLIP_TOP_BOTTOM)


def metrics(native, actual):
    def support(data):
        return [any(abs(data[i + j] - 252) > 2 for j in range(3)) for i in range(0, len(data), 3)]
    ns, actual_support = support(native), support(actual)
    return [sum(ns), sum(actual_support), sum(a and b for a, b in zip(ns, actual_support)),
            sum(a or b for a, b in zip(ns, actual_support)),
            sum(sum(abs(native[i * 3 + j] - actual[i * 3 + j]) for j in range(3))
                for i, (a, b) in enumerate(zip(ns, actual_support)) if a or b)]


def main():
    if hashlib.sha256(FIXTURE.read_bytes()).hexdigest() != FIXTURE_SHA256:
        raise ValueError("synthetic source fixture hash mismatch")
    receipt = json.loads(RECEIPT.read_text())
    env = os.environ | {"CARGO_TARGET_DIR": "/tmp/sdocx-agent-target", "CARGO_INCREMENTAL": "0",
                        "CARGO_PROFILE_DEV_DEBUG": "0", "CARGO_PROFILE_TEST_DEBUG": "0"}
    with tempfile.TemporaryDirectory(prefix="montblanc-public-fallback-") as directory:
        output = Path(directory)
        subprocess.run(["cargo", "run", "--offline", "--quiet", "--manifest-path",
                        str(ROOT / "tools/montblanc/public-fallback/Cargo.toml"), "--", directory],
                       cwd=ROOT, env=env, check=True)
        for case in CASES:
            raw = gzip.decompress((ROOT / f"tools/montblanc/captures/{case}.json.gz").read_bytes())
            assert hashlib.sha256(raw).hexdigest() == receipt["cases"][case]["capture_sha256"]
            capture = json.loads(raw)
            frame = zlib.decompress(base64.b64decode(capture["post_readback"]["rgba8_zlib_base64"]))
            assert hashlib.sha256(frame).hexdigest() == capture["post_readback"]["sha256"]
            native = composite_native_frame(frame, (256, 256)).tobytes()
            subprocess.run(["resvg", str(output / f"{case}.svg"), str(output / "page.png")], check=True)
            svg = Image.open(output / "page.png").convert("RGB").tobytes()
            with pymupdf.open(output / f"{case}.pdf") as document:
                page = document[0]
                pixmap = page.get_pixmap(matrix=pymupdf.Matrix(256 / page.rect.width, 256 / page.rect.height), alpha=False)
                assert (pixmap.width, pixmap.height) == (256, 256)
                pdf = Image.frombytes("RGB", (256, 256), pixmap.samples).tobytes()
            for kind, data in (("svg", svg), ("pdf", pdf)):
                actual = metrics(native, data)
                print(case, kind, actual)
                if actual != receipt["cases"][case][kind]:
                    raise ValueError(f"{case} {kind} differs from receipt")


if __name__ == "__main__":
    main()
