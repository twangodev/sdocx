#!/usr/bin/env python3
"""Compare native Pencil3 V1/V3 sampler inputs before their RNG-specific kernels.

This runner hash-pins and patches the existing saved-caller source in memory.
Only route selection, native saved-redraw lifecycle, and the explicit synthetic
source/context profile are exercised. By default all five input profiles are
rerun under Unicorn and compared with the compact fixture.
"""
import argparse
import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import re
import struct
import sys

ROOT = Path(__file__).resolve().parents[1]
BASE_SCRIPT = ROOT / "conformance/pencil3_saved_caller_native.py"
FIXTURE = ROOT / "conformance/pencil3-sampler-inputs-native.json"
BASE_SCRIPT_SHA256 = "129001ce587c104e22e1675e759df9a04d942e884a93fb747b195d001b6a776e"
PENCIL3_SHA256 = "41a995cac7a597af53359401df26187d4299221f3460c91b56540596fc73d359"

CASES = {
    "baseline_tool2_curve": {
        "points": [[0.0, 0.0], [10.0, 0.0], [20.0, 0.0], [30.0, 10.0]],
        "size": 10.0, "curve_enabled": True, "tool_type": 2, "stroke_type": 0,
        "pressure": [0.25, 0.5, 0.75, 1.0], "timestamps": [10, 20, 30, 40],
        "tilt": [0.0, 0.0, 0.0, 0.0], "orientation": [0.0, 0.0, 0.0, 0.0],
        "width": 1920, "height": 1080,
    },
    "pressure_tilt_time_tool2_curve_5pt": {
        "points": [[1.0, 2.0], [8.0, 6.0], [17.0, 11.0], [28.0, 4.0], [43.0, 19.0]],
        "size": 13.25, "curve_enabled": True, "tool_type": 2, "stroke_type": 0,
        "pressure": [0.05, 0.93, 0.37, 0.72, 1.2], "timestamps": [100, -40, 120, 1000, 1020],
        "tilt": [0.1, 0.3, 0.6, 0.8, 0.05], "orientation": [0.0, 0.5, -0.2, 0.9, 1.1],
        "width": 1920, "height": 1080,
    },
    "tool1_straight_4pt": {
        "points": [[-5.0, 3.0], [3.0, 12.0], [19.0, 8.0], [32.0, 21.0]],
        "size": 7.5, "curve_enabled": False, "tool_type": 1, "stroke_type": 0,
        "pressure": [0.15, 0.99, 0.25, 0.85], "timestamps": [500, 501, 550, 900],
        "tilt": [0.56, 0.7, 0.2, 0.0], "orientation": [0.0, 0.0, 0.0, 0.0],
        "width": 1920, "height": 1080,
    },
    "tool3_curve_4pt": {
        "points": [[2.0, -1.0], [9.0, 13.0], [22.0, 5.0], [35.0, 18.0]],
        "size": 18.0, "curve_enabled": True, "tool_type": 3, "stroke_type": 0,
        "pressure": [0.2, 0.8, 0.4, 1.0], "timestamps": [0, 200, 201, 9000],
        "tilt": [0.79, 0.55, 0.35, 0.68], "orientation": [0.0, 0.1, 0.3, 0.7],
        "width": 1920, "height": 1080,
    },
    "baseline_tool2_curve_context_1280x720": {
        "points": [[0.0, 0.0], [10.0, 0.0], [20.0, 0.0], [30.0, 10.0]],
        "size": 10.0, "curve_enabled": True, "tool_type": 2, "stroke_type": 0,
        "pressure": [0.25, 0.5, 0.75, 1.0], "timestamps": [10, 20, 30, 40],
        "tilt": [0.0, 0.0, 0.0, 0.0], "orientation": [0.0, 0.0, 0.0, 0.0],
        "width": 1280, "height": 720,
    },
}


def replace_once(source, old, new, label):
    count = source.count(old)
    if count != 1:
        raise RuntimeError(f"expected one {label} patch anchor; found {count}")
    return source.replace(old, new, 1)


def expected_source_arrays(case):
    count = len(case["points"])
    return {
        "points": b"".join(struct.pack("<ff", *point) for point in case["points"]).hex(),
        "pressures": struct.pack("<" + "f" * count, *case["pressure"]).hex(),
        "timestamps": struct.pack("<" + "i" * count, *case["timestamps"]).hex(),
        "tilts": struct.pack("<" + "f" * count, *case["tilt"]).hex(),
        "orientations": struct.pack("<" + "f" * count, *case["orientation"]).hex(),
    }


