#!/usr/bin/env python3
"""Verify Pencil3 V3 private-state sampling through the pinned native caller.

Run with the repository's pinned Unicorn and AArch64 runtime. By default this
reruns native Drawing selection and the explicit saved-redraw call, then checks
the compact fixture. Native hooks cover drawPoint entry, each internal 64-bit
MADD result, and RTV1 AddPoint; host context and source inputs remain explicit.
"""
import argparse
import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import struct
import sys

ROOT = Path(__file__).resolve().parents[1]
BASE_SCRIPT = ROOT / "conformance/pencil3_saved_caller_native.py"
FIXTURE = ROOT / "conformance/pencil3-v3-native.json"
BASE_SCRIPT_SHA256 = "129001ce587c104e22e1675e759df9a04d942e884a93fb747b195d001b6a776e"
PENCIL3_SHA256 = "41a995cac7a597af53359401df26187d4299221f3460c91b56540596fc73d359"


def replace_once(source, old, new, label):
    count = source.count(old)
    if count != 1:
        raise RuntimeError(f"expected one patch anchor for {label}, found {count}")
    return source.replace(old, new, 1)


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def f32_hex(value):
    return struct.pack("<f", f32(value)).hex()


def run_native(width, height):
    base_bytes = BASE_SCRIPT.read_bytes()
    base_hash = hashlib.sha256(base_bytes).hexdigest()
    if base_hash != BASE_SCRIPT_SHA256:
        raise SystemExit(f"saved caller hash mismatch: {base_hash}")
    source = base_bytes.decode("utf-8")

    parser_start = source.index("parser=argparse.ArgumentParser(description=__doc__)")
    parser_end = source.index("import builtins", parser_start)
    parser_replacement = '''parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--print-capture',action='store_true')
parser.add_argument('--advanced-setting',choices=('null','empty'),default='empty')
parser.add_argument('--width',type=int,default=1920)
parser.add_argument('--height',type=int,default=1080)
parser.add_argument('--verbose',action='store_true')
args=parser.parse_args()
if not args.print_capture or args.advanced_setting != 'empty':
 raise SystemExit('patched saved caller requires print-capture and explicit empty advanced setting')
'''
    source = source[:parser_start] + parser_replacement + source[parser_end:]
    source = replace_once(
        source,
        "p=PencilCanvasProbe()\nimport pencil_native as _pn",
        "p=PencilCanvasProbe()\n"
        "v3_state_trace=[]\n"
        "v3_add_args=[]\n"
        "v3_madd_states=[]\n"
        "v3_draw_indices={'draw_object_stroke':-1,'saved_redraw':-1}\n"
        "v3_drawpoint_symbol=p.syms.get('SPen::Pencil3StrokeDrawableGLV3::drawPoint(SPen::SmPoint*, float, float, SPen::RectF*)')\n"
        "v3_madd_pc_rva={v3_drawpoint_symbol+delta:rva for delta,rva in ((0x68,'0x10b798'),(0x74,'0x10b7a4'),(0x84,'0x10b7b4'))}\n"
        "import pencil_native as _pn",
        "native trace collections",
    )
    trace_hook = ''' if a==p.syms.get('SPen::Pencil3StrokeDrawableRTV1::AddPoint(SPen::Vector3<float>, float, float)'):
  v3_add_args.append({'phase':phase,'this':hex(u.reg_read(R.UC_ARM64_REG_X0)),'s0_s4':[hex(u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))) for i in range(5)]})
 if a==p.syms.get('SPen::Pencil3StrokeDrawableGLV3::RedrawPen(SPen::MotionEvent const*, SPen::RectF*)'):
  this=u.reg_read(R.UC_ARM64_REG_X0)
  v3_state_trace.append({'kind':'event_redraw_entry','phase':phase,'this':hex(this),'state_before_seed_u64':hex(struct.unpack('<Q',u.mem_read(this+80,8))[0])})
 if a==p.syms.get('SPen::Pencil3StrokeDrawableGLV3::drawPoint(SPen::SmPoint*, float, float, SPen::RectF*)'):
  this=u.reg_read(R.UC_ARM64_REG_X0)
  v3_draw_indices[phase]+=1
  v3_state_trace.append({'kind':'drawPoint_entry','phase':phase,'index':v3_draw_indices[phase],'this':hex(this),'private_state_before_u64':hex(struct.unpack('<Q',u.mem_read(this+80,8))[0]),'point_ptr':hex(u.reg_read(R.UC_ARM64_REG_X1)),'point_xy_f32_bits_le':bytes(u.mem_read(u.reg_read(R.UC_ARM64_REG_X1),8)).hex(),'size_s0_u32_hex':hex(u.reg_read(R.UC_ARM64_REG_S0)),'alpha_s1_u32_hex':hex(u.reg_read(R.UC_ARM64_REG_S1))})
 if a in v3_madd_pc_rva:
  v3_madd_states.append({'phase':phase,'drawpoint_index':v3_draw_indices[phase],'pc_rva':v3_madd_pc_rva[a],'state_u64':hex(u.reg_read(R.UC_ARM64_REG_X8))})
'''
    source = replace_once(source, " if a in method_trace_names_by_pc:\n", trace_hook + " if a in method_trace_names_by_pc:\n", "V3 native instruction hooks")

    namespace = {"__name__": "__main__", "__file__": str(BASE_SCRIPT)}
    old_argv = sys.argv
    old_cwd = Path.cwd()
    old_env = os.environ.get("PYTHONPATH")
    sys.argv = [str(BASE_SCRIPT), "--print-capture", "--advanced-setting", "empty",
                "--width", str(width), "--height", str(height)]
    output = io.StringIO()
    try:
        with contextlib.redirect_stdout(output):
            exec(compile(source, str(BASE_SCRIPT), "exec"), namespace)
    finally:
        sys.argv = old_argv
        os.chdir(old_cwd)
        if old_env is None:
            os.environ.pop("PYTHONPATH", None)
        else:
            os.environ["PYTHONPATH"] = old_env
    capture = json.loads(output.getvalue())
    if capture["libraries"]["Pencil3"] != PENCIL3_SHA256:
        raise RuntimeError("native Pencil3 DSO hash differs from pinned image")
    return capture, namespace, base_hash


