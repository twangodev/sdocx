#!/usr/bin/env python3
"""Replay native InkPen V1 redraw/sample boundaries across drawable reuse cases.

The inputs are synthetic ObjectStroke setter calls. Each run starts from the
pinned reuse caller, applies exact in-memory patches, and captures live native
RT vector ranges at reDrawLine/endPen entry and return. The default command
replays and checks the fixture; writing requires --write-fixture.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "conformance/inkpen-v1-reuse-native.json"
PINNED_RUNNER = ROOT / "conformance/inkpen_reuse_native.py"
CALLER = ROOT / "conformance/inkpen_saved_caller_native.py"
NUMERIC = ROOT / "conformance/inkpen-v1-saved-native.json"
LIBRARY_DIR = ROOT / "scratch/apk-analysis-native/arm64-v8a"

PINS = {
    "conformance/inkpen_reuse_native.py": "7ec03bfb57549e9f29fb42cb484e1469d857e2396288864955f13f85552ce5aa",
    "conformance/inkpen_saved_caller_native.py": "c0b1657cdcf94bcf2c173ce68ed7817e55a4de246883b47265bb7b4d3f50dc04",
    "conformance/inkpen-v1-saved-native.json": "ade2af7ee7c7c07d4663151074932009f7def1a91ba4c14a4c55c294a3e32fb4",
    "conformance/elf_plt.py": "816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b",
    "conformance/fountain_native.py": "40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30",
    "conformance/oblique_canvas_native.py": "1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2",
}
DSO_PINS = {
    "libSPenBase.so": "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb",
    "libSPenComposer.so": "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f",
    "libSPenDrawing.so": "788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd",
    "libSPenFountainPen.so": "1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec",
    "libSPenGraphics.so": "aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621",
    "libSPenInkPen.so": "939a7c719838306d693f96701994486b03f5c7eea35f16f25c6a89d84daba401",
    "libSPenModel.so": "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a",
    "libSPenPenCommon.so": "afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d",
    "libc++_shared.so": "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4",
}
REDRAW_LOOP_RVA = (0x2749C, 0x27554)
REDRAW_LOOP_BYTES_SHA256 = "5fb4afb537c32178df98367e9c7e03292335ed08bb04a48d38f663e98e50520a"
PROFILE_ARGS = ["--advanced-setting", "null", "--width", "1440", "--height", "2560"]
CASE_ARGS = {
    "fresh": [],
    "short_first": ["--reuse-first", "short"],
    "long_first": ["--reuse-first", "long"],
    "distinct_object": ["--reuse-first", "short", "--distinct-object"],
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_pins():
    result = {}
    for relative, expected in PINS.items():
        path = ROOT / relative
        actual = digest(path)
        if actual != expected:
            raise RuntimeError(f"pinned helper changed: {relative}: {actual}")
        result[relative] = actual
    for filename, expected in DSO_PINS.items():
        actual = digest(LIBRARY_DIR / filename)
        if actual != expected:
            raise RuntimeError(f"pinned DSO changed: {filename}: {actual}")
        result[f"scratch/apk-analysis-native/arm64-v8a/{filename}"] = actual
    return result


def redraw_loop_evidence():
    """Extract exact RedrawPen init/loop bytes through ELF PT_LOAD mapping."""
    struct = __import__("struct")
    path = LIBRARY_DIR / "libSPenInkPen.so"
    data = path.read_bytes()
    elf = struct.unpack_from("<16sHHIQQQIHHHHHH", data, 0)
    phoff, phentsize, phnum = elf[5], elf[9], elf[10]
    start, end = REDRAW_LOOP_RVA
    file_offset = None
    for index in range(phnum):
        p_type, _flags, p_offset, p_vaddr, _paddr, p_filesz, _memsz, _align = struct.unpack_from(
            "<IIQQQQQQ", data, phoff + index * phentsize
        )
        if p_type == 1 and p_vaddr <= start and end <= p_vaddr + p_filesz:
            file_offset = p_offset + start - p_vaddr
            break
    if file_offset is None:
        raise RuntimeError("RedrawPen init/loop range is not file-backed by an ELF PT_LOAD")
    code = data[file_offset : file_offset + end - start]
    code_sha = hashlib.sha256(code).hexdigest()
    if code_sha != REDRAW_LOOP_BYTES_SHA256:
        raise RuntimeError(f"RedrawPen source loop bytes changed: {code_sha}")
    return {
        "dso": "scratch/apk-analysis-native/arm64-v8a/libSPenInkPen.so",
        "dso_sha256": DSO_PINS["libSPenInkPen.so"],
        "symbol": "SPen::InkPenStrokeDrawableGLV1::RedrawPen(ObjectStroke const*, RectF*)",
        "rva_range_inclusive_start_exclusive_end": [hex(start), hex(end)],
        "pt_load_file_offset": hex(file_offset),
        "bytes_sha256": code_sha,
        "bytes_hex": code.hex(),
        "decoded_anchors": [
            "0x2749c-0x274d0 reads point[0], writes drawable +80/+84 and +104/+108, stores timestamp[0] at +272, and writes zero at +280 (the meaning of +280 is not inferred here).",
            "0x274e4 computes point_count-1; 0x274fc initializes the point cursor at points+12 and 0x27500 initializes pressure at pressure+4, so the loop starts at source point index 1.",
            "0x2752c clamps effective pressure to 1; tool types 1 and 3 select the 0.5 constant at 0x2750c-0x27524, while other tools load source pressure at 0x27528.",
            "0x27540 calls reDrawLine; 0x27544-0x27550 advances XY by 8 bytes and pressure by 4 bytes until point_count-1 samples have run.",
            "0x27594 onward constructs the terminal MotionEvent using index point_count-1; 0x27614 calls endPen after the sample loop.",
        ],
    }


def replace_once(source, old, new, label):
    count = source.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one source anchor, found {count}")
    return source.replace(old, new, 1)


def instrument_v1_trace(module):
    original = module.patched_caller_source
    old_block = (
        "  if any(name.startswith('SPen::InkPenStrokeDrawableGLV4::RedrawPen(SPen::ObjectStroke const*') for name in method_trace_names_by_pc[a]):\n"
        "   this=u.reg_read(R.UC_ARM64_REG_X0)\n"
        "   snapshot={'phase':phase,'entry_pc':hex(a),'receiver_address':hex(this),'receiver_role':'selected primary drawable after native non-virtual thunk adjustment'}\n"
        "   for label,offset in (('tolerance_plus_44_f32_bits',44),('spacing_plus_88_f32_bits',88),('residual_plus_92_f32_bits',92),('tool_threshold_plus_96_f32_bits',96)):\n"
        "    snapshot[label]=bytes(u.mem_read(this+offset,4)).hex()\n"
        "   snapshot['alternation_plus_100_u8']=u.mem_read(this+100,1)[0]\n"
        "   drawable_entry_snapshots.append(snapshot)\n"
    )
    new_block = (
        "  for method in method_trace_names_by_pc[a]:\n"
        "   if 'SPen::InkPenStrokeDrawableGLV1::' in method:\n"
        "    this=u.reg_read(R.UC_ARM64_REG_X0)\n"
        "    if not any(s.get('phase')==phase and s.get('method')==method for s in drawable_entry_snapshots):\n"
        "     drawable_entry_snapshots.append({'phase':phase,'method':method,'entry_pc':hex(a),'receiver_address':hex(this),'receiver_bytes_hex':bytes(u.mem_read(this,0x200)).hex()})\n"
        "  if any(name.startswith('SPen::InkPenStrokeDrawableGLV4::RedrawPen(SPen::ObjectStroke const*') for name in method_trace_names_by_pc[a]):\n"
        "   this=u.reg_read(R.UC_ARM64_REG_X0)\n"
        "   snapshot={'phase':phase,'entry_pc':hex(a),'receiver_address':hex(this),'receiver_role':'selected primary drawable after native non-virtual thunk adjustment'}\n"
        "   for label,offset in (('tolerance_plus_44_f32_bits',44),('spacing_plus_88_f32_bits',88),('residual_plus_92_f32_bits',92),('tool_threshold_plus_96_f32_bits',96)):\n"
        "    snapshot[label]=bytes(u.mem_read(this+offset,4)).hex()\n"
        "   snapshot['alternation_plus_100_u8']=u.mem_read(this+100,1)[0]\n"
        "   drawable_entry_snapshots.append(snapshot)\n"
    )
    source = original()
    if source.count(old_block) != 1:
        raise RuntimeError("pinned caller's original RedrawPen capture anchor changed")
    source = source.replace(old_block, new_block, 1)
    method_anchor = "('::Draw(','::RedrawPen(','::drawLine(','::drawPoint(','::startPen(','::endPen(')))"
    method_replacement = "('::Draw(','::RedrawPen(','::reDrawLine(','::drawLine(','::drawPoint(','::startPen(','::endPen(')))"
    source = replace_once(source, method_anchor, method_replacement, "native reDrawLine method registration")
    source = replace_once(
        source,
        "native_draw_entries=[]\n",
        "native_draw_entries=[]\nnative_redraw_sample_ranges=[]\npending_redraw_sample_returns={}\nredraw_sample_indices={}\n",
        "native sample boundary state",
    )
    source = replace_once(
        source,
        "  record={'signatures':method_trace_names_by_pc[a],'phase':phase,'pc':hex(a),'regs':regs,'pointer_heads':pointers}\n",
        "  record={'signatures':method_trace_names_by_pc[a],'phase':phase,'pc':hex(a),'regs':regs,'pointer_heads':pointers}\n"
        "  for signature in method_trace_names_by_pc[a]:\n"
        "   if 'SPen::InkPenStrokeDrawableGLV1::reDrawLine(' in signature or 'SPen::InkPenStrokeDrawableGLV1::endPen(' in signature:\n"
        "    this=u.reg_read(R.UC_ARM64_REG_X0); rt=struct.unpack('<Q',u.mem_read(this+16,8))[0]; vector_obj=struct.unpack('<Q',u.mem_read(rt+80,8))[0]; begin,end,capacity=struct.unpack('<3Q',u.mem_read(vector_obj,24)); before=end-begin\n"
        "    if not (begin<=end<=capacity) or before%12: raise RuntimeError(f'invalid V1 sample vector at entry {begin:#x}/{end:#x}/{capacity:#x}')\n"
        "    line_event='reDrawLine' if '::reDrawLine(' in signature else 'endPen'; sample_index=redraw_sample_indices.get((phase,line_event),0); redraw_sample_indices[(phase,line_event)]=sample_index+1\n"
        "    sample={'method':line_event,'signature':signature,'phase':phase,'source_history_index':sample_index+1 if line_event=='reDrawLine' else None,'entry_pc':hex(a),'receiver_address':hex(this),'rt':hex(rt),'vector_obj':vector_obj,'vector_object':hex(vector_obj),'before_count_bytes':before,'before_record_count':before//12,'args_s0_s2_bits':[hex(u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))) for i in range(3)],'raw_x3_register':hex(u.reg_read(R.UC_ARM64_REG_X3)),'return_pc':hex(u.reg_read(R.UC_ARM64_REG_LR))}\n"
        "    pending_redraw_sample_returns[u.reg_read(R.UC_ARM64_REG_LR)]=sample\n"
        "    native_redraw_sample_ranges.append(sample)\n",
        "native V1 sample entry/RT snapshot",
    )
    source = replace_once(
        source,
        " if a in pending_inkpen_buffer_returns:\n",
        " if a in pending_redraw_sample_returns:\n"
        "  sample=pending_redraw_sample_returns.pop(a); vector_obj=sample['vector_obj']; begin,end,capacity=struct.unpack('<3Q',u.mem_read(vector_obj,24)); after=end-begin\n"
        "  if not (begin<=end<=capacity) or after%12 or after<sample['before_count_bytes']: raise RuntimeError(f'invalid V1 sample vector at return {begin:#x}/{end:#x}/{capacity:#x}')\n"
        "  sample.update({'return_pc':hex(a),'after_count_bytes':after,'after_record_count':after//12,'added_record_count':(after-sample['before_count_bytes'])//12,'after_stream_bytes':bytes(u.mem_read(begin,after)).hex() if after else ''})\n"
        " if a in pending_inkpen_buffer_returns:\n",
        "native V1 sample return/full bytes",
    )
    source = replace_once(
        source,
        "'drawable_entry_snapshots':drawable_entry_snapshots,",
        "'drawable_entry_snapshots':drawable_entry_snapshots,'native_redraw_sample_ranges':native_redraw_sample_ranges,",
        "emit native V1 sample ranges",
    )
    return source


def _packed_float_bits(value):
    return __import__("struct").pack("<f", value).hex()


def _input_arrays(input_row):
    return {
        "points": b"".join(bytes.fromhex(v) for point in input_row["points_f32_bits"] for v in point),
        "pressures": b"".join(bytes.fromhex(v) for v in input_row["pressure_f32_bits"]),
        "timestamps": b"".join(__import__("struct").pack("<i", v) for v in input_row["timestamps_i32"]),
        "tilts": b"".join(bytes.fromhex(v) for v in input_row["tilt_f32_bits"]),
        "orientations": b"".join(bytes.fromhex(v) for v in input_row["orientation_f32_bits"]),
    }


def _check_object_arrays(arrays, input_row, label):
    expected = _input_arrays(input_row)
    for channel, raw in expected.items():
        actual = arrays[channel]
        if bytes.fromhex(actual["data_hex"]) != raw:
            raise RuntimeError(f"{label} native ObjectStroke getter bytes differ for {channel}")
        if actual["count"] != len(raw) // actual["element_size"]:
            raise RuntimeError(f"{label} native ObjectStroke getter count differs for {channel}")


def _reg_to_le_bits(value):
    return int(value, 16).to_bytes(4, "little").hex()


def _check_native_phase(row, phase, points_bits, full_stream):
    records = row["reuse"]["native_redraw_sample_ranges"]
    phase_rows = [sample for sample in records if sample["phase"] == phase]
    redraws = [sample for sample in phase_rows if sample["method"] == "reDrawLine"]
    terminals = [sample for sample in phase_rows if sample["method"] == "endPen"]
    point_count = len(points_bits)
    if len(redraws) != point_count - 1 or len(terminals) != 1:
        raise RuntimeError(f"{phase}: expected {point_count - 1} redraw samples then one endPen")
    expected_indices = list(range(1, point_count))
    if [sample["source_history_index"] for sample in redraws] != expected_indices:
        raise RuntimeError(f"{phase}: RedrawPen history indices do not cover 1..N-1")

    previous_count = 0
    for index, sample in enumerate(redraws, 1):
        if sample["before_record_count"] != previous_count:
            raise RuntimeError(f"{phase} point {index}: RT vector prefix is not continuous")
        if sample["entry_pc"] != "0x5026fa8" or sample["return_pc"] != "0x5027544":
            raise RuntimeError(f"{phase} point {index}: unexpected native reDrawLine entry/return")
        x_bits, y_bits = points_bits[index]
        args = sample["args_s0_s2_bits"]
        if _reg_to_le_bits(args[0]) != x_bits or _reg_to_le_bits(args[1]) != y_bits:
            raise RuntimeError(f"{phase} point {index}: native reDrawLine coordinates differ from source arrays")
        # In this bounded tool-type-1 control, V1 passes an effective pressure
        # of 0.5 to reDrawLine even when ObjectStroke pressure is higher.
        if args[2] != "0x3f000000":
            raise RuntimeError(f"{phase} point {index}: unexpected effective V1 pressure argument")
        added = sample["added_record_count"]
        after = sample["after_record_count"]
        if after != previous_count + added or sample["after_count_bytes"] != after * 12:
            raise RuntimeError(f"{phase} point {index}: inconsistent native AddPoint range")
        prefix = bytes.fromhex(sample["after_stream_bytes"])
        if prefix != full_stream[:after * 12]:
            raise RuntimeError(f"{phase} point {index}: entry/return vector bytes differ from final stream prefix")
        previous_count = after

    terminal = terminals[0]
    final_count = len(full_stream) // 12
    if terminal["before_record_count"] != previous_count or terminal["after_record_count"] != final_count:
        raise RuntimeError(f"{phase}: endPen does not begin after history callbacks and finish the RT stream")
    if terminal["entry_pc"] != "0x502599c" or terminal["return_pc"] != "0x5027618":
        raise RuntimeError(f"{phase}: unexpected native endPen entry/return")
    if bytes.fromhex(terminal["after_stream_bytes"]) != full_stream:
        raise RuntimeError(f"{phase}: endPen terminal vector bytes differ from final stream")
    return {
        "redraw_history_indices": expected_indices,
        "redraw_cumulative_record_ends": [sample["after_record_count"] for sample in redraws],
        "end_pen_source_point_index": point_count - 1,
        "end_pen_record_range": [terminal["before_record_count"], final_count],
        "per_point_effective_pressure_f32_bits": [sample["args_s0_s2_bits"][2] for sample in redraws],
        "first_source_point_role": "verified by the pinned RedrawPen byte range and disassembly anchors in fixture-level static evidence",
    }


def validate_case(row, case_id):
    reuse = row["reuse"]
    target = bytes.fromhex(reuse["target_stream_bytes"])
    first = bytes.fromhex(reuse["first_stream_bytes"])
    for flag, field in (("same_pen_object", "pen"), ("same_drawable_object", "drawable"), ("same_rt_object", "rt")):
        if not reuse[flag] or reuse["first_native_state"][field] != reuse["target_native_state"][field]:
            raise RuntimeError(f"{case_id}: {flag} is not backed by matching native pointers")
    expected_first_count = {"fresh": 25, "short_first": 4, "long_first": 86, "distinct_object": 4}[case_id]
    if len(first) != expected_first_count * 12:
        raise RuntimeError(f"{case_id}: first-stroke RT byte length differs from its expected callback count")
    if len(target) != 25 * 12:
        raise RuntimeError(f"{case_id}: target RT stream is not the 25-record V1 control")
    first_input = row["input"] if case_id == "fresh" else reuse["first_input"]
    target_input = row["input"]
    _check_object_arrays(reuse["first_source_ObjectStroke_arrays"], first_input, f"{case_id} first")
    _check_object_arrays(reuse["target_source_ObjectStroke_arrays"], target_input, f"{case_id} target")
    first_points = first_input["points_f32_bits"]
    target_points = target_input["points_f32_bits"]
    first_phase = _check_native_phase(row, "draw_object_stroke", first_points, first)
    target_phase_name = "draw_object_stroke" if case_id == "fresh" else "reuse_second_draw"
    target_phase = _check_native_phase(row, target_phase_name, target_points, target)
    saved_phase = None
    if case_id != "distinct_object":
        saved = bytes.fromhex(next(
            sample["after_stream_bytes"] for sample in reuse["native_redraw_sample_ranges"]
            if sample["phase"] == "saved_redraw" and sample["method"] == "endPen"
        ))
        if saved != target:
            raise RuntimeError(f"{case_id}: direct saved redraw differs from Drawing target stream")
        saved_phase = _check_native_phase(row, "saved_redraw", target_points, saved)
    else:
        if reuse["first_object_address"] == reuse["target_object_address"]:
            raise RuntimeError("distinct-object control reused the source ObjectStroke address")
        if reuse["first_user_id"] != 101 or reuse["target_user_id"] != 202:
            raise RuntimeError("distinct-object control did not retain its native object identity markers")
        saved_rows = [sample for sample in reuse["native_redraw_sample_ranges"] if sample["phase"] == "saved_redraw"]
        saved_callbacks = [event for event in row["runtime_event_order"] if event["phase"] == "saved_redraw" and event["event"] == "AddPoint"]
        if saved_rows or saved_callbacks or row["streams"]["saved_redraw"]["callback_count"] != 0:
            raise RuntimeError("distinct clean target's direct saved redraw unexpectedly emitted callbacks")

    row["v1_reuse_capture"]["native_source_sample_attribution"] = {
        "redraw_line_entry_return_rvas": {"entry": "0x26fa8", "return": "0x27544"},
        "end_pen_entry_return_rvas": {"entry": "0x2599c", "return": "0x27618"},
        "index_basis": "The probe assigns source_history_index from the ordered native reDrawLine calls; every XY register pair is checked against native ObjectStroke getter arrays. Pinned RedrawPen bytes independently show point 0 initialization, point cursor at points+12, pressure cursor at pressure+4, and endPen after the loop at N-1. No native history-index argument is claimed.",
        "input_pressure_note": "The input pressure getter bytes are retained verbatim. For this tool-type-1 V1 control, actual reDrawLine S2 is 0.5 at every history point; it is validated separately as the native effective argument, not represented as raw source pressure. raw_x3_register is preserved without a timestamp interpretation.",
        "drawing_first_stroke": first_phase,
        "drawing_target_stroke": target_phase,
        "saved_redraw": saved_phase,
    }


def capture_case(case_id):
    pins = verify_pins()
    spec = importlib.util.spec_from_file_location("pinned_inkpen_reuse", PINNED_RUNNER)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    if module.CALLER_SHA256 != PINS["conformance/inkpen_saved_caller_native.py"]:
        raise RuntimeError("reuse runner's caller pin differs from the captured caller")
    if digest(module.INKPEN_DSO) != DSO_PINS["libSPenInkPen.so"]:
        raise RuntimeError("reuse runner's InkPen DSO pin differs")
    patched = instrument_v1_trace(module)
    module.patched_caller_source = lambda: patched
    row = module.capture([*PROFILE_ARGS, *CASE_ARGS[case_id]])
    if row["route"]["selected_drawable_class"] != "SPen::InkPenStrokeDrawableGLV1" or row["route"]["selected_drawable_vtable_id"] != "v1":
        raise RuntimeError(f"case did not select native V1: {row['route']}")
    if row["input"]["advanced_setting_state"] != "null":
        raise RuntimeError("case did not use absent/null advanced-setting control")
    if not row["screen_resolution"] or any(value != {"width": 1440, "height": 2560} for value in row["screen_resolution"]):
        raise RuntimeError("case did not use the captured 1440x2560 system-display profile")
    row["v1_reuse_capture"] = {
        "case_id": case_id,
        "consumed_source_hashes": pins,
        "in_memory_patched_caller_sha256": hashlib.sha256(patched.encode()).hexdigest(),
        "profile": {"width": 1440, "height": 2560},
        "scope": "Synthetic ObjectStroke setter inputs through native Drawing with null settings selecting InkPen V1; actual per-sample and terminal RT vector snapshots. No serialized-note admission or device/export parity claim.",
    }
    validate_case(row, case_id)
    return row


def run_case(case_id):
    completed = subprocess.run(
        [sys.executable, str(Path(__file__).resolve()), "--capture", case_id],
        cwd=ROOT,
        env=os.environ.copy(),
        text=True,
        capture_output=True,
    )
    if completed.returncode:
        raise RuntimeError(f"native child for {case_id} failed ({completed.returncode}):\n{completed.stderr}\n{completed.stdout[-3000:]}")
    return json.loads(completed.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--write-fixture", action="store_true", help="explicitly write or refresh the fixture")
    mode.add_argument("--check-fixture", action="store_true", help="replay every case and compare with the fixture")
    parser.add_argument("--capture", choices=tuple(CASE_ARGS), help=argparse.SUPPRESS)
    args = parser.parse_args()
    verify_pins()
    if args.capture:
        print(json.dumps(capture_case(args.capture), sort_keys=True))
        return
    cases = {case_id: run_case(case_id) for case_id in CASE_ARGS}
    reference = bytes.fromhex(cases["fresh"]["reuse"]["target_stream_bytes"])
    for case_id, row in cases.items():
        if bytes.fromhex(row["reuse"]["target_stream_bytes"]) != reference:
            raise RuntimeError(f"{case_id}: target stream differs from fresh native V1 bytes")
    fixture = {
        "schema": "sdocx-inkpen-v1-reuse-native-v1",
        "provenance": {
            "wrapper_sha256": digest(Path(__file__)),
            "pinned_sources_and_binaries": verify_pins(),
            "case_ids": list(CASE_ARGS),
        },
        "redraw_pen_static_evidence": redraw_loop_evidence(),
        "cases": cases,
    }
    if args.write_fixture:
        FIXTURE.write_text(json.dumps(fixture, indent=2, sort_keys=True) + "\n")
        print(f"wrote {FIXTURE.relative_to(ROOT)} ({len(cases)} native reuse cases)")
    else:
        if not FIXTURE.exists():
            raise SystemExit("native V1 reuse fixture missing; use --write-fixture explicitly")
        current = json.loads(FIXTURE.read_text())
        if current != fixture:
            raise SystemExit("native V1 reuse fixture mismatch")
        print(f"verified {len(cases)} native InkPen V1 reuse cases")


if __name__ == "__main__":
    main()
