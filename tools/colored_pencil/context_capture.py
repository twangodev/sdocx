"""Capture native GLV4 source geometry across pen sizes and display contexts."""

import argparse
import contextlib
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
SOURCE = (
    ROOT
    / "tmp/vector-parity-plan/pen-family-audit/colored-pencil-v4-mesa-paint-v20-public-color-controls/color_input_capture_candidate.py"
)
SOURCE_SHA = "45624f3fbe1dbb6f29c1b122fbc925062b1371c3cbc7b20fd7f5b1894593c067"
HELPER = (
    ROOT
    / "scratch/colored-pencil-v4-input-capture-root-v4/colored_pencil_canvas_native.py"
)
HELPER_SHA = "4bc94bc3ce896b6cd4bc14d33dab7d358357e67d3323b6eb00e38c07b69bf128"
BENT = {
    "points": [[2, 3], [7, 1], [12, 6], [19, 10], [23, -2], [35, 8]],
    "pressure": [0.1, 0.3, 0.6, 1, 0.5, 0.2],
    "timestamps": [0, 8, 18, 30, 48, 70],
    "tilt": [0, 0.79, 0.65, 0.1, 1.1, 0.3],
    "orientation": [-0.25, 1.25, 3.5, -2.75, 0.125, 1.0],
}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def controls():
    rows = [
        {
            "id": f"size_{str(size).replace('.', 'p')}",
            "size": size,
            "screen": [1920, 1080],
        }
        for size in [-1, 0, 0.4, 1, 1.5, 2, 4, 6, 8, 12, 16, 32, 64, 256, 800, 1024]
    ]
    rows += [
        {"id": f"screen_{w}x{h}", "size": 8, "screen": [w, h]}
        for w, h in [
            (1080, 1920),
            (800, 600),
            (640, 480),
            (2560, 1440),
            (3840, 2160),
            (1, 16384),
            (16384, 16384),
        ]
    ]
    rows += [
        {
            "id": f"bent_{size}_{w}x{h}",
            "size": size,
            "screen": [w, h],
            "overrides": BENT,
        }
        for size, w, h in [(4, 800, 600), (12, 3840, 2160)]
    ]
    for name, size, xs in [
        ("gate_below", 800, [0, 23.999, 40]),
        ("gate_equal", 800, [0, 24, 40]),
        ("gate_above", 800, [0, 24.001, 40]),
        ("gate_latched", 800, [0, 10, 27, 28, 60, 70]),
        ("gate_dense", 8, [0, 0.1, 0.25, 0.5, 1, 2]),
        ("gate_tail", 8, [0, 1, 1.1, 1.2, 2]),
    ]:
        count = len(xs)
        rows.append(
            {
                "id": name,
                "size": size,
                "screen": [1920, 1080],
                "overrides": {
                    "points": [[x, 0] for x in xs],
                    "pressure": [0.5] * count,
                    "timestamps": list(range(count)),
                    "tilt": [0.2] * count,
                    "orientation": [0.75] * count,
                },
            }
        )
    return rows


def run_geometry(case):
    source = SOURCE.read_text()
    assert sha(source.encode()) == SOURCE_SHA
    assert sha(HELPER.read_bytes()) == HELPER_SHA
    target = "type=float,choices=(8.0,16.0),default=8.0"
    assert source.count(target) == 1
    source = source.replace(target, "type=float,default=8.0")
    target = "input_case['size']=args.saved_pen_size"
    assert source.count(target) == 1
    source = source.replace(
        target, target + "\ninput_case.update(CONTEXT_INPUT_OVERRIDES)"
    )
    # Zero getPosTan calls are valid evidence for a width larger than this path.
    target = "if not native_path_samples or not {'draw_object_stroke','saved_redraw'} <= {row['phase'] for row in native_path_samples}:"
    assert source.count(target) == 1
    source = source.replace(
        target,
        "if native_path_samples and not {'draw_object_stroke','saved_redraw'} <= {row['phase'] for row in native_path_samples}:",
    )
    namespace = {
        "__name__": "colored_pencil_context_source",
        "__file__": str(SOURCE),
        "CONTEXT_INPUT_OVERRIDES": case.get("overrides", {}),
    }
    original_argv = sys.argv[:]
    sys.path.insert(0, str(HELPER.parent))
    try:
        sys.argv = [
            str(SOURCE),
            "--advanced-setting",
            "4",
            "--saved-pen-size",
            str(case["size"]),
            "--width",
            str(case["screen"][0]),
            "--height",
            str(case["screen"][1]),
            "--argb",
            "0x8044aaee",
        ]
        with open(os.devnull, "w") as quiet, contextlib.redirect_stdout(quiet):
            exec(compile(source, str(SOURCE), "exec"), namespace)
    finally:
        sys.argv = original_argv
    capture = namespace["result"]
    assert (
        capture["native_calls"]["route"]["drawing_selected_drawable_class"]
        == "ColoredPencilStrokeDrawableGLV4"
    )
    assert capture["native_calls"]["pen_screen_resolution"] == [
        {"width": case["screen"][0], "height": case["screen"][1]}
    ]
    return namespace, capture


