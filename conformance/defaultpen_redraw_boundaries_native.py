#!/usr/bin/env python3
"""Pinned native DefaultPen return-boundary witness for a six-point saved stroke.

The wrapper applies small in-memory input/instrumentation patches to the already
pinned DefaultPen Drawing caller. It never edits or imports an alternate native
implementation, and it does not overwrite the caller's four-point fixture.
"""
from __future__ import annotations

import argparse
import contextlib
import hashlib
import io
import json
import pathlib
import re
import struct
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
CALLER = ROOT / "conformance/defaultpen_saved_caller_native.py"
FIXTURE = ROOT / "conformance/defaultpen-redraw-boundaries-native.json"
PIN_TOKEN = "__SELF_PIN__"
EXPECTED_CALLER_SHA256 = "14e1b1d85df0aaebdc889871656217b927bedfc45ad1eba309fbe562eedd559a"
# Set to the normalized source digest (SELF_SHA256 replaced by PIN_TOKEN) after
# editing this runner. This avoids a self-hash cycle while pinning all other bytes.
SELF_SHA256 = "884981df10cce74b3fc5b3c9958a8fea0fa7332f9bd677794fd56a2effa940c1"

POINTS = [(0., 0.), (10., 0.), (20., 0.), (30., 10.), (40., 0.), (60., 20.)]
CASE_ID = "six_point_tool1_nonstationary"


def normalized_self_sha256() -> str:
    source = pathlib.Path(__file__).read_text()
    source = source.replace(f'SELF_SHA256 = "{SELF_SHA256}"', f'SELF_SHA256 = "{PIN_TOKEN}"')
    return hashlib.sha256(source.encode()).hexdigest()


def bits(value: float) -> str:
    return struct.pack("<f", value).hex()



def elf_vaddr_bytes(path: pathlib.Path, address: int, size: int = 4) -> bytes:
    """Resolve a virtual address through an ELF64 PT_LOAD segment."""
    data = path.read_bytes()
    if data[:4] != b"\x7fELF" or data[4] != 2 or data[5] != 1:
        raise RuntimeError(f"expected little-endian ELF64: {path}")
    e_phoff = struct.unpack_from("<Q", data, 32)[0]
    e_phentsize, e_phnum = struct.unpack_from("<HH", data, 54)
    for index in range(e_phnum):
        offset = e_phoff + index * e_phentsize
        p_type, = struct.unpack_from("<I", data, offset)
        if p_type != 1:
            continue
        p_offset, p_vaddr = struct.unpack_from("<QQ", data, offset + 8)
        p_filesz, = struct.unpack_from("<Q", data, offset + 32)
        if p_vaddr <= address and address + size <= p_vaddr + p_filesz:
            file_offset = p_offset + address - p_vaddr
            return data[file_offset:file_offset + size]
    raise RuntimeError(f"ELF virtual address {address:#x} not file-backed in {path}")


