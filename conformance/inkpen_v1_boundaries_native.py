#!/usr/bin/env python3
"""Native InkPen V1 source-event and terminal RT vector boundary oracle.

The saved ObjectStroke inputs and 1440x2560/1920x1080 system profiles are
synthetic native Model setter controls, not decoded Samsung Notes documents.
This records native GLV1 source-event return boundaries and full RTV1 triples.
Writing requires --write-fixture; the default replays and checks the fixture.
"""
import hashlib
import importlib.util
import json
import os
import struct
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "conformance"
FIXTURE = OUT / "inkpen-v1-boundaries-native.json"
WRAPPER = ROOT / "conformance/inkpen_v1_reuse_native.py"
BASE_CASES = ROOT / "conformance/inkpen-v1-saved-native.json"
PROFILE = {"width": 1440, "height": 2560}
PINNED_HELPERS = {
    "conformance/inkpen_v1_reuse_native.py": "8306643a8d83aa9ed7ce06984ccc2540cc4ce426bb73b3bd596e9b637c042006",
    "conformance/inkpen-v1-saved-native.json": "ade2af7ee7c7c07d4663151074932009f7def1a91ba4c14a4c55c294a3e32fb4",
}

def f32(bits):
    return struct.unpack("<f", bytes.fromhex(bits))[0]

def case_input(row):
    i = row["input"]
    return {
        "points": [[f32(x), f32(y)] for x, y in i["points_f32_bits"]],
        "size": f32(i["pen_size_f32_bits"]),
        "curve_enabled": i["curve_enabled"],
        "tool_type": i["tool_type"],
        "stroke_type": i["stroke_type"],
        "pressure": [f32(x) for x in i["pressure_f32_bits"]],
        "timestamps": list(i["timestamps_i32"]),
        "tilt": [f32(x) for x in i["tilt_f32_bits"]],
        "orientation": [f32(x) for x in i["orientation_f32_bits"]],
    }

def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod

def validate_source_arrays(row, input_row, label):
    expected = {
        "points": b"".join(struct.pack("<ff", *point) for point in input_row["points"]),
        "pressures": b"".join(struct.pack("<f", value) for value in input_row["pressure"]),
        "timestamps": b"".join(struct.pack("<i", value) for value in input_row["timestamps"]),
        "tilts": b"".join(struct.pack("<f", value) for value in input_row["tilt"]),
        "orientations": b"".join(struct.pack("<f", value) for value in input_row["orientation"]),
    }
    arrays = row["reuse"]["target_source_ObjectStroke_arrays"]
    widths = {"points": 8, "pressures": 4, "timestamps": 4, "tilts": 4, "orientations": 4}
    for channel, expected_bytes in expected.items():
        actual = arrays[channel]
        if actual["element_size"] != widths[channel]:
            raise RuntimeError(f"{label}: native ObjectStroke {channel} element size differs")
        if actual["count"] != len(expected_bytes) // widths[channel]:
            raise RuntimeError(f"{label}: native ObjectStroke {channel} count differs")
        if bytes.fromhex(actual["data_hex"]) != expected_bytes:
            raise RuntimeError(f"{label}: native ObjectStroke {channel} bytes differ from declared input")