def compact(case, capture):
    calls = capture["native_calls"]
    entries = [
        e
        for e in calls["selected_drawable_redraw_lifecycle_method_entries"]
        if e["phase"] == "draw_object_stroke"
        and any("::drawLine(" in name for name in e["signatures"])
    ]
    values = []
    for entry in entries:
        raw = bytes.fromhex(entry["pen_data_target_head"])
        values.append(
            {
                "size_f32_bits": raw[:4].hex(),
                "width": struct.unpack_from("<i", raw, 32)[0],
                "height": struct.unpack_from("<i", raw, 36)[0],
                "spacing_extra": struct.unpack_from("<i", raw, 40)[0],
            }
        )
    assert values and all(value == values[0] for value in values)
    a = capture["phase_final_vectors"]["draw_object_stroke"]
    b = capture["phase_final_vectors"]["saved_redraw"]
    assert a["vector_bytes"] == b["vector_bytes"]
    raw = bytes.fromhex(a["vector_bytes"])
    return {
        "id": case["id"],
        "requested_size": case["size"],
        "screen": case["screen"],
        "input": capture["input"],
        "pen_data": values[0],
        "pen_get_size_f32_bits": calls["pen_GetSize_after_draw_f32_bits"],
        "record_count": len(raw) // 20,
        "stream_sha256": sha(raw),
        "stream_hex": raw.hex(),
        "drawing_and_selected_redraw_equal": True,
        "path_sample_count": sum(
            row["phase"] == "draw_object_stroke"
            for row in calls["sm_path_getPosTan_calls"]
        ),
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--child")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--extend-existing", action="store_true")
    args = parser.parse_args()
    if args.child:
        case = next(c for c in controls() if c["id"] == args.child)
        _, capture = run_geometry(case)
        args.output.write_text(
            json.dumps(
                {"case": compact(case, capture), "libraries": capture["libraries"]},
                separators=(",", ":"),
            )
        )
        return
    receipt = {
        "schema": "sdocx-colored-pencil-v4-source-context-native-v1",
        "capture": {
            "script": "tools/colored_pencil/context_capture.py",
            "script_sha256": sha(Path(__file__).read_bytes()),
            "source_controller_sha256": SOURCE_SHA,
            "canvas_helper_sha256": HELPER_SHA,
        },
        "scope": "Synthetic native ObjectStroke setters and native Drawing/selected RedrawPen, literal 4; and tool 2. Explicit system-display callbacks, identity canvas/color conversion and existing queue/ownership host boundary. Only parser input choices and input arrays are extended; zero path samples are admitted as an observation, while Drawing/Redraw stream equality and native class/context guards remain. No native drawable fields, sampler methods or shaders are patched. Geometry only, not serialized-note context, paint or device fidelity.",
        "cases": [],
    }
    retained = {}
    if args.extend_existing:
        previous_path = (
            ROOT / "conformance/colored-pencil-v4-source-context-native.json"
        )
        previous_raw = previous_path.read_bytes()
        previous = json.loads(previous_raw)
        assert previous["capture"]["source_controller_sha256"] == SOURCE_SHA
        assert previous["capture"]["canvas_helper_sha256"] == HELPER_SHA
        retained = {row["id"]: row for row in previous["cases"]}
        assert list(retained) == [row["id"] for row in controls()[: len(retained)]]
        receipt["capture"]["retained_base"] = {
            "receipt_sha256": sha(previous_raw),
            "capture": previous["capture"],
            "case_ids": list(retained),
        }
        receipt["libraries"] = previous["libraries"]
    with tempfile.TemporaryDirectory(
        prefix="sdocx-colored-pencil-context-"
    ) as directory:
        for case in controls():
            if case["id"] in retained:
                row = retained[case["id"]]
                assert row["requested_size"] == case["size"]
                assert row["screen"] == case["screen"]
                receipt["cases"].append(row)
                continue
            output = Path(directory) / "case.json"
            result = subprocess.run(
                [
                    sys.executable,
                    str(Path(__file__).resolve()),
                    "--child",
                    case["id"],
                    "--output",
                    str(output),
                ],
                cwd=ROOT,
                capture_output=True,
                text=True,
                timeout=120,
            )
            if result.returncode:
                print(case["id"], result.stderr[-6000:])
                raise SystemExit(result.returncode)
            native = json.loads(output.read_text())
            receipt["cases"].append(native["case"])
            receipt.setdefault("libraries", native["libraries"])
            print(
                case["id"],
                native["case"]["pen_data"],
                native["case"]["record_count"],
                flush=True,
            )
        output = ROOT / "conformance/colored-pencil-v4-source-context-native.json"
        temporary = output.with_suffix(".json.tmp")
        temporary.write_text(json.dumps(receipt, separators=(",", ":")) + "\n")
        temporary.replace(output)
        print(output, output.stat().st_size)


if __name__ == "__main__":
    main()
