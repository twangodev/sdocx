#!/usr/bin/env python3
"""Frozen NoCurve MagicPen edge-paint oracle (fresh native replay by default)."""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import zlib


def discover_root() -> Path:
    for parent in Path(__file__).resolve().parents:
        if parent.name == "conformance":
            return parent.parent
        if (parent / "conformance").is_dir() and (parent / "AGENTS.md").is_file():
            return parent
    raise RuntimeError("could not locate repository root")


ROOT = discover_root()
HERE = Path(__file__).resolve().parent
WRAPPER = HERE / "magicpen_nocurve_edge_paint_native.py"
CAPTURE = HERE / "magicpen_nocurve_edge_paint_capture.py"
ORDINARY_CAPTURE = HERE / "magicpen_nocurve_paint_capture.py"
ORDINARY_WRAPPER = HERE / "magicpen_nocurve_paint_native.py"
FIXTURE = HERE / "magicpen-nocurve-edge-paint-native.json"
INPUT_SHA256 = "8711c5d434ade733cb9044b68927a20707dcd251185e57a63fd5c03d5b433ed9"
EMBEDDED_INPUTS_SHA256 = "57158b37bb39eca418f0bf67b227b24a8f25170a5242158d90ace5cb5ad9e7b7"
ORDINARY_CAPTURE_SHA256 = "76c85701436f716cefae525c5a50ee68e062616509a57239aeea1e8e63135601"
ORDINARY_WRAPPER_SHA256 = "c30c5d612c9599562b2726f8e73b1d0a32df9414f3a68325a01932c88fc32be7"
CAPTURE_SHA256 = "6dc90d9733e139ccc22ced06bf6183fbd1c9869667bfd480cb60eee649fdc06a"


def sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def file_sha(path: Path) -> str:
    return sha(path.read_bytes())


def load_inputs() -> dict:
    fixture = json.loads(FIXTURE.read_text())
    if fixture.get("schema") != "magicpen-nocurve-edge-native-paint-v1" or fixture.get("inputs_sha256") != INPUT_SHA256:
        raise SystemExit("frozen NoCurve edge input schema/hash changed")
    inputs = fixture.get("inputs")
    if not isinstance(inputs, dict) or inputs.get("schema") != "magicpen-nocurve-edge-paint-inputs-v1":
        raise SystemExit("edge input fixture is missing its source case array")
    canonical = json.dumps(inputs, sort_keys=True, separators=(",", ":")).encode()
    if sha(canonical) != EMBEDDED_INPUTS_SHA256 or fixture.get("embedded_inputs_sha256") != EMBEDDED_INPUTS_SHA256:
        raise SystemExit("embedded edge inputs differ from the reviewed canonical digest")
    return inputs


def capture_ids(inputs: dict) -> list[tuple[str, str, int]]:
    result = [(case["id"], "transparent", scale) for case in inputs["cases"] for scale in inputs["view_scales"]]
    result.extend([
        ("two_subthreshold_terminal_nonzero_alpha255", "opaque", 1),
        ("subpixel_phase_low_pressure_alpha128", "opaque", 1),
        ("tiny_rejected_terminal_alpha0", "opaque", 1),
    ])
    return result


def check_pins() -> None:
    if file_sha(ORDINARY_CAPTURE) != ORDINARY_CAPTURE_SHA256:
        raise SystemExit("ordinary pinned native capture helper changed")
    if file_sha(ORDINARY_WRAPPER) != ORDINARY_WRAPPER_SHA256:
        raise SystemExit("ordinary pinned conformance wrapper changed")
    if file_sha(CAPTURE) != CAPTURE_SHA256:
        raise SystemExit("edge capture adapter changed; update the reviewed pin first")
    # Reuse the ordinary runner's checks for the pinned Python/Unicorn runtime,
    # ARM64 DSOs, EGL/GLES libraries, and the exact invoked native tools.
    import magicpen_nocurve_paint_native as ordinary
    ordinary.check_pins()