def capture(mod, input_row, label, profile):
    src = mod.patched_caller_source()
    old = next(line for line in src.splitlines() if line.startswith("input_case={"))
    new = "input_case=" + repr(input_row)
    if src.count(old) != 1:
        raise RuntimeError("expected a unique pinned caller input_case anchor")
    src = src.replace(old, new, 1)
    launcher = (
        "import pathlib,sys\n"
        "filename=sys.argv[1]; argv=sys.argv[2:]; sys.argv=[filename,*argv]\n"
        "namespace={'__file__':filename,'__name__':'__main__'}\n"
        "exec(compile(sys.stdin.read(),filename,'exec'),namespace)\n"
    )
    cmd = [sys.executable, "-c", launcher, str(mod.CALLER), "--print-capture",
           "--advanced-setting", "null", "--width", str(profile["width"]),
           "--height", str(profile["height"])]
    proc = subprocess.run(cmd, input=src, cwd=ROOT, env=os.environ.copy(), text=True, capture_output=True)
    if proc.returncode:
        raise RuntimeError(f"{label} native caller failed ({proc.returncode}):\n{proc.stderr}\n{proc.stdout[-2000:]}")
    row = json.loads(proc.stdout)
    if row["route"]["selected_drawable_class"] != "SPen::InkPenStrokeDrawableGLV1":
        raise RuntimeError(f"{label} did not select V1: {row['route']}")
    validate_source_arrays(row, input_row, label)
    if row["input"]["points_f32_bits"] != [[struct.pack("<f", v).hex() for v in p] for p in input_row["points"]]:
        raise RuntimeError(f"{label} native source arrays changed")
    if row["input"]["pressure_f32_bits"] != [struct.pack("<f", v).hex() for v in input_row["pressure"]]:
        raise RuntimeError(f"{label} native pressure array changed")
    if row["input"]["timestamps_i32"] != input_row["timestamps"]:
        raise RuntimeError(f"{label} native timestamp array changed")
    if row["input"]["tilt_f32_bits"] != [struct.pack("<f", v).hex() for v in input_row["tilt"]]:
        raise RuntimeError(f"{label} native tilt array changed")
    if row["input"]["orientation_f32_bits"] != [struct.pack("<f", v).hex() for v in input_row["orientation"]]:
        raise RuntimeError(f"{label} native orientation array changed")
    if row["input"]["pen_size_f32_bits"] != struct.pack("<f", input_row["size"]).hex():
        raise RuntimeError(f"{label} native ObjectStroke GetPenSize differs from declared input")
    if (row["input"]["tool_type"], row["input"]["stroke_type"], row["input"]["curve_enabled"]) != (input_row["tool_type"], input_row["stroke_type"], input_row["curve_enabled"]):
        raise RuntimeError(f"{label} native style getters changed")
    if row["streams"]["draw_object_stroke"]["sha256"] != row["streams"]["saved_redraw"]["sha256"]:
        raise RuntimeError(f"{label} Drawing and saved-redraw final streams differ")
    row["event_boundary_probe"] = {
        "in_memory_patched_caller_sha256": hashlib.sha256(src.encode()).hexdigest(),
        "history_index_basis": "ordered native event calls checked against Model getter source XY; no native source-index argument is claimed",
    }
    return row

def add_no_curve_trace(source):
    anchors = [
        ("('::Draw(','::RedrawPen(','::reDrawLine(','::drawLine(','::drawPoint(','::startPen(','::endPen(')))",
         "('::Draw(','::RedrawPen(','::reDrawLine(','::reDrawPen_NoCurve(','::endPen_NoCurve(','::drawLine(','::drawPoint(','::startPen(','::endPen(')))"),
        ("if 'SPen::InkPenStrokeDrawableGLV1::reDrawLine(' in signature or 'SPen::InkPenStrokeDrawableGLV1::endPen(' in signature:",
         "if any(tag in signature for tag in ('SPen::InkPenStrokeDrawableGLV1::reDrawLine(','SPen::InkPenStrokeDrawableGLV1::reDrawPen_NoCurve(','SPen::InkPenStrokeDrawableGLV1::endPen_NoCurve(','SPen::InkPenStrokeDrawableGLV1::endPen(')):") ,
        ("line_event='reDrawLine' if '::reDrawLine(' in signature else 'endPen'; sample_index=redraw_sample_indices.get((phase,line_event),0); redraw_sample_indices[(phase,line_event)]=sample_index+1",
         "line_event=('reDrawLine' if '::reDrawLine(' in signature else ('reDrawPen_NoCurve' if '::reDrawPen_NoCurve(' in signature else ('endPen_NoCurve' if '::endPen_NoCurve(' in signature else 'endPen'))); sample_index=redraw_sample_indices.get((phase,line_event),0); redraw_sample_indices[(phase,line_event)]=sample_index+1"),
        ("'source_history_index':sample_index+1 if line_event=='reDrawLine' else None",
         "'source_history_index':sample_index+1 if line_event=='reDrawLine' else None"),
    ]
    for old, new in anchors:
        n = source.count(old)
        if n != 1:
            raise RuntimeError(f"no-curve trace patch anchor expected one occurrence, found {n}: {old[:70]}")
        source = source.replace(old, new, 1)
    return source

