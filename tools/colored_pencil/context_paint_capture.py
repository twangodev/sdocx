"""Paint native ColoredPencil source-context records over verified RGBA priors."""

import argparse
import ast
import base64
import hashlib
import json
from pathlib import Path
import struct
import sys
import zlib

ROOT = Path(__file__).resolve().parents[2]
BATCH = ROOT / "tools/colored_pencil/capture.py"
BATCH_SHA = "0159d2b4a94bbb8e735ef734aeb4b786a75910075d61d8611087f2757ba1689a"
HELPER = (
    ROOT
    / "tmp/vector-parity-plan/pen-family-audit/colored-pencil-v4-mesa-paint-v20-public-color-controls/capture_candidate.py"
)
HELPER_SHA = "0ffc0a80181faaba1c31a1b4c7e3592bcc66e63d90bc59431961d060a158dd68"
GEOMETRY = ROOT / "conformance/colored-pencil-v4-source-context-native.json"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    assert sha(BATCH.read_bytes()) == BATCH_SHA
    source = HELPER.read_text()
    assert sha(source.encode()) == HELPER_SHA
    geometry = json.loads(GEOMETRY.read_bytes())
    assert len(geometry["cases"]) == 31
    streams, sizes, aliases, canonical = {}, {}, {}, {}
    for case in geometry["cases"]:
        raw = bytes.fromhex(case["stream_hex"])
        assert sha(raw) == case["stream_sha256"]
        key = (case["pen_data"]["size_f32_bits"], case["stream_sha256"])
        if key in canonical:
            aliases[case["id"]] = canonical[key]
            continue
        name = case["id"]
        canonical[key] = name
        streams[name] = raw
        sizes[name] = struct.unpack("<f", bytes.fromhex(key[0]))[0]
    tree = ast.parse(BATCH.read_text())
    inject = next(
        ast.literal_eval(node.value)
        for node in tree.body
        if isinstance(node, ast.Assign)
        and any(isinstance(t, ast.Name) and t.id == "INJECT" for t in node.targets)
    )
    target = "    for stream_name,stream in BATCH_STREAMS.items():\n"
    assert inject.count(target) == 1
    setter = "SPen::PenDrawableRT::SetPenData(float, int)"
    inject = inject.replace(
        target,
        target
        + "        p.f(0,BATCH_SIZES[stream_name])\n"
        + f"        native_call(p,{setter!r},[rt,args.argb])\n",
    )
    target = "'sparse_zlib_base64':base64.b64encode(zlib.compress(sparse,9)).decode()"
    assert inject.count(target) == 1
    inject = inject.replace(
        target,
        "'rgba_zlib_base64':base64.b64encode(zlib.compress(image,9)).decode()",
    )
    target = "    native_call(p,PEN_DRAWABLE_SET_BUFFER,[rt,vector,0])\n"
    assert inject.count(target) == 1
    inject = inject.replace(
        target,
        "    p.f(0,8.)\n" + f"    native_call(p,{setter!r},[rt,args.argb])\n" + target,
    )
    marker = "    return {'schema':'sdocx-colored-pencil-v4-native-rtv1-mesa-draw.v6'"
    assert source.count(marker) == 1
    namespace = {
        "__name__": "colored_pencil_context_paint",
        "__file__": str(HELPER),
        "BATCH_STREAMS": streams,
        "BATCH_SIZES": sizes,
        "BATCH_BACKGROUNDS": {
            "clear": [0, 0, 0, 0],
            "white": [255] * 4,
            "purple": [64, 32, 96, 128],
        },
        "BATCH_SCALES": [1, 2],
        "BATCH_RESULTS": {},
        "base64": base64,
        "zlib": zlib,
    }
    exec(
        compile(source.replace(marker, inject + marker), str(HELPER), "exec"), namespace
    )
    sys.path.insert(0, str(namespace["CP_CANDIDATE"].parent))
    namespace["preflight"]()
    native = namespace["capture"](
        argparse.Namespace(
            argb=0x8044AAEE, width=256, height=256, failure_telemetry=None
        )
    )
    baseline = json.loads(
        (ROOT / "conformance/colored-pencil-paint-native.json").read_bytes()
    )
    expected = next(row for row in baseline["cases"] if row["argb"] == "0x8044aaee")
    assert (
        native["native_draw"]["rgba8_sha256"]
        == expected["native_rgba8_raw_gl_readback_sha256"]
    )
    receipt = {
        "schema": "sdocx-colored-pencil-v4-context-paint-native-v1",
        "coordinates": "Bottom-origin premultiplied RGBA8, full frames zlib/base64.",
        "capture": {
            "script": "tools/colored_pencil/context_paint_capture.py",
            "script_sha256": sha(Path(__file__).read_bytes()),
            "batch_script_sha256": BATCH_SHA,
            "base_source_sha256": HELPER_SHA,
            "geometry_receipt_sha256": sha(GEOMETRY.read_bytes()),
        },
        "scope": "Native source-context records, public callback/AddPoint/SetBuffer/SetPenData/SendDataToGPU followed by unmodified RTV1 Update/Draw. Selected size from native source capture; ARGB from native public ObjectStroke color route. Actual prior upload/readback verified for every frame. Optional depth map stays null. Synthetic glibc/Mesa record replay, not full native Drawing paint, serialized-note context or device parity.",
        "aliases": aliases,
        "host": native["host_mesa"],
        "pins": native["pins"],
        "baseline_sha256": native["native_draw"]["rgba8_sha256"],
        "shader_sha256": [
            part["sha256"]
            for row in native["native_paint_material"]["shader_source_payloads"]
            for part in row["parts"]
        ],
        "cases": namespace["BATCH_RESULTS"],
    }
    assert len(receipt["cases"]) == len(streams) * 12
    output = ROOT / "conformance/colored-pencil-v4-context-paint-native.json"
    temporary = output.with_suffix(".json.tmp")
    temporary.write_text(
        json.dumps(namespace["json_safe"](receipt), separators=(",", ":")) + "\n"
    )
    temporary.replace(output)
    print(output, len(receipt["cases"]), output.stat().st_size)


if __name__ == "__main__":
    main()
