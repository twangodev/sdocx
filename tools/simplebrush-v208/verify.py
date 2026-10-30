#!/usr/bin/env python3
"""Replay source exports against retained native frames and reader receipt."""

import ast
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import zlib

HERE = Path(__file__).resolve().parent
ROOT = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else HERE.parents[1]
receipt = json.loads((HERE / "receipt.json").read_text())


def sha(data):
    return hashlib.sha256(data).hexdigest()


assert sha((HERE / "capture.py").read_bytes()) == receipt["capture_script_sha256"]
for path, expected in receipt["comparators_sha256"].items():
    assert sha((HERE.parents[1] / path).read_bytes()) == expected, path


for path, expected in receipt["clean_source_replay"]["source_sha256"].items():
    assert sha((ROOT / path).read_bytes()) == expected, path


with tempfile.TemporaryDirectory(prefix="sdocx-simplebrush-v208-") as tmp:
    for case in receipt["cases"]:
        count = case["count"]
        directory = Path(tmp) / str(count)
        directory.mkdir()
        data = zlib.decompress((HERE / f"native-full-{count}.json.zlib").read_bytes())
        assert sha(data) == case["native_json_sha256"]
        verdict = json.loads(data)["native_gl_error_verdict"]
        assert verdict == case["gl_error_verdict"]
        assert verdict["status"] == "all_observed_draw_readback_task_errors_zero"
        assert verdict["paint_clean"] and verdict["pixel_readback_available"]
        (directory / "native.json").write_bytes(data)
        captures = directory / "captures"
        captures.mkdir()
        frame = zlib.decompress((HERE / f"frame-{count}.zlib").read_bytes())
        assert sha(frame) == case["frame_sha256"]
        (
            captures / f"simplebrush-native-draw-rgba-{case['frame_sha256']}.bin"
        ).write_bytes(frame)
        env = dict(
            os.environ,
            CARGO_TARGET_DIR="/tmp/sdocx-agent-target",
            CARGO_INCREMENTAL="0",
            CARGO_PROFILE_DEV_DEBUG="0",
            CARGO_PROFILE_TEST_DEBUG="0",
            SDOCX_POINTS_COUNT=str(count),
            PROBE_SIZE="3",
            SDOCX_TOOL="0",
            SDOCX_V62_NATIVE=str(directory / "native.json"),
            SDOCX_SIMPLEBRUSH_TWO_EXPORT=str(directory),
        )
        subprocess.run(
            [
                "cargo",
                "test",
                "-p",
                "sdocx",
                "--features",
                "pdf,serde",
                "two_pixel_native_export_fixture",
                "--lib",
            ],
            cwd=ROOT,
            env=env,
            check=True,
        )
        for name, expected in case["exports_sha256"].items():
            assert sha((directory / name).read_bytes()) == expected, name
        output = subprocess.check_output(
            [
                sys.executable,
                str(HERE.parent / "simplebrush-v62/compare.py"),
                str(directory),
            ],
            text=True,
        )
        rows = {
            line.split(" ", 1)[0]: ast.literal_eval(line.split(" ", 1)[1])
            for line in output.splitlines()
        }
        metrics = case["error_generic_then_profile"]
        assert [rows[x]["svg_alpha_error"] for x in ("generic", "two")] == metrics[
            "resvg_alpha"
        ]
        assert [rows[x]["matched"] for x in ("generic", "two")] == metrics[
            "mupdf_matched_paper_rgb"
        ]
        output = subprocess.check_output(
            [sys.executable, str(HERE / "pdfium.py"), str(directory)], text=True
        )
        pdfium = {line.split()[0]: int(line.split()[1]) for line in output.splitlines()}
        assert [pdfium[x] for x in ("generic", "two")] == metrics[
            "pdfium_unshifted_rgb"
        ]
        print(f"{count} points: export hashes and reader metrics match", flush=True)