def add_no_curve_line_returns(source):
    registration = "('::Draw(','::RedrawPen(','::reDrawLine(','::reDrawPen_NoCurve(','::endPen_NoCurve(','::drawLine(','::drawPoint(','::startPen(','::endPen(')))"
    if source.count(registration) != 1:
        raise RuntimeError("drawLine_NoCurve registration anchor expected once")
    source = source.replace(
        registration,
        "('::Draw(','::RedrawPen(','::reDrawLine(','::reDrawPen_NoCurve(','::endPen_NoCurve(','::drawLine_NoCurve(','::drawLine(','::drawPoint(','::startPen(','::endPen(')))",
        1,
    )
    if "'::drawLine_NoCurve('" not in source:
        raise RuntimeError("failed to register V1 drawLine_NoCurve symbol")
    source = source.replace(
        "native_redraw_sample_ranges=[]\npending_redraw_sample_returns={}\nredraw_sample_indices={}\n",
        "native_redraw_sample_ranges=[]\npending_redraw_sample_returns={}\nredraw_sample_indices={}\n"
        "native_no_curve_line_ranges=[]\npending_no_curve_line_returns={}\nno_curve_line_indices={}\n",
        1,
    )
    entry_anchor = "  record={'signatures':method_trace_names_by_pc[a],'phase':phase,'pc':hex(a),'regs':regs,'pointer_heads':pointers}\n"
    if source.count(entry_anchor) != 1:
        raise RuntimeError("drawLine_NoCurve entry instrumentation anchor changed")
    entry_code = entry_anchor + (
        "  if any('SPen::InkPenStrokeDrawableGLV1::drawLine_NoCurve(' in sig for sig in method_trace_names_by_pc[a]):\n"
        "   this=u.reg_read(R.UC_ARM64_REG_X0); rt=struct.unpack('<Q',u.mem_read(this+16,8))[0]; vector_obj=struct.unpack('<Q',u.mem_read(rt+80,8))[0]; begin,end,capacity=struct.unpack('<3Q',u.mem_read(vector_obj,24)); before=end-begin\n"
        "   if not (begin<=end<=capacity) or before%12: raise RuntimeError(f'invalid V1 no-curve segment vector {begin:#x}/{end:#x}/{capacity:#x}')\n"
        "   idx=no_curve_line_indices.get(phase,0); no_curve_line_indices[phase]=idx+1\n"
        "   ev={'method':'drawLine_NoCurve','phase':phase,'segment_call_index':idx,'entry_pc':hex(a),'receiver_address':hex(this),'rt':hex(rt),'vector_obj':vector_obj,'before_count_bytes':before,'before_record_count':before//12,'args_s0_s2_bits':[hex(u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))) for i in range(3)],'arg_x1':hex(u.reg_read(R.UC_ARM64_REG_X1)),'arg_x2':hex(u.reg_read(R.UC_ARM64_REG_X2)),'arg_x3':hex(u.reg_read(R.UC_ARM64_REG_X3)),'return_pc':hex(u.reg_read(R.UC_ARM64_REG_LR))}\n"
        "   pending_no_curve_line_returns[u.reg_read(R.UC_ARM64_REG_LR)]=ev; native_no_curve_line_ranges.append(ev)\n"
    )
    source = source.replace(entry_anchor, entry_code, 1)
    return_anchor = " if a in pending_redraw_sample_returns:\n"
    if source.count(return_anchor) != 1:
        raise RuntimeError("drawLine_NoCurve return instrumentation anchor changed")
    return_code = (
        " if a in pending_no_curve_line_returns:\n"
        "  ev=pending_no_curve_line_returns.pop(a); vector_obj=ev['vector_obj']; begin,end,capacity=struct.unpack('<3Q',u.mem_read(vector_obj,24)); after=end-begin\n"
        "  if not (begin<=end<=capacity) or after%12 or after<ev['before_count_bytes']: raise RuntimeError(f'invalid V1 no-curve segment return {begin:#x}/{end:#x}/{capacity:#x}')\n"
        "  ev.update({'return_pc':hex(a),'after_count_bytes':after,'after_record_count':after//12,'added_record_count':(after-ev['before_count_bytes'])//12,'after_stream_bytes':bytes(u.mem_read(begin,after)).hex() if after else ''})\n"
        + return_anchor
    )
    source = source.replace(return_anchor, return_code, 1)
    emit_anchor = "'native_redraw_sample_ranges':native_redraw_sample_ranges,"
    if source.count(emit_anchor) != 1:
        raise RuntimeError("drawLine_NoCurve output instrumentation anchor changed")
    source = source.replace(emit_anchor, emit_anchor + "'native_no_curve_line_ranges':native_no_curve_line_ranges,", 1)
    return source

