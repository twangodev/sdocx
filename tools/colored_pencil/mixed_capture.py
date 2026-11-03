"""Capture native ColoredPencil Drawing over retained mixed Pencil2 paint."""

import argparse
import base64
import hashlib
import json
from pathlib import Path
import sys
import zlib

import drawing_capture as drawing

ROOT = Path(__file__).resolve().parents[2]
DRAWING_SHA = "9dcfdd5180c1b020033279a5db305a5dc6a11d949d33e0ba1cc40470ffd14f97"
PENCIL = ROOT / "conformance/pencil2-v14-page-context-native.json"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def priors():
    pencil = json.loads(PENCIL.read_bytes())
    results = {}
    for scale in [1, 2]:
        for order in ["mixed", "mixed_reversed"]:
            name = f"{order}_top_scale{scale}_white"
            case = pencil["results"][name]
            image = bytearray(bytes(case["background"]) * (192 * 192))
            for x, y, *rgba in case["pixels"]:
                image[(y * 192 + x) * 4 : (y * 192 + x + 1) * 4] = bytes(rgba)
            assert sha(image) == case["sha256"]
            frame = bytearray(b"\xff" * (256 * 256 * 4))
            for y in range(192):
                frame[((y + 64) * 256) * 4 : ((y + 64) * 256 + 192) * 4] = image[
                    y * 192 * 4 : (y + 1) * 192 * 4
                ]
            results[f"top_{scale}_pencil_{order}"] = (name, bytes(frame))
    return results


def main():
    assert (
        sha((ROOT / "tools/colored_pencil/drawing_capture.py").read_bytes())
        == DRAWING_SHA
    )
    source = drawing.HELPER.read_text()
    assert sha(source.encode()) == drawing.HELPER_SHA
    geometry = json.loads(drawing.GEOMETRY.read_bytes())
    depth = json.loads(drawing.DEPTH.read_bytes())["depth_inputs"]
    maps = {
        "A": (
            depth["grid"]["width"],
            depth["grid"]["height"],
            bytes.fromhex(depth["grid"]["rgba_hex"]),
        ),
        "B": (1, 1, bytes.fromhex(depth["black"]["rgba_hex"])),
    }
    prior_inputs = priors()
    injection = drawing.INJECT
    replacements = [
        ("for origin in ['top','bottom']:", "for origin in ['top']:"),
        (
            "[('clear',[0,0,0,0]),('white',[255]*4),('purple',[64,32,96,128])]",
            "[('pencil_mixed',[255]*4),('pencil_mixed_reversed',[255]*4)]",
        ),
        ("prior=bytes(background)*(w*h)", "prior=MIXED_PRIORS[sequence][1]"),
        ("enumerate(actions):", "enumerate(actions[:3]):"),
        (
            "'sequence':sequence,'step':step",
            "'sequence':sequence,'prior_id':sequence,'step':step",
        ),
    ]
    for old, new in replacements:
        assert injection.count(old) == 1, old
        injection = injection.replace(old, new)
    marker = "    return {'schema':'sdocx-colored-pencil-v4-native-rtv1-mesa-draw.v6'"
    assert source.count(marker) == 1
    scope = {
        "__name__": "colored_pencil_mixed_drawing",
        "__file__": str(drawing.HELPER),
        "DRAWING_DEPTH_INPUTS": maps,
        "DRAWING_DEPTH_PROOF": {},
        "DRAWING_SOURCE_STREAMS": {
            row["id"]: bytes.fromhex(row["stream_hex"])
            for row in geometry["cases"]
            if row["id"] in ("size_8", "size_16")
        },
        "DRAWING_RESULTS": {},
        "MIXED_PRIORS": prior_inputs,
        "base64": base64,
        "zlib": zlib,
    }
    exec(
        compile(
            source.replace(marker, injection + marker), str(drawing.HELPER), "exec"
        ),
        scope,
    )
    sys.path.insert(0, str(scope["CP_CANDIDATE"].parent))
    scope["preflight"]()
    native = scope["capture"](
        argparse.Namespace(
            argb=0x8044AAEE, width=256, height=256, failure_telemetry=None
        )
    )
    baseline = json.loads(
        (ROOT / "conformance/colored-pencil-paint-native.json").read_bytes()
    )
    reference = next(row for row in baseline["cases"] if row["argb"] == "0x8044aaee")
    assert (
        native["native_draw"]["rgba8_sha256"]
        == reference["native_rgba8_raw_gl_readback_sha256"]
    )
    receipt = {
        "schema": "sdocx-colored-pencil-v4-pencil2-drawing-native-v1",
        "capture": {
            "script": "tools/colored_pencil/mixed_capture.py",
            "script_sha256": sha(Path(__file__).read_bytes()),
            "drawing_script_sha256": DRAWING_SHA,
            "base_source_sha256": drawing.HELPER_SHA,
            "pencil_receipt_sha256": sha(PENCIL.read_bytes()),
            "geometry_receipt_sha256": sha(drawing.GEOMETRY.read_bytes()),
            "depth_receipt_sha256": sha(drawing.DEPTH.read_bytes()),
        },
        "scope": "Native ColoredPencil Model/Drawing/GL/callback/RT/GLES continuation over verified nonuniform native Pencil2 pixel inputs. Four three-stroke sequences vary Pencil2 record order, projection 1x/2x, native ColoredPencil size/color and retained depth. The retained 192x192 Pencil2 frame is placed at GL (0,64) in an opaque 256x256 prior without resampling. The native Pencil2 producer and ColoredPencil continuation are separate captures joined by RGBA bytes, not one mixed-tool Drawing invocation, SDK note deserialization or device validation. Existing explicit host boundaries remain. No native fields, geometry, shader, uniform or scheduler rewrites.",
        "coordinates": "Bottom-origin premultiplied RGBA8 frames, full zlib/base64.",
        "host": native["host_mesa"],
        "pins": native["pins"],
        "baseline_sha256": native["native_draw"]["rgba8_sha256"],
        "prior_inputs": {
            name: {
                "pencil_case": case,
                "placement": [0, 64],
                "background": [255] * 4,
                "frame": [256, 256],
                "frame_sha256": sha(rgba),
                "rgba_zlib_base64": base64.b64encode(zlib.compress(rgba, 9)).decode(),
            }
            for name, (case, rgba) in prior_inputs.items()
        },
        "depth_inputs": {
            name: {
                "width": w,
                "height": h,
                "rgba_hex": rgba.hex(),
                "native_upload": scope["DRAWING_DEPTH_PROOF"][name],
            }
            for name, (w, h, rgba) in maps.items()
        },
        "cases": scope["DRAWING_RESULTS"],
    }
    assert len(receipt["cases"]) == 12
    output = ROOT / "conformance/colored-pencil-v4-pencil2-drawing-native.json"
    temporary = output.with_suffix(".json.tmp")
    temporary.write_text(
        json.dumps(scope["json_safe"](receipt), separators=(",", ":")) + "\n"
    )
    temporary.replace(output)
    print(output, output.stat().st_size)


if __name__ == "__main__":
    main()