def patch_caller(source: str) -> str:
    replacements = [
        ("case_id='straight_curve_tool1_default'", f"case_id='{CASE_ID}'", 2),
        ("input_case={'points':[[0.,0.],[10.,0.],[20.,0.],[30.,10.]],'size':10.0,'curve_enabled':True,'tool_type':1,'stroke_type':0,'pressure':[.25,.5,.75,1.0],'timestamps':[10,20,30,40],'tilt':[0.]*4,'orientation':[0.]*4}",
         "input_case={'points':[[0.,0.],[10.,0.],[20.,0.],[30.,10.],[40.,0.],[60.,20.]],'size':10.0,'curve_enabled':True,'tool_type':1,'stroke_type':0,'pressure':[.25,.35,.5,.65,.8,1.0],'timestamps':[10,20,30,40,50,60],'tilt':[0.]*6,'orientation':[0.]*6}", 1),
        ("'::RedrawPen(','::drawLine(','::drawPoint(','::startPen(','::endPen('",
         "'::RedrawPen(','::redrawLine(','::drawLine(','::drawPoint(','::startPen(','::endPen('", 1),
        ("defaultpen_buffer_events=[]", "defaultpen_buffer_events=[]\nredraw_boundaries=[]\npending_redraw_returns={}", 1),
        ("def log_vcall(u,a,z,d):\n if a in pending_motion_getters",
         "def log_vcall(u,a,z,d):\n if a in pending_redraw_returns:\n  boundary=pending_redraw_returns.pop(a)\n  boundary['callback_end']=len(defaultpen_buffer_events)\n  boundary['callbacks_emitted']=boundary['callback_end']-boundary['callback_start']\n  boundary['return_pc']=hex(a)\n  redraw_boundaries.append(boundary)\n if a in pending_motion_getters", 1),
        ("  native_draw_entries.append(record)\n  for signature in method_trace_names_by_pc[a]:",
         "  native_draw_entries.append(record)\n  for signature in method_trace_names_by_pc[a]:\n   if '::redrawLine(' in signature or '::redrawLineNoCurve(' in signature or '::endPen(' in signature:\n    lr=u.reg_read(R.UC_ARM64_REG_LR)\n    pending_redraw_returns[lr]={'phase':phase,'method':signature.split('::',1)[-1].split('(')[0], 'entry_pc':hex(a), 'return_pc_expected':hex(lr), 'callback_start':len(defaultpen_buffer_events), 's0_s3_bits':[hex(u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))) for i in range(4)]}", 1),
        ("if len(draw_stream)!=132 or draw_stream!=saved_stream: raise RuntimeError(f'expected identical 132-byte stream, got {len(draw_stream)} and {len(saved_stream)} bytes')",
         "if draw_stream!=saved_stream: raise RuntimeError(f'draw and saved streams differ: {len(draw_stream)} and {len(saved_stream)} bytes')", 1),
        ("if len(events)!=11 or any(int(row['rt'],16)!=selected_rt for row in events):\n   raise RuntimeError(f'expected eleven callbacks on the Drawing-selected RT in {phase_name}')",
         "if not events or any(int(row['rt'],16)!=selected_rt for row in events):\n   raise RuntimeError(f'expected callbacks on the Drawing-selected RT in {phase_name}')", 1),
        ("result['motion_event_source_returns']=motion_sources",
         "result['motion_event_source_returns']=motion_sources\n result['native_redraw_callback_boundaries']=[row for row in redraw_boundaries if row['phase'] in ('draw_object_stroke','saved_redraw')]", 1),
    ]
    for old, new, expected_count in replacements:
        count = source.count(old)
        if count != expected_count:
            raise RuntimeError(f"caller patch anchor count changed: expected {expected_count}, got {count}: {old[:90]!r}")
        source = source.replace(old, new)
    # The original caller filters its trace table; include redrawLine entries.
    trace_filter = "('::RedrawPen(','::endPen(','::drawPoint(')"
    if source.count(trace_filter) != 1:
        raise RuntimeError("caller trace-filter anchor count changed")
    source = source.replace(trace_filter, "('::RedrawPen(','::redrawLine(','::endPen(','::drawPoint(')")
    return source