def compact_capture(capture, namespace, base_hash, runner_hash, width, height):
    if capture["route"]["selected_drawable_class"] != "SPen::Pencil3StrokeDrawableGLV3":
        raise RuntimeError("explicit empty setting did not select Pencil3 V3")
    if capture["input"]["advanced_setting_state"] != "empty" or capture["input"]["advanced_getter_utf8"] != "":
        raise RuntimeError("capture did not preserve explicit empty advanced setting")
    if capture["rng"]["calls"]:
        raise RuntimeError("V3 unexpectedly invoked modeled libc rand/srand callbacks")
    if not capture["streams"]["saved_redraw"]["same_bytes_as_drawing_phase"]:
        raise RuntimeError("native saved-redraw buffer differs from Drawing phase")
    if not capture["streams"]["saved_redraw"]["same_rt_as_drawing_phase"]:
        raise RuntimeError("native saved-redraw RTV identity differs from Drawing phase")

    input_data = capture["input"]
    count = len(input_data["timestamps_i32"])
    # Compare native Model-owned source memory against the actual setter data.
    expected_arrays = {
        "points": b"".join(bytes.fromhex("".join(pair)) for pair in input_data["points_f32_bits"]),
        "pressures": b"".join(bytes.fromhex(value) for value in input_data["pressure_f32_bits"]),
        "timestamps": b"".join(struct.pack("<i", value) for value in input_data["timestamps_i32"]),
        "tilts": b"".join(bytes.fromhex(value) for value in input_data["tilt_f32_bits"]),
        "orientations": b"".join(bytes.fromhex(value) for value in input_data["orientation_f32_bits"]),
    }
    arrays = capture["native_calls"]["source_ObjectStroke_arrays"]
    if set(arrays) != set(expected_arrays):
        raise RuntimeError(f"expected five native source arrays; got {sorted(arrays)}")
    for name, expected in expected_arrays.items():
        row = arrays[name]
        if row["count"] != count or row["data_hex"] != expected.hex():
            raise RuntimeError(f"native ObjectStroke {name} source array differs from setter data")

    trace = namespace["v3_state_trace"]
    add_args = namespace["v3_add_args"]
    madds = namespace["v3_madd_states"]
    stream = capture["streams"]["draw_object_stroke"]["values_f32_bits"]
    if len(stream) != 240 or len(add_args) != 96:
        raise RuntimeError(f"expected full 240-word stream and 96 AddPoint callbacks; got {len(stream)} and {len(add_args)}")
    state_contract = {
        "state_offset_bytes": 80,
        "constructor_initial_state_u64": "0x0000000000000001",
        "redraw_reset_constant_u64": "0x0000000000000001",
        "constructor_store_instruction": {"rva": "0x10a498", "word_le": "7fda04a9"},
        "redraw_reset_instructions": [
            {"rva": "0x10b118", "word_le": "28008052", "meaning": "mov w8,#1"},
            {"rva": "0x10b120", "word_le": "082800f9", "meaning": "str x8,[x0,#80]"},
        ],
        "madd_sequence": [
            {"after_instruction_rva": "0x10b794", "hook_rva": "0x10b798", "word_le": "0829099b"},
            {"after_instruction_rva": "0x10b7a0", "hook_rva": "0x10b7a4", "word_le": "0829099b"},
            {"after_instruction_rva": "0x10b7b0", "hook_rva": "0x10b7b4", "word_le": "0829099b"},
        ],
        "multiplier_u64": "0x00000000000343fd",
        "increment_u64": "0x0000000000269ec3",
        "wrap_mask_u64": "0xffffffffffffffff",
        "advances_per_particle": 3,
        "range_expression": "fma(size+1,min(size+1,5),30)",
        "xy_expression": "((state >> 16) & 0x7fffffff) % trunc(range) / 20 - trunc(range)/40",
        "row_expression": "(third_state >> 16) & 0xf",
        "AddPoint_arg_order": "x,y,row,size+1,alpha",
        "packed_order": "x,y,(size+1)/2,row,alpha",
        "internal_states_evidence": "Native Unicorn PC hooks capture x8 immediately after each of the three MADD instructions; sequence model is checked against these captured values.",
    }
    phases = {}
    model_words_by_phase = {}
    for phase in ("draw_object_stroke", "saved_redraw"):
        event = [r for r in trace if r["kind"] == "event_redraw_entry" and r["phase"] == phase]
        points = [r for r in trace if r["kind"] == "drawPoint_entry" and r["phase"] == phase]
        calls = [r for r in add_args if r["phase"] == phase]
        native_madds = [r for r in madds if r["phase"] == phase]
        if len(event) != 1 or len(points) != 48 or len(calls) != 48 or len(native_madds) != 144:
            raise RuntimeError(f"{phase}: incomplete native trace: event={len(event)} drawPoint={len(points)} AddPoint={len(calls)} MADD={len(native_madds)}")
        if any(r["this"] != event[0]["this"] for r in points):
            raise RuntimeError(f"{phase}: V3 drawable receiver changed")
        if int(points[0]["private_state_before_u64"], 16) != 1:
            raise RuntimeError(f"{phase}: first drawPoint did not observe reset state 1")
        add_receivers = {r["this"] for r in calls}
        if len(add_receivers) != 1:
            raise RuntimeError(f"{phase}: RTV AddPoint receiver changed")
        state = 1
        model_words = []
        records = []
        for index, (point, call) in enumerate(zip(points, calls)):
            if point["index"] != index or int(point["private_state_before_u64"], 16) != state:
                raise RuntimeError(f"{phase} particle {index}: native entry state/index differs")
            observed = native_madds[index * 3:index * 3 + 3]
            pcs = ["0x10b798", "0x10b7a4", "0x10b7b4"]
            if [r["pc_rva"] for r in observed] != pcs or any(r["drawpoint_index"] != index for r in observed):
                raise RuntimeError(f"{phase} particle {index}: internal MADD hook boundaries differ")
            observed_states = [int(r["state_u64"], 16) for r in observed]
            modeled_states = []
            for _ in range(3):
                state = (state * 0x343fd + 0x269ec3) & ((1 << 64) - 1)
                modeled_states.append(state)
            if observed_states != modeled_states:
                raise RuntimeError(f"{phase} particle {index}: native MADD states differ from recurrence")

            px, py = struct.unpack("<ff", bytes.fromhex(point["point_xy_f32_bits_le"]))
            size = struct.unpack("<f", struct.pack("<I", int(point["size_s0_u32_hex"], 16)))[0]
            alpha = struct.unpack("<f", struct.pack("<I", int(point["alpha_s1_u32_hex"], 16)))[0]
            size1 = f32(size + 1.0)
            range_value = f32(size1 * min(size1, 5.0) + 30.0)
            divisor = int(range_value)
            row = (observed_states[2] >> 16) & 0xf
            dx = f32(((observed_states[0] >> 16) & 0x7fffffff) % divisor / 20.0)
            dy = f32(((observed_states[1] >> 16) & 0x7fffffff) % divisor / 20.0)
            x = f32(f32(px + dx) - f32(divisor / 40.0))
            y = f32(f32(py + dy) - f32(divisor / 40.0))
            expected_args_le = [struct.pack("<f", v).hex() for v in (x, y, float(row), size1, alpha)]
            native_args = [int(v, 16) for v in call["s0_s4"]]
            expected_args = [int.from_bytes(bytes.fromhex(v), "little") for v in expected_args_le]
            if native_args != expected_args:
                raise RuntimeError(f"{phase} particle {index}: native AddPoint arguments differ")
            packed = [struct.pack("<f", v).hex() for v in (x, y, size1 * 0.5, float(row), alpha)]
            model_words.extend(packed)
            records.append({
                "native_private_state_before_u64": point["private_state_before_u64"],
                "native_private_state_after_each_madd_u64": [f"0x{v:016x}" for v in observed_states],
                "model_checked_state_after_each_madd_u64": [f"0x{v:016x}" for v in modeled_states],
                "native_drawpoint_input": {
                    "point_xy_f32_bits_le": point["point_xy_f32_bits_le"],
                    "size_s0_u32_hex": point["size_s0_u32_hex"],
                    "alpha_s1_u32_hex": point["alpha_s1_u32_hex"],
                },
                "native_addpoint_s0_s4_u32_hex": call["s0_s4"],
                "modeled_packed_words_f32_bits_le": packed,
            })
        model_words_by_phase[phase] = model_words
        phases[phase] = {
            "redraw_entry_state_before_reset_u64": event[0]["state_before_seed_u64"],
            "first_drawpoint_state_after_reset_u64": points[0]["private_state_before_u64"],
            "particle_count": len(records),
            "records": records,
            "native_stream_sha256": capture["streams"][phase]["sha256"],
            "native_packed_stream_f32_bits_le": stream if phase == "draw_object_stroke" else None,
            "saved_stream_same_as_drawing": capture["streams"][phase].get("same_bytes_as_drawing_phase"),
            "modeled_final_private_state_u64": f"0x{state:016x}",
        }
    if model_words_by_phase["draw_object_stroke"] != stream:
        raise RuntimeError("kernel model differs from all 240 native packed Drawing words")
    if model_words_by_phase["saved_redraw"] != model_words_by_phase["draw_object_stroke"]:
        raise RuntimeError("saved-redraw modeled packed words differ from Drawing phase")
    if phases["saved_redraw"]["native_stream_sha256"] != phases["draw_object_stroke"]["native_stream_sha256"]:
        raise RuntimeError("saved-redraw and Drawing native stream hashes differ")

    helpers = {
        "runner_sha256": runner_hash,
        "base_saved_caller_sha256": base_hash,
        **capture["helper_hashes"],
    }
    # Explicitly identify the wrapper patch dependency even though its digest
    # also appears as base_saved_caller_sha256.
    return {
        "schema": "pencil3-v3-private-sampling-native-v1",
        "case_id": "straight_curve_tool2_empty",
        "scope": "Actual Drawing-selected Pencil3 V3 with explicit empty advanced setting and synthetic source/context inputs only.",
        "profile": capture["profile"],
        "input": input_data,
        "source_ObjectStroke_arrays": arrays,
        "native_model_getters": capture["native_model_getters"],
        "selected_route": {
            "drawable_class": capture["route"]["selected_drawable_class"],
            "drawable_vtable_id": capture["route"]["selected_drawable_vtable_id"],
            "selected_redraw_entry": capture["route"]["redraw_entry"],
            "drawing_return": capture["route"]["drawing_return"],
            "saved_redraw_return": capture["route"]["saved_redraw_return"],
            "same_rt_identity": capture["route"]["same_rt_id"],
        },
        "binary_sha256": capture["libraries"],
        "helper_sha256": helpers,
        "native_libc_rng_callback_count": 0,
        "v3_state_contract": state_contract,
        "phases": phases,
        "repeat_same_instance": {
            "same_rt_identity": capture["route"]["same_rt_id"],
            "pre_second_redraw_state_u64": phases["saved_redraw"]["redraw_entry_state_before_reset_u64"],
            "first_drawpoint_state_each_phase_u64": [phases[k]["first_drawpoint_state_after_reset_u64"] for k in ("draw_object_stroke", "saved_redraw")],
            "same_native_stream_sha256": phases["draw_object_stroke"]["native_stream_sha256"] == phases["saved_redraw"]["native_stream_sha256"],
        },
        "host_boundaries": capture["host_boundaries"],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check-fixture", action="store_true")
    mode.add_argument("--write-fixture", action="store_true")
    mode.add_argument("--print-capture", action="store_true")
    parser.add_argument("--width", type=int, default=1920)
    parser.add_argument("--height", type=int, default=1080)
    args = parser.parse_args()
    if not (args.print_capture or args.check_fixture or args.write_fixture):
        args.check_fixture = True
    if not (0 < args.width < 65536 and 0 < args.height < 65536):
        raise SystemExit("profile dimensions must be positive u16 values")
    capture, namespace, base_hash = run_native(args.width, args.height)
    runner_hash = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    fixture = compact_capture(capture, namespace, base_hash, runner_hash, args.width, args.height)
    encoded = json.dumps(fixture, indent=2, sort_keys=True) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
    elif args.check_fixture:
        if not FIXTURE.exists():
            raise SystemExit("Pencil3 V3 fixture is missing; use --write-fixture explicitly")
        if FIXTURE.read_text() != encoded:
            raise SystemExit("Pencil3 V3 native fixture mismatch")
        print("verified Pencil3 V3 private sampler: 48 particles x 2 phases, native MADD states and all 240 packed words")
    else:
        print(encoded, end="")


if __name__ == "__main__":
    main()
