"""Native Oblique saved-style defaults when ObjectStroke setters are omitted.

Run with the project research runtime:
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/oblique_defaults_native.py --write-fixture
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/oblique_defaults_native.py --check-fixture

The six bounded cases use a synthetic ObjectStroke and actual native Drawing,
Pen setters, selected saved redraw, and Oblique V1/V4 geometry. They do not
exercise SDocx parsing or Samsung device/export policy.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CALLER = ROOT / "conformance/oblique_saved_caller_native.py"
FIXTURE = ROOT / "conformance/oblique_defaults_native.json"
MODEL_DEFAULTS = ROOT / "conformance/object-stroke-defaults-native.json"
SCHEMA = "sdocx-oblique-missing-style-defaults-native-v1"

parser = argparse.ArgumentParser(description=__doc__)
mode = parser.add_mutually_exclusive_group(required=True)
mode.add_argument("--write-fixture", action="store_true")
mode.add_argument("--check-fixture", action="store_true")
args = parser.parse_args()


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def capture(route, omit_size, omit_color):
    command = [sys.executable, str(CALLER), "--case", "color_contract", "--print-capture"]
    if route == "v4":
        command += ["--advanced-setting", "4"]
    if omit_size:
        command.append("--omit-size")
    if omit_color:
        command.append("--omit-color")
    env = dict(os.environ)
    env["PYTHONPATH"] = str(ROOT / "scratch/apk-analysis-runtime/python") + os.pathsep + env.get("PYTHONPATH", "")
    result = subprocess.run(command, cwd=ROOT, env=env, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True)
    data = json.loads(result.stdout)
    expected_version = "1" if route == "v1" else "4"
    input_data = data["input"]
    native = data["native_calls"]
    probe = native["missing_style_defaults_probe"]
    if data["case_id"] != "color_contract":
        raise RuntimeError("caller returned the wrong case")
    if input_data["style_setters_omitted"] != {"pen_size": omit_size, "color_argb": omit_color}:
        raise RuntimeError("omitted setter boundary was not recorded")
    if native["route"]["drawing_selected_drawable_version"] != expected_version:
        raise RuntimeError(f"expected selected V{expected_version} Drawing route")
    size_bits = "00000000" if omit_size else "0000803f"
    argb = "0xff000000" if omit_color else "0x80402010"
    if input_data["pen_size_f32_bits"] != size_bits:
        raise RuntimeError("ObjectStroke GetPenSize does not match setter admission")
    if probe["ObjectStroke_GetPenSize_f32_bits"] != size_bits:
        raise RuntimeError("native size getter witness disagrees with input")
    if probe["ObjectStroke_GetColor_argb_u32"] != argb:
        raise RuntimeError("native color getter witness disagrees with input")
    expected_pen_size = "cdcccc3e" if omit_size else "0000803f"
    if probe["Pen_GetSize_after_Drawing_f32_bits"] != expected_pen_size:
        raise RuntimeError("Pen::SetSize clamp/forwarding result changed")
    if probe["selected_drawable_getSize_after_saved_redraw_f32_bits"] != expected_pen_size:
        raise RuntimeError("selected saved drawable size differs from Drawing Pen size")
    # The native getter/setter arguments prove explicit ARGB forwarding; only
    # the omitted-color result has a fixed default-color contract to assert.
    if omit_color and probe["drawing_selected_rt_RGBA_f32_bits"] != ["00000000", "00000000", "00000000", "0000803f"]:
        raise RuntimeError("omitted color did not produce native opaque black RT color")
    setters = probe["native_Pen_SetSize_SetColor_entries_during_Drawing"]
    by_method = {row["method"]: row for row in setters}
    if set(by_method) != {"SPen::Pen::SetSize(float)", "SPen::Pen::SetColor(int)"}:
        raise RuntimeError("Drawing did not execute both native Pen style setters")
    if by_method["SPen::Pen::SetSize(float)"]["arg_s0_bits"] != size_bits:
        raise RuntimeError("Drawing forwarded an unexpected size to Pen::SetSize")
    if by_method["SPen::Pen::SetColor(int)"]["arg_w1"] != int(argb, 16):
        raise RuntimeError("Drawing forwarded an unexpected ARGB value to Pen::SetColor")
    for phase in ("draw_object_stroke", "saved_redraw"):
        vector = data["phase_final_vectors"][phase]
        if len(bytes.fromhex(vector["vector_bytes"])) != vector["vector_count_bytes"]:
            raise RuntimeError("native vector byte count mismatch")
    if data["phase_final_vectors"]["draw_object_stroke"] != data["phase_final_vectors"]["saved_redraw"]:
        raise RuntimeError("native Drawing and selected saved redraw streams differ")
    return data


def build():
    cases = {}
    for route in ("v1", "v4"):
        for omission, size, color in (("both_omitted", True, True),
                                      ("size_omitted", True, False),
                                      ("color_omitted", False, True)):
            cases[f"{route}_{omission}"] = capture(route, size, color)
    return {
        "schema": SCHEMA,
        "scope": "synthetic native ObjectStroke caller with actual native Drawing and selected V1/V4 saved redraw; not serialized SDocx input or Samsung app/device export policy",
        "source_hashes": {
            "oblique_saved_caller_native.py": sha(CALLER),
            "object-stroke-defaults-native.json": sha(MODEL_DEFAULTS),
        },
        "cases": cases,
    }


actual = build()
encoded = json.dumps(actual, indent=2, sort_keys=True) + "\n"
if args.write_fixture:
    FIXTURE.write_text(encoded)
    print(f"wrote {FIXTURE.relative_to(ROOT)} ({sha(FIXTURE)})")
else:
    if not FIXTURE.exists():
        raise SystemExit("native Oblique defaults fixture is missing")
    expected = json.loads(FIXTURE.read_text())
    if expected != actual:
        raise SystemExit("native Oblique defaults fixture mismatch")
    print(f"verified {FIXTURE.relative_to(ROOT)} ({sha(FIXTURE)})")