def capture() -> dict:
    caller_bytes = CALLER.read_bytes()
    caller_sha = hashlib.sha256(caller_bytes).hexdigest()
    if caller_sha != EXPECTED_CALLER_SHA256:
        raise RuntimeError(f"pinned caller changed: {caller_sha}")
    source = patch_caller(caller_bytes.decode())
    saved_argv = sys.argv
    sys.argv = [str(CALLER), "--print-capture"]
    output = io.StringIO()
    namespace = {"__name__": "__main__", "__file__": str(CALLER), "__package__": None}
    try:
        with contextlib.redirect_stdout(output):
            exec(compile(source, str(CALLER), "exec"), namespace)
    finally:
        sys.argv = saved_argv
    result = json.loads(output.getvalue())
    input_points = [[bits(x), bits(y)] for x, y in POINTS]
    rows = result["native_redraw_callback_boundaries"]
    for phase in ("draw_object_stroke", "saved_redraw"):
        phase_rows = [row for row in rows if row["phase"] == phase]
        redraw_rows = [row for row in phase_rows if row["method"].endswith("redrawLine")]
        if len(redraw_rows) != len(POINTS) - 2:
            raise RuntimeError(f"{phase}: expected N-2 redrawLine returns, got {len(redraw_rows)}")
        for index, row in enumerate(redraw_rows, start=1):
            actual_xy = [f"{int(bits, 16):08x}" for bits in row["s0_s3_bits"][:2]]
            expected_xy = [f"{int.from_bytes(bytes.fromhex(value), 'little'):08x}" for value in input_points[index]]
            if actual_xy != expected_xy:
                raise RuntimeError(f"{phase} redraw {index} args {actual_xy} != source sample {expected_xy}")
            row["source_sample_index_zero_based"] = index
        end_rows = [row for row in phase_rows if row["method"].endswith("endPen")]
        if len(end_rows) != 1:
            raise RuntimeError(f"{phase}: expected one endPen return")
        end_rows[0]["source_sample_index_zero_based"] = len(POINTS) - 1
    draw = result["streams"]["draw_object_stroke"]
    saved = result["streams"]["saved_redraw"]
    if draw["values_f32_bits"] is None or draw["callback_count"] != 22:
        raise RuntimeError("unexpected six-point callback vector")
    if not saved["same_bytes_as_drawing_phase"] or saved["sha256"] != draw["sha256"]:
        raise RuntimeError("Drawing and saved-redraw callback vectors differ")
    result["phase_final_vectors"] = {
        phase: {
            "callback_count": result["streams"][phase]["callback_count"],
            "sha256": result["streams"][phase]["sha256"],
            "triples_f32_bits": [draw["values_f32_bits"][i:i + 3] for i in range(0, len(draw["values_f32_bits"]), 3)],
        }
        for phase in ("draw_object_stroke", "saved_redraw")
    }
    result["source_zero_disassembly_contract"] = {
        "defaultpen_objectstroke_redraw_rva": "0x18b54",
        "defaultpen_event_ctor_call_rva": "0x18c00",
        "base_array_motion_event_ctor_rva": "0xbfd84",
        "base_array_ctor_calls_default_ctor_rva": "0xbfdd4",
        "base_default_ctor_zeroes_inner_source_offset_8_rva": "0xbf46c",
        "base_get_source_loads_inner_offset_8_rva": "0xc0aa0",
        "source_field_offset_from_motion_event_data": 8,
        "instructions": [
            {
                "library": library,
                "rva": hex(address),
                "bytes_little_endian": (raw := elf_vaddr_bytes(ROOT / "scratch/apk-analysis-native/arm64-v8a" / library, address)).hex(),
                "instruction_word_hex": f"{int.from_bytes(raw, 'little'):08x}",
                "operation": operation,
            }
            for library, address, operation in [
                ("libSPenDefaultPen.so", 0x18c00, "BL MotionEvent(int,int,PointF const*,...) from ObjectStroke RedrawPen"),
                ("libSPenBase.so", 0xbfdd4, "BL MotionEvent() from MotionEvent array constructor"),
                ("libSPenBase.so", 0xbf46c, "STR WZR,[X0,#8], initializes MotionEvent source to zero"),
                ("libSPenBase.so", 0xc0aa0, "LDR W0,[X8,#8], MotionEvent::GetSource reads that field"),
            ]
        ],
    }
    helper_names = ["defaultpen_saved_caller_native.py", "oblique_canvas_native.py", "fountain_native.py", "elf_plt.py"]
    result["source_hashes"] = {
        "runner_sha256_normalized_self_pin": normalized_self_sha256(),
        "caller_sha256": caller_sha,
        **{name: hashlib.sha256((ROOT / "conformance" / name).read_bytes()).hexdigest() for name in helper_names},
    }
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--write-fixture", action="store_true")
    modes.add_argument("--check-fixture", action="store_true")
    args = parser.parse_args()
    calculated = normalized_self_sha256()
    if calculated != SELF_SHA256:
        raise SystemExit(f"runner self-pin mismatch: expected {SELF_SHA256}, computed {calculated}")
    result = capture()
    fixture = {"schema": "sdocx-defaultpen-redraw-boundaries-native-v1", "cases": {CASE_ID: result}}
    encoded = json.dumps(fixture, indent=2, sort_keys=True) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)} sha256={hashlib.sha256(encoded.encode()).hexdigest()}")
    elif not FIXTURE.exists() or json.loads(FIXTURE.read_text()) != fixture:
        raise SystemExit("native redraw-boundary fixture mismatch; inspect changes, then use --write-fixture")
    else:
        print(f"verified {FIXTURE.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