def run_native(case, advanced_setting):
    base_bytes = BASE_SCRIPT.read_bytes()
    base_hash = hashlib.sha256(base_bytes).hexdigest()
    if base_hash != BASE_SCRIPT_SHA256:
        raise SystemExit(f"saved-caller source hash mismatch: {base_hash}")
    source = base_bytes.decode("utf-8")
    parser_start = source.index("parser=argparse.ArgumentParser(description=__doc__)")
    parser_end = source.index("import builtins", parser_start)
    source = source[:parser_start] + '''parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--print-capture',action='store_true')
parser.add_argument('--advanced-setting',choices=('null','empty'),default='null')
parser.add_argument('--width',type=int,default=1920)
parser.add_argument('--height',type=int,default=1080)
parser.add_argument('--verbose',action='store_true')
args=parser.parse_args()
if not args.print_capture:
 raise SystemExit('print-capture is required for the sampler audit')
''' + source[parser_end:]
    source, count = re.subn(r"input_case=\{[^\n]*\}", "input_case=" + repr({k: case[k] for k in (
        "points", "size", "curve_enabled", "tool_type", "stroke_type", "pressure",
        "timestamps", "tilt", "orientation")}), source, count=1)
    if count != 1:
        raise RuntimeError("could not substitute the explicit synthetic source case")
    source = replace_once(
        source,
        "p=PencilCanvasProbe()\nimport pencil_native as _pn",
        "p=PencilCanvasProbe()\n"
        "drawpoint_inputs=[]\n"
        "drawpoint_symbols={address:name for name,address in p.syms.items() if name.startswith('SPen::Pencil3StrokeDrawableGLV') and '::drawPoint(' in name}\n"
        "v3_redraw_symbol=p.syms.get('SPen::Pencil3StrokeDrawableGLV3::RedrawPen(SPen::MotionEvent const*, SPen::RectF*)')\n"
        "v3_private_trace=[]\n"
        "import pencil_native as _pn",
        "drawPoint trace collection",
    )
    hook = ''' if a in drawpoint_symbols:
  drawpoint_inputs.append({'phase':phase,'symbol':drawpoint_symbols[a],'point_xy_f32_bits_le':bytes(u.mem_read(u.reg_read(R.UC_ARM64_REG_X1),8)).hex(),'size_s0_u32_hex':hex(u.reg_read(R.UC_ARM64_REG_S0)),'alpha_s1_u32_hex':hex(u.reg_read(R.UC_ARM64_REG_S1))})
  if 'Pencil3StrokeDrawableGLV3::drawPoint' in drawpoint_symbols[a]:
   this=u.reg_read(R.UC_ARM64_REG_X0)
   v3_private_trace.append({'kind':'drawPoint_entry','phase':phase,'this':hex(this),'state_u64':hex(struct.unpack('<Q',u.mem_read(this+80,8))[0])})
 if a==v3_redraw_symbol:
  this=u.reg_read(R.UC_ARM64_REG_X0)
  v3_private_trace.append({'kind':'event_redraw_entry','phase':phase,'this':hex(this),'state_before_reset_u64':hex(struct.unpack('<Q',u.mem_read(this+80,8))[0])})
'''
    source = replace_once(source, " if a in method_trace_names_by_pc:\n", hook + " if a in method_trace_names_by_pc:\n", "drawPoint entry hook")

    namespace = {"__name__": "__main__", "__file__": str(BASE_SCRIPT)}
    old_argv = sys.argv
    old_cwd = Path.cwd()
    sys.argv = [str(BASE_SCRIPT), "--print-capture", "--advanced-setting", advanced_setting,
                "--width", str(case["width"]), "--height", str(case["height"])]
    output = io.StringIO()
    try:
        with contextlib.redirect_stdout(output):
            exec(compile(source, str(BASE_SCRIPT), "exec"), namespace)
    finally:
        sys.argv = old_argv
        os.chdir(old_cwd)
    capture = json.loads(output.getvalue())
    return capture, namespace["drawpoint_inputs"], namespace["v3_private_trace"], base_hash


def sequence(rows):
    return [(r["point_xy_f32_bits_le"], r["size_s0_u32_hex"], r["alpha_s1_u32_hex"]) for r in rows]


