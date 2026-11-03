"""Replay retained captures on a pinned clean revision; outputs go to a caller-selected directory."""

import gzip
import hashlib
import io
import json
import os
import pathlib
import subprocess
import sys
import tarfile
import zlib


def run_cases(root, src, out):
    cases = [
        ("320", 320, 8, 2, "reverse"),
        ("64-alternate", 64, 3, 0, "alternate"),
        ("65-baseline", 65, 3, 0, ""),
        ("128-baseline", 128, 3, 0, ""),
    ]
    for name, count, size, tool, variant in cases:
        d = out / (
            "sb-gradient-" + ("base-" if os.environ["MODE"] == "base" else "") + name
        )
        d.mkdir(exist_ok=True)
        (d / "captures").mkdir(exist_ok=True)
        if name == "320":
            p = root / "tools/simplebrush-one-long"
            vec = zlib.decompress((p / "320.vector.zlib").read_bytes())
            frame = zlib.decompress((p / "320.rgba.zlib").read_bytes())
            capture_receipt = json.loads((p / "320-receipt.json").read_text())
            assert hashlib.sha256(vec).hexdigest() == capture_receipt["geometry_sha256"]
            h = capture_receipt["frame_sha256"]
            assert hashlib.sha256(frame).hexdigest() == h
            native = {
                "drawing_vector": {"vector_bytes": vec.hex()},
                "native_draw_target_readback": {"sha256": h},
            }
        elif count == 64:
            p = root / "tools/simplebrush-v209"
            native = json.loads(
                zlib.decompress((p / "native-full.json.zlib").read_bytes())
            )
            frame = zlib.decompress((p / "frame.rgba.zlib").read_bytes())
            h = hashlib.sha256(frame).hexdigest()
        elif count == 65:
            p = root / "tools/simplebrush-v210"
            native = json.loads(
                gzip.decompress((p / "baseline-native.json.gz").read_bytes())
            )
            frame = gzip.decompress((p / "d-frame.rgba.gz").read_bytes())
            h = hashlib.sha256(frame).hexdigest()
        else:
            p = root / "tools/simplebrush-size3-128"
            native = json.loads((p / "baseline-native.json").read_text())
            frame = gzip.decompress((p / "baseline-frame.rgba.gz").read_bytes())
            h = hashlib.sha256(frame).hexdigest()
        assert native["native_draw_target_readback"]["sha256"] == h
        (d / "native.json").write_text(json.dumps(native))
        (d / "captures" / f"simplebrush-native-draw-rgba-{h}.bin").write_bytes(frame)
        env = {
            k: v
            for k, v in os.environ.items()
            if not k.startswith("SDOCX_") and k not in ("PROBE_SIZE", "VARIANT")
        }
        env.update(
            CARGO_TARGET_DIR="/tmp/sdocx-agent-target",
            CARGO_INCREMENTAL="0",
            CARGO_PROFILE_DEV_DEBUG="0",
            CARGO_PROFILE_TEST_DEBUG="0",
            CARGO_BUILD_JOBS="2",
            SDOCX_POINTS_COUNT=str(count),
            PROBE_SIZE=str(size),
            SDOCX_TOOL=str(tool),
            SDOCX_V62_NATIVE=str(d / "native.json"),
            SDOCX_SIMPLEBRUSH_TWO_EXPORT=str(d),
        )
        if variant:
            env["VARIANT"] = variant
        subprocess.run(
            [
                "cargo",
                "test",
                "--locked",
                "--offline",
                "-p",
                "sdocx",
                "--features",
                "pdf,serde",
                "--lib",
                "two_pixel_native_export_fixture",
            ],
            cwd=src,
            env=env,
            check=True,
        )
        for filename, expected in receipt["cases"][name][os.environ["MODE"]].items():
            data = (d / filename).read_bytes()
            assert (
                len(data) == expected["bytes"]
                and hashlib.sha256(data).hexdigest() == expected["sha256"]
            ), (name, filename)


root = pathlib.Path(__file__).resolve().parents[2]
out = pathlib.Path(sys.argv[1]).resolve()
out.mkdir(parents=True, exist_ok=False)
src = out / "source"
src.mkdir()
revision = "43c3e3c4f14e371741ac37df564d9167e8f21591"
archive = subprocess.check_output(["git", "archive", revision], cwd=root)

with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
    tar.extractall(src, filter="data")
receipt = json.loads(
    (root / "tools/simplebrush-gradient-reuse/receipt.json").read_text()
)
for path, digest in receipt["source_sha256"].items():
    assert hashlib.sha256((src / path).read_bytes()).hexdigest() == digest
patch_path = root / "tools/simplebrush-gradient-reuse/candidate.patch"
assert (
    hashlib.sha256(patch_path.read_bytes()).hexdigest()
    == receipt["candidate_patch_sha256"]
)
for mode in ("base", "candidate"):
    if mode == "candidate":
        subprocess.run(
            [
                "patch",
                "-p1",
                "-i",
                str(patch_path),
            ],
            cwd=src,
            check=True,
        )
    (src / "crates/sdocx/src/render/simplebrush.rs").touch()
    os.environ["MODE"] = mode
    run_cases(root, src, out)

subprocess.run(
    [
        sys.executable,
        str(root / "tools/simplebrush-gradient-reuse/compare-clean.py"),
        str(out),
    ],
    check=True,
)
