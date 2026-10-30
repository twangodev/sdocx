#!/usr/bin/env python3
"""Reproduce bounded SimpleBrush host captures with project-local native inputs."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(os.environ.get("SDOCX_ROOT", Path(__file__).resolve().parents[2]))
SOURCE = ROOT / "tmp/vector-parity-plan/pen-family-audit/simplebrush-paint-mesa-v25"
EXPECTED = "86afe686345ccc650c01ad2f51d5a79728f2a0d7aec39462605c7d302205fdea"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("case", choices=["clipped8", "fraction12", "reverse20"])
    parser.add_argument("output", type=Path)
    parser.add_argument("--tool", type=int, choices=[0, 2, 3], default=2)
    args = parser.parse_args()
    if args.case != "fraction12" or (args.tool != 2 and not ((args.tool == 0 and int(os.environ["POINT_COUNT"]) in (32, 33, 64)) or (args.tool == 3 and int(os.environ["POINT_COUNT"]) in (33, 64, 225, 256)))):
        raise ValueError("point-count controls use fraction12; tool 0 supports 32, 33 and 64 points; tool 3 supports 33 and 64 points")
    original = SOURCE / "simplebrush-source-capture.py"
    receipt = json.loads(
        (
            ROOT / "conformance/simplebrush-scale-2x-host.json"
        ).read_text()
    )
    for path, expected in [
        (
            SOURCE / "simplebrush-paint-capture.py",
            receipt["capture"]["input_scripts"]["paint_sha256"],
        ),
        (
            ROOT / "scratch/apk-analysis-native/arm64-v8a/libSPenSimpleBrush.so",
            receipt["capture"]["library_sha256"],
        ),
    ]:
        if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
            raise RuntimeError(f"capture dependency hash mismatch: {path}")
    if hashlib.sha256(original.read_bytes()).hexdigest() != EXPECTED:
        raise RuntimeError(
            "Missing or changed source capture; restore the receipt-pinned scratch input"
        )
    with tempfile.TemporaryDirectory(prefix="simplebrush-scale-") as name:
        work = Path(name)
        shutil.copy(SOURCE / "simplebrush-paint-capture.py", work)
        (work / "helper-root").symlink_to(
            SOURCE / "helper-root", target_is_directory=True
        )
        text = original.read_text()
        matrix = (
            (1, 0, 0, 0, 1, 0, 0.25, 0.5, 1)
            if args.case in ("fraction12", "reverse20")
            else (2, 0, 0, 0, 2, 0, 0, 0, 1)
        )
        insertion = f""" p.pen_canvas=actual_pen_canvas\n matrix=p.alloc(36); p.u.mem_write(matrix,struct.pack('<9f',*{matrix!r}))\n p.call(oc.GRAPHICS_SET_MATRIX,[actual_pen_canvas,matrix],limit=1_000_000)\n p.call(oc.GRAPHICS_GET_MATRIX,[actual_pen_canvas],limit=1_000_000)\n matrix_ptr=p.x(0); matrix_read=bytes(p.u.mem_read(matrix_ptr,36))\n if matrix_read!=bytes(p.u.mem_read(matrix,36)): raise RuntimeError('native matrix getter mismatch')\n"""
        needle = " p.pen_canvas=actual_pen_canvas\n"
        if text.count(needle) != 1:
            raise RuntimeError("unexpected source capture structure")
        text = text.replace(needle, insertion)

        def replace_exact(source, old, new, count):
            if source.count(old) != count:
                raise RuntimeError(f"unexpected replacement count for {old!r}")
            return source.replace(old, new)

        if args.case in ("fraction12", "reverse20"):
            text = replace_exact(
                text,
                "'points':[[0.0,0.0],[10.0,0.0],[20.0,5.0]]",
                "'points':[[32.0,32.0],[42.0,32.0],[52.0,37.0]]",
                3,
            )
        if args.case == "reverse20":
            text = replace_exact(
                text,
                "'points':[[32.0,32.0],[42.0,32.0],[52.0,37.0]]",
                "'points':[[80.0,70.0],[129.0,78.0],[48.0,67.0]]",
                3,
            )
        # The inherited metadata assumes identity; correct it for this explicit context.
        text = replace_exact(text, "(1,0,0,0,1,0,0,0,1)", repr(matrix), 1)
        text = replace_exact(
            text,
            "'canvas_model_matrix_f32':[1,0,0,0,1,0,0,0,1]",
            f"'canvas_model_matrix_f32':{list(matrix)!r}",
            1,
        )
        count = int(os.environ["POINT_COUNT"])
        if count not in (2, 4, 7, 16, 32, 33, 64, 129, 225, 256, 320):
            raise ValueError("unsupported point count")
        points = [[25.0 + (i % 64), 35.0 + (i // 64) * 18.0 + [0, 2, 5, 7, 8, 7, 5, 2][i % 8]] for i in range(count)] if count > 128 else [[25.0 + i * (0.75 if count >= 33 else 2), 40.0 + [0, 2, 5, 7, 8, 7, 5, 2][i % 8]] for i in range(count)] if count >= 16 else [[32.0 + i * 6, 32.0 + (i % 3) * 3] for i in range(count)]
        pressures = [[0.25, 0.5, 0.75, 1.0][i % 4] for i in range(count)] if count >= 16 else [0.5] * count
        variant = os.environ.get("VARIANT")
        if variant is not None and (variant not in ("reverse", "alternate") or count not in (32, 64, 256, 320) or (count == 64 and variant != "reverse")):
            raise ValueError("variants require 32 points, or 64 points for reverse")
        if variant == "reverse":
            points = points[::-1]
            pressures = pressures[::-1]
        elif variant == "alternate":
            points = [[27.0 + 2*i, 48.0 + [0,-3,-5,-4,0,5,7,3][i%8]] for i in range(count)]
            pressures = [[1.0, 0.3, 0.6, 0.8, 0.4][i%5] for i in range(count)]
        for old in ("[0.5,0.25,0.75]", "[0.75,0.25,1.0]"):
            text = text.replace("'pressure':" + old, "'pressure':" + repr(pressures))
        for key, old, values in [("timestamps", "[0,8,19]", list(range(count))), ("tilt", "[0.0,0.2,0.4]", [0.0] * count), ("orientation", "[0.0,0.25,0.5]", [0.0] * count)]:
            text = text.replace("'" + key + "':" + old, "'" + key + "':" + repr(values))
        text = text.replace("[[32.0,32.0],[42.0,32.0],[52.0,37.0]]", repr(points))
        (work / original.name).write_text(text)
        env = dict(os.environ)
        env["PYTHONPATH"] = str(ROOT / "scratch/apk-analysis-runtime/python")
        command = [
            "python3",
            str(work / "simplebrush-paint-capture.py"),
            "--width",
            "256",
            "--height",
            "256",
        ]
        if args.case in ("fraction12", "reverse20"):
            command += ["--pen-size", os.environ.get("PROBE_SIZE", "12") if args.case == "fraction12" else "20"]
        command += ["--tool-type", str(args.tool)]
        for seq in range(8):
            command += ["--dispatch-task-seq", str(seq)]
        with args.output.open("w") as output:
            subprocess.run(command, cwd=ROOT, env=env, stdout=output, check=True)
        captured = json.loads(args.output.read_text())
        if not captured.get("native_gl_error_verdict", {}).get("paint_clean"):
            raise RuntimeError("native paint failed")
        # Capture scripts write pixels inside work; keep them beside requested receipt.
        pixels = work / "captures"
        if pixels.exists():
            shutil.copytree(pixels, args.output.parent / "captures", dirs_exist_ok=True)


if __name__ == "__main__":
    main()