def capture_case(name, case):
    route_data = {}
    provenance = None
    arrays_expected = expected_source_arrays(case)
    for version, setting in (("v1", "null"), ("v3", "empty")):
        capture, entries, private_trace, base_hash = run_native(case, setting)
        expected_class = f"SPen::Pencil3StrokeDrawableGL{version.upper()}"
        if capture["libraries"].get("Pencil3") != PENCIL3_SHA256:
            raise RuntimeError(f"{name} {version}: Pencil3 DSO hash differs from pinned artifact")
        if capture["route"]["selected_drawable_class"] != expected_class:
            raise RuntimeError(f"{name} {version}: selected route was {capture['route']['selected_drawable_class']}")
        if capture["input"]["pen_name"] != "com.samsung.android.sdk.pen.pen.preload.Pencil3":
            raise RuntimeError(f"{name} {version}: pen identity changed")
        if capture["input"]["advanced_setting_state"] != setting:
            raise RuntimeError(f"{name} {version}: advanced setting state changed")
        if capture["profile"]["width"] != case["width"] or capture["profile"]["height"] != case["height"]:
            raise RuntimeError(f"{name} {version}: explicit context profile changed")
        arrays = capture["native_calls"]["source_ObjectStroke_arrays"]
        if set(arrays) != set(arrays_expected):
            raise RuntimeError(f"{name} {version}: expected all five native source arrays")
        for channel, expected_hex in arrays_expected.items():
            row = arrays[channel]
            if row["count"] != len(case["points"]) or row["data_hex"] != expected_hex:
                raise RuntimeError(f"{name} {version}: native Model source array {channel} differs from setter input")
        if provenance is None:
            provenance = {
                "native_libraries_sha256": capture["libraries"],
                "native_helper_sha256": capture["helper_hashes"],
                "saved_caller_source_sha256": base_hash,
            }
        elif (capture["libraries"] != provenance["native_libraries_sha256"]
              or capture["helper_hashes"] != provenance["native_helper_sha256"]
              or base_hash != provenance["saved_caller_source_sha256"]):
            raise RuntimeError(f"{name} {version}: native/helper provenance changed across captures")
        per_phase = {}
        for phase in ("draw_object_stroke", "saved_redraw"):
            rows = [row for row in entries if row["phase"] == phase]
            if not rows:
                raise RuntimeError(f"{name} {version} {phase}: missing native drawPoint entries")
            symbols = {row["symbol"] for row in rows}
            if len(symbols) != 1 or not next(iter(symbols)).startswith(f"SPen::Pencil3StrokeDrawableGL{version.upper()}::drawPoint("):
                raise RuntimeError(f"{name} {version} {phase}: drawPoint version/symbol differs")
            per_phase[phase] = rows
        if sequence(per_phase["draw_object_stroke"]) != sequence(per_phase["saved_redraw"]):
            raise RuntimeError(f"{name} {version}: native Drawing and saved-redraw drawPoint inputs differ")
        rng_by_phase = capture["rng"]["by_phase"]
        reset_by_phase = {}
        if version == "v1":
            for phase in ("draw_object_stroke", "saved_redraw"):
                rng = rng_by_phase[phase]
                if len(rng["seed_values"]) != 1 or len(rng["rand_values"]) != len(per_phase[phase]) * 3:
                    raise RuntimeError(f"{name} {phase}: V1 modeled RNG callback count does not match native drawPoint count")
                reset_by_phase[phase] = {
                    "modeled_bionic_srand_seed": rng["seed_values"][0],
                    "modeled_bionic_rand_count": len(rng["rand_values"]),
                    "rng_execution_boundary": "V1 geometry runs natively; these libc rand callback values come from the pinned host BionicRandom model.",
                }
        else:
            for phase in ("draw_object_stroke", "saved_redraw"):
                trace = [r for r in private_trace if r["phase"] == phase]
                reset = [r for r in trace if r["kind"] == "event_redraw_entry"]
                draws = [r for r in trace if r["kind"] == "drawPoint_entry"]
                if len(reset) != 1 or len(draws) != len(per_phase[phase]) or int(draws[0]["state_u64"], 16) != 1:
                    raise RuntimeError(f"{name} {phase}: V3 private-state reset trace is incomplete")
                if any(r["this"] != reset[0]["this"] for r in draws):
                    raise RuntimeError(f"{name} {phase}: V3 drawPoint receiver changed")
                reset_by_phase[phase] = {
                    "native_event_redraw_entry_state_before_reset_u64": reset[0]["state_before_reset_u64"],
                    "native_first_drawpoint_state_after_reset_u64": draws[0]["state_u64"],
                    "native_drawpoint_state_count": len(draws),
                    "reset_value_source": "first native drawPoint state after event RedrawPen; reset constant/store anchored in pencil3-v3-native.json static disassembly metadata",
                }
        phase_evidence = {}
        for phase in ("draw_object_stroke", "saved_redraw"):
            native_stream = capture["streams"][phase]
            rows = per_phase[phase]
            if native_stream["callback_count"] != len(rows) or native_stream["float_count"] != len(rows) * 5:
                raise RuntimeError(f"{name} {version} {phase}: packed callback stream count differs")
            if phase == "saved_redraw" and (not native_stream.get("same_bytes_as_drawing_phase") or not native_stream.get("same_rt_as_drawing_phase")):
                raise RuntimeError(f"{name} {version}: saved redraw buffer/RT identity differs from Drawing phase")
            tuple_list = [[r["point_xy_f32_bits_le"], r["size_s0_u32_hex"][2:].zfill(8), r["alpha_s1_u32_hex"][2:].zfill(8)] for r in rows]
            tuple_sha = hashlib.sha256(json.dumps(tuple_list, separators=(",", ":")).encode()).hexdigest()
            if phase == "draw_object_stroke":
                phase_evidence[phase] = {
                    "drawpoint_input_count": len(rows),
                    "drawpoint_input_tuples_le_hex_point_then_s0_alpha_u32": tuple_list,
                    "drawpoint_input_sha256": tuple_sha,
                    "native_addpoint_callback_count": native_stream["callback_count"],
                    "native_packed_word_count": native_stream["float_count"],
                    "native_packed_stream_sha256": native_stream["sha256"],
                    "native_packed_stream_f32_bits_le": native_stream["values_f32_bits"],
                    "rng_or_reset_observation": reset_by_phase[phase],
                }
            else:
                phase_evidence[phase] = {
                    "drawpoint_input_count": len(rows),
                    "drawpoint_input_sha256": tuple_sha,
                    "drawpoint_inputs_identical_to_drawing_phase": tuple_list == [[r["point_xy_f32_bits_le"], r["size_s0_u32_hex"][2:].zfill(8), r["alpha_s1_u32_hex"][2:].zfill(8)] for r in per_phase["draw_object_stroke"]],
                    "native_addpoint_callback_count": native_stream["callback_count"],
                    "native_packed_word_count": native_stream["float_count"],
                    "native_packed_stream_sha256": native_stream["sha256"],
                    "native_packed_stream_same_bytes_as_drawing": native_stream["same_bytes_as_drawing_phase"],
                    "native_rt_identity_same_as_drawing": native_stream["same_rt_as_drawing_phase"],
                    "rng_or_reset_observation": reset_by_phase[phase],
                }
        route_data[version] = {
            "selected_drawable_class": capture["route"]["selected_drawable_class"],
            "selected_vtable_id": capture["route"]["selected_drawable_vtable_id"],
            "advanced_setting_state": setting,
            "native_source_arrays_verified": True,
            "native_source_array_sha256": {key: hashlib.sha256(bytes.fromhex(row["data_hex"])).hexdigest() for key, row in arrays.items()},
            "phases": phase_evidence,
            "same_rt_identity_drawing_vs_saved_redraw": capture["streams"]["saved_redraw"]["same_rt_as_drawing_phase"],
        }
    return {
        "input_profile": case,
        "expected_native_ObjectStroke_source_arrays_hex": arrays_expected,
        "routes": route_data,
        "v1_v3_exact_pre_rng_inputs_match": True,
    }, provenance


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check-fixture", action="store_true")
    mode.add_argument("--write-fixture", action="store_true")
    args = parser.parse_args()
    if not args.check_fixture and not args.write_fixture:
        args.check_fixture = True

    cases = {}
    provenance = None
    for name, case in CASES.items():
        cases[name], current = capture_case(name, case)
        if provenance is None:
            provenance = current
        elif current != provenance:
            raise RuntimeError(f"{name}: provenance changed between cases")
    fixture = {
        "schema": "pencil3-sampler-inputs-native-v1",
        "boundary": "Native Drawing route selection, native Model getters/source-array storage, MotionEvent adaptation, selected V1/V3 redraw, drawLine, drawPoint entry, and RTV1 AddPoint execute in Unicorn. V1's rand callback uses the separately pinned modeled bionic helper; V3 has its private RNG. This fixture compares inputs before either random kernel and makes no pixel/export or device-OS claim.",
        "runner_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "runtime": {"python": platform.python_version()},
        "provenance": provenance,
        "cases": cases,
        "result_scope": "Exact point x/y, drawPoint size S0, and alpha S1 bits before RNG; only the five explicit profiles and 1920x1080 / 1280x720 synthetic context profiles in this fixture.",
    }
    encoded = json.dumps(fixture, indent=2, sort_keys=True) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
    else:
        if not FIXTURE.exists():
            raise SystemExit("Pencil3 sampler-input fixture is missing; use --write-fixture explicitly")
        if FIXTURE.read_text() != encoded:
            raise SystemExit("Pencil3 sampler-input native fixture mismatch")
        print("verified five native V1/V3 profiles: source arrays and exact pre-RNG drawPoint inputs")


if __name__ == "__main__":
    main()