def validate(case_id: str, background: str, scale: int, value: dict, case: dict) -> None:
    if set(value) != {"source_capture", "paint"}:
        raise SystemExit(f"{case_id}/{background}/{scale}: unexpected capture shape")
    capture, paint = value["source_capture"], value["paint"]
    source = capture["source_input"]
    expected_fields = {
        "case_id": case_id,
        "points": case["points"],
        "pressure": case["pressure"],
        "size": case["size"],
        "color_argb": case["argb"],
        "curve_enabled": False,
        "tool_type": 2,
        "stroke_type": 0,
    }
    if any(source.get(key) != expected for key, expected in expected_fields.items()):
        raise SystemExit(f"{case_id}: native source Model getters do not match the frozen input")
    if capture["cases"][0]["source_model_get_point_count"] != 3:
        raise SystemExit(f"{case_id}: native ObjectStroke point count differs")
    if paint.get("source_inputs") != case or paint.get("source_points_unmodified") is not True:
        raise SystemExit(f"{case_id}: native paint input/mapping differs")
    if paint["draw_matrix_scale"] != scale or paint["draw_matrix_y_sign"] != -1 or paint["draw_matrix_translation"] != [0, 256 * scale]:
        raise SystemExit(f"{case_id}: native page-to-target transform differs")
    rgba = paint["readback"]
    expected_bg = [0, 0, 0, 0] if background == "transparent" else [64, 128, 192, 255]
    if paint["target_initial_background"] != {"kind": background, "rgba8": expected_bg}:
        raise SystemExit(f"{case_id}: target initial background differs")
    raw_rgba = zlib.decompress(base64.b64decode(rgba["rgba8_zlib_base64"], validate=True))
    extent = 256 * scale
    if (rgba["width"], rgba["height"], rgba["bytes"]) != (extent, extent, extent * extent * 4):
        raise SystemExit(f"{case_id}: framebuffer readback dimensions differ")
    if len(raw_rgba) != rgba["bytes"] or sha(raw_rgba) != rgba["sha256"]:
        raise SystemExit(f"{case_id}: framebuffer payload hash/length mismatch")
    vector = bytes.fromhex(paint["native_vector_hex"])
    if len(vector) != paint["native_vector_bytes"] or sha(vector) != paint["native_vector_sha256"]:
        raise SystemExit(f"{case_id}: native vector payload hash/length mismatch")
    if capture["cases"][0]["native_rtv1_vector"]["sha256"] != paint["native_vector_sha256"]:
        raise SystemExit(f"{case_id}: AddPoint vector and paint buffer differ")
    upload = paint["exact_native_gles_buffer_uploads"]
    if not upload or not any(item.get("dataSha256") == paint["native_vector_sha256"] for item in upload):
        raise SystemExit(f"{case_id}: queued native vector was not uploaded")
    bindings = paint["framebuffer_bindings_before_readpixels"]
    if bindings["draw_value"] != bindings["read_value"] or bindings["query_gl_error"] != 0:
        raise SystemExit(f"{case_id}: readback framebuffer binding query failed")
    for attachment in (bindings["draw_attachment_query"], bindings["read_attachment_query"]):
        if attachment["object_type"] != 5890 or attachment["object_name"] != 2 or attachment["status"] != 36053:
            raise SystemExit(f"{case_id}: actual readback attachment query/FBO status differs")
    if paint["gl_errors_final"] != 0 or not paint["framebuffer_completeness"]["all_complete"]:
        raise SystemExit(f"{case_id}: native draw produced GL errors or incomplete framebuffer")
    # Empty pixel support is valid for alpha=0. Geometry and full payload checks above remain mandatory.