def validate_event_ranges(row, inp, label):
    points = row["input"]["points_f32_bits"]
    pressure = row["input"]["pressure_f32_bits"]
    tool = row["input"]["tool_type"]
    stream = bytes.fromhex(row["reuse"]["target_stream_bytes"])
    summaries = []
    for phase in ("draw_object_stroke", "saved_redraw"):
        events = [e for e in row["reuse"]["native_redraw_sample_ranges"] if e["phase"] == phase]
        redraws = [e for e in events if e["method"] == "reDrawLine"]
        ends = [e for e in events if e["method"] == "endPen"]
        if row["input"]["curve_enabled"]:
            if len(redraws) != len(points) - 1 or len(ends) != 1:
                raise RuntimeError(f"{label}/{phase}: expected N-1 reDrawLine and one endPen, got {len(redraws)}/{len(ends)}")
            prev = 0
            boundaries = []
            for index, ev in enumerate(redraws, 1):
                if ev["source_history_index"] != index or ev["before_record_count"] != prev:
                    raise RuntimeError(f"{label}/{phase}: discontinuous source-event boundary at point {index}")
                if ev["entry_pc"] != "0x5026fa8" or ev["return_pc"] != "0x5027544":
                    raise RuntimeError(f"{label}/{phase}: unexpected reDrawLine trace PCs")
                args = ev["args_s0_s2_bits"]
                if int(args[0], 16).to_bytes(4, "little").hex() != points[index][0] or int(args[1], 16).to_bytes(4, "little").hex() != points[index][1]:
                    raise RuntimeError(f"{label}/{phase}: source XY mismatch at point {index}")
                expected_p = 0.5 if tool in (1, 3) else min(1.0, f32(pressure[index]))
                if args[2] != hex(struct.unpack("<I", struct.pack("<f", expected_p))[0]):
                    raise RuntimeError(f"{label}/{phase}: V1 effective pressure mismatch at point {index}: {args[2]}")
                end = ev["after_record_count"]
                if ev["after_count_bytes"] != end * 12 or ev["after_stream_bytes"] != stream[:end * 12].hex():
                    raise RuntimeError(f"{label}/{phase}: native event prefix differs from final RT stream")
                boundaries.append(end)
                prev = end
            term = ends[0]
            if term["before_record_count"] != prev or term["after_stream_bytes"] != stream.hex():
                raise RuntimeError(f"{label}/{phase}: terminal event does not cover exact suffix/final stream")
            if term["entry_pc"] != "0x502599c" or term["return_pc"] != "0x5027618":
                raise RuntimeError(f"{label}/{phase}: unexpected endPen trace PCs")
            summary = {"phase": phase, "sample_ends": boundaries,
                       "terminal": [term["before_record_count"], term["after_record_count"]],
                       "sample_added": [e["added_record_count"] for e in redraws],
                       "boundary_granularity": "one native reDrawLine return per source index 1..N-1"}
        else:
            batches = [e for e in events if e["method"] == "reDrawPen_NoCurve"]
            terminals = [e for e in events if e["method"] == "endPen_NoCurve"]
            if len(batches) != 1 or len(terminals) != 1:
                raise RuntimeError(f"{label}/{phase}: expected one no-curve history batch + terminal, got {len(batches)}/{len(terminals)}")
            batch, term = batches[0], terminals[0]
            count = len(stream) // 12
            if batch["entry_pc"] != "0x50279b8" or batch["return_pc"] != "0x5027760":
                raise RuntimeError(f"{label}/{phase}: unexpected reDrawPen_NoCurve trace PCs")
            if batch["before_record_count"] != 0 or batch["after_record_count"] != count or batch["after_stream_bytes"] != stream.hex():
                raise RuntimeError(f"{label}/{phase}: no-curve history batch does not match final RT stream")
            if term["entry_pc"] != "0x5025db8" or term["return_pc"] != "0x5027b88":
                raise RuntimeError(f"{label}/{phase}: unexpected endPen_NoCurve trace PCs")
            if term["before_record_count"] != count or term["after_record_count"] != count:
                raise RuntimeError(f"{label}/{phase}: no-curve terminal unexpectedly changes RT stream")
            segments = [e for e in row["reuse"].get("native_no_curve_line_ranges", []) if e["phase"] == phase]
            if len(segments) != len(points) - 1:
                raise RuntimeError(f"{label}/{phase}: no-curve drawLine_NoCurve calls do not cover N-1 source indices")
            segment_ends = [0]
            segment_before = 0
            for index, ev in enumerate(segments, 1):
                args = ev["args_s0_s2_bits"]
                if ev["segment_call_index"] != index - 1 or ev["before_record_count"] != segment_before:
                    raise RuntimeError(f"{label}/{phase}: no-curve segment return ranges are discontinuous")
                if ev["entry_pc"] != "0x5028328" or ev["return_pc"] != "0x5027ad8":
                    raise RuntimeError(f"{label}/{phase}: unexpected drawLine_NoCurve entry/return PCs")
                if int(args[0], 16).to_bytes(4, "little").hex() != points[index][0] or int(args[1], 16).to_bytes(4, "little").hex() != points[index][1]:
                    raise RuntimeError(f"{label}/{phase}: no-curve endpoint registers differ at source index {index}")
                if tool == 1 and args[2] != "0x3f000000":
                    raise RuntimeError(f"{label}/{phase}: tool 1 no-curve effective pressure differs")
                end = ev["after_record_count"]
                if ev["after_stream_bytes"] != stream[:end * 12].hex():
                    raise RuntimeError(f"{label}/{phase}: no-curve segment native prefix differs from final stream")
                segment_ends.append(end)
                segment_before = end
            if segment_before != count:
                raise RuntimeError(f"{label}/{phase}: no-curve segment calls do not account for full history buffer")
            summary = {"phase": phase, "sample_ends": segment_ends,
                       "terminal": [term["before_record_count"], term["after_record_count"]],
                       "sample_added": [e["added_record_count"] for e in segments],
                       "history_batch": [batch["before_record_count"], batch["after_record_count"]],
                       "segment_return_method": "drawLine_NoCurve",
                       "boundary_granularity": "per-source history segment drawLine_NoCurve return; source index follows ordered calls and is checked against current XY registers"}
        summary.update({"event_prefixes_match_final_stream": True, "native_final_count": len(stream) // 12})
        summaries.append(summary)
    return summaries

def main():
    for relative, expected in PINNED_HELPERS.items():
        actual = hashlib.sha256((ROOT / relative).read_bytes()).hexdigest()
        if actual != expected:
            raise RuntimeError(f"preflight pin mismatch for {relative}: {actual}")
    source = json.loads(BASE_CASES.read_text())
    source_cases = source["cases"]
    wrapper = load("inkpen_v1_reuse_probe", WRAPPER)
    pins = wrapper.verify_pins()
    caller = load("inkpen_reuse_probe", wrapper.PINNED_RUNNER)
    patched_source = add_no_curve_line_returns(add_no_curve_trace(wrapper.instrument_v1_trace(caller)))
    caller.patched_caller_source = lambda: patched_source
    # Keep the historical source matrix intact and add isolated tool controls.
    cases = {k: case_input(v) for k, v in source_cases.items() if k != "single_rejected_1920x1080"}
    template = cases["five_1920x1080"]
    for tool in (0, 3, 4):
        custom = json.loads(json.dumps(template))
        custom["tool_type"] = tool
        cases[f"tool{tool}_five_1920x1080"] = custom
    results = {}
    for label, inp in cases.items():
        # Use each source row's profile where known; supplemental controls use 1920x1080.
        profile = source_cases[label].get("screen_resolution", [PROFILE])[0] if label in source_cases else {"width":1920,"height":1080}
        row = capture(caller, inp, label, profile)
        # Attach the verified event-return boundaries without rewriting captured outputs.
        row["event_boundary_summary"] = validate_event_ranges(row, inp, label)
        results[label] = row
        print(f"captured {label}: {row['streams']['draw_object_stroke']['callback_count']} records", flush=True)
    fixture = {
        "schema": "sdocx-inkpen-v1-native-event-boundaries-v1",
        "scope": "Synthetic native Model ObjectStroke setter inputs through Drawing null-setting V1 and direct saved redraw. Captures actual native curved reDrawLine and endPen returns, and curve-false drawLine_NoCurve segment returns with full RTV1 triples. Does not claim serialized-note admission or device/export parity.",
        "provenance": {"runner_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                       "base_numeric_fixture_sha256": hashlib.sha256(BASE_CASES.read_bytes()).hexdigest(),
                       "pinned_sources_and_dsos": pins,
                       "trace_instrumentation_wrapper_sha256": hashlib.sha256(WRAPPER.read_bytes()).hexdigest(),
                       "base_fixture_case_ids": [k for k in source_cases if k != "single_rejected_1920x1080"],
                       "tool_controls": {"tool0_five_1920x1080": {"derived_from": "five_1920x1080", "tool_type": 0},
                                         "tool3_five_1920x1080": {"derived_from": "five_1920x1080", "tool_type": 3},
                                         "tool4_five_1920x1080": {"derived_from": "five_1920x1080", "tool_type": 4}}},
        "event_contract": {"redraw_entry_return": ["0x5026fa8","0x5027544"],
                            "terminal_entry_return": ["0x502599c","0x5027618"],
                            "no_curve_segment_entry_return": ["0x5028328","0x5027ad8"],
                            "no_curve_batch_entry_return": ["0x50279b8","0x5027760"],
                            "no_curve_terminal_entry_return": ["0x5025db8","0x5027b88"],
                            "history_index": "ordered reDrawLine call maps source index 1..N-1, corroborated with source XY registers; no native history-index argument is claimed",
                            "pressure": "curved reDrawLine S2 is 0.5 for tools 1/3 and source pressure clamped at 1 for 0/2/4; no-curve tool1 S2 is 0.5 in the captured line case",
                            "sample_ends": "exclusive count after each native history segment return; terminal is the unassigned endPen/endPen_NoCurve suffix"},
        "cases": results,
    }
    encoded = json.dumps(fixture, indent=2, sort_keys=True) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)} ({len(results)} native V1 boundary cases)")
    else:
        if not FIXTURE.exists():
            raise SystemExit("native InkPen V1 event-boundary fixture missing; use --write-fixture explicitly")
        current = json.loads(FIXTURE.read_text())
        if current != fixture:
            raise SystemExit("native InkPen V1 event-boundary fixture mismatch")
        print(f"verified {len(results)} native InkPen V1 event-boundary cases")

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--write-fixture", action="store_true", help="explicitly write or refresh the captured fixture")
    group.add_argument("--check-fixture", action="store_true", help="explicitly replay and compare with the fixture")
    args = parser.parse_args()
    main()
