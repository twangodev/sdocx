"""Compact native evidence for fixed-width saved Oblique bounds.

This reuses the bounded GLV4 RedrawPen -> RTV1 AddPoint Unicorn runner. The
fixture stores only the returned raw RectF and a digest of native callback
triples; it is not a raster or renderer oracle.
"""
import argparse
import hashlib
import json
import struct
from pathlib import Path

from oblique_redraw_native import BASE_LIBRARY_SHA256, RedrawOracle, core

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "conformance/oblique-fixed-width-bounds.json"
POINTS = [[10.0, 20.0], [18.0, 25.0], [26.0, 20.0], [34.0, 18.0]]


def bits(value):
    return struct.pack("<f", value).hex()


def run_case(path, enabled, width):
    case = {
        "points": POINTS,
        "timestamps": [0, 1, 2, 3],
        "pressures": [0.5] * 4,
        "tilts": [0.0] * 4,
        "orientations": [0.0] * 4,
        "size": 4.0,
        "curve_enabled": True,
        "tool_type": 2,
        "fixed_enabled": enabled,
        "fixed_value": width,
        "inverse_scale": [0.25, 2.0],
    }
    result = RedrawOracle(path, case).run()
    raw = b"".join(bytes.fromhex(value) for triple in result["callback_triple_f32_bits"] for value in triple)
    return {
        "fixed_enabled": enabled,
        "fixed_width": width,
        "inverse_scale": case["inverse_scale"],
        "size_u32": result["size_u32"],
        "add_point_calls": result["call_count"],
        "mesh_vector3_count": len(result["callback_triples"]),
        "mesh_triples_sha256": hashlib.sha256(raw).hexdigest(),
        "raw_rect_f32_bits": [bits(value) for value in result["pen_rect_raw"]],
    }


def generate(path):
    widths = [(False, 0.0), (True, 0.4), (True, 1.0), (True, 2.0), (True, 8.0), (True, 800.0)]
    return {
        "schema": "sdocx-oblique-fixed-width-bounds-v1",
        "libraries": {
            "libSPenObliquePen.so": core.LIBRARY_SHA256,
            "libSPenBase.so": BASE_LIBRARY_SHA256,
        },
        "entrypoint": "GLV4_RedrawPen 0x40928; native RTV1 AddPoint 0x45288",
        "host_boundary": {
            "SetPenRect": "stubbed no-op in the existing manager-interface boundary; the reported raw RectF is written by the native redraw body before return",
            "other": "MotionEvent, cache/manager accessors, runtime matrix supplier, and host allocation remain explicit stubs in RedrawOracle",
            "not_exercised": "Drawing canvas clipping, renderer transforms, and rasterization",
        },
        "mesh_digest": "SHA-256 over concatenated little-endian f32 bytes for each native AddPoint Vector3 in callback order",
        "cases": [run_case(path, enabled, width) for enabled, width in widths],
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--library", type=Path, default=core.LIBRARY)
    parser.add_argument("--write-fixture", action="store_true")
    parser.add_argument("--check-fixture", action="store_true")
    args = parser.parse_args()
    result = generate(args.library)
    if args.write_fixture:
        FIXTURE.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    elif args.check_fixture:
        if json.loads(FIXTURE.read_text()) != result:
            raise SystemExit("native fixed-width bounds differ from conformance/oblique-fixed-width-bounds.json")
        print(f"verified {len(result['cases'])} native fixed-width bounds cases")
    else:
        print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