def fresh_cases(inputs: dict) -> dict[str, dict]:
    env = {
        "PATH": os.pathsep.join((str(ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin"), "/usr/bin")),
        "PYTHONPATH": os.pathsep.join((str(ROOT / "scratch/apk-analysis-runtime/python"), str(HERE))),
        "LD_LIBRARY_PATH": str(ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu"),
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
    }
    by_id = {case["id"]: case for case in inputs["cases"]}
    captures = {}
    for case_id, background, scale in capture_ids(inputs):
        env["MAGICPEN_CASE_JSON"] = json.dumps(by_id[case_id], separators=(",", ":"))
        env["MAGICPEN_SCALE"] = str(scale)
        command = [sys.executable, str(CAPTURE), "--_case-id", case_id, "--_scale", str(scale), "--_capture-background", background]
        proc = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True, check=False)
        key = f"{case_id}-{background}-{scale}x"
        if proc.returncode:
            raise SystemExit(f"{key}: native capture failed ({proc.returncode}):\n{proc.stdout}\n{proc.stderr}")
        try:
            value = json.loads(proc.stdout)
        except json.JSONDecodeError as exc:
            raise SystemExit(f"{key}: capture adapter emitted invalid JSON: {exc}\n{proc.stdout}") from exc
        validate(case_id, background, scale, value, by_id[case_id])
        captures[key] = value
    return captures


def encode(inputs: dict, captures: dict[str, dict]) -> bytes:
    doc = {
        "schema": "magicpen-nocurve-edge-native-paint-v1",
        "wrapper_sha256": file_sha(WRAPPER),
        "capture_adapter_sha256": file_sha(CAPTURE),
        "ordinary_capture_sha256": ORDINARY_CAPTURE_SHA256,
        "ordinary_wrapper_sha256": ORDINARY_WRAPPER_SHA256,
        "inputs_sha256": INPUT_SHA256,
        "embedded_inputs_sha256": EMBEDDED_INPUTS_SHA256,
        "inputs": inputs,
        "case_order": list(captures),
        "captures": captures,
        "scope": "Synthetic ObjectStroke source arrays and host resource/queue/GLES adapters; native Drawing selection, MagicPen GLV1, queued RTV1 upload and Renderer Draw execute. Mesa RGBA targets are host-owned and are not Android-device output. All five frozen source inputs are replayed at transparent 1x/2x/4x; opaque 1x cases are explicit blend controls. Alpha-zero may produce nonempty geometry with unchanged target pixels.",
    }
    return (json.dumps(doc, sort_keys=True, indent=2) + "\n").encode()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--write-fixture", action="store_true", help="replace the frozen fixture after fresh capture")
    modes.add_argument("--check-fixture", action="store_true", help="freshly replay and compare with the fixture (default)")
    args = parser.parse_args()
    if not args.write_fixture:
        frozen = json.loads(FIXTURE.read_text())
        if frozen.get("wrapper_sha256") != file_sha(WRAPPER):
            raise SystemExit("wrapper hash differs from fixture; use explicit --write-fixture after review")
        if frozen.get("capture_adapter_sha256") != file_sha(CAPTURE):
            raise SystemExit("capture adapter hash differs from fixture")
    check_pins()
    inputs = load_inputs()
    captures = fresh_cases(inputs)
    raw = encode(inputs, captures)
    if args.write_fixture:
        fd, temporary = tempfile.mkstemp(prefix=FIXTURE.name + ".", dir=FIXTURE.parent)
        try:
            with os.fdopen(fd, "wb") as stream:
                stream.write(raw)
                stream.flush()
                os.fsync(stream.fileno())
            os.replace(temporary, FIXTURE)
        finally:
            if os.path.exists(temporary):
                os.unlink(temporary)
        print(f"wrote {FIXTURE.relative_to(ROOT)} sha256={sha(raw)} captures={len(captures)}")
        return 0
    if FIXTURE.read_bytes() != raw:
        raise SystemExit("fresh native edge replay differs from frozen fixture")
    print(f"verified {FIXTURE.relative_to(ROOT)} sha256={sha(raw)} captures={len(captures)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
