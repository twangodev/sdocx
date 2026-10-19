#!/usr/bin/env python3
"""Replay Fountain saved-color defaults at the native model and Drawing boundaries.

The fixture embeds two type-1 frames extracted from hashed corpus documents.
The separate eight-row Drawing matrix uses synthetic ObjectStroke setter inputs
to verify ARGB forwarding; it is not represented as serialized-corpus evidence.
"""
import contextlib
import hashlib
import importlib.util
import io
import json
import os
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "conformance/fountain-saved-color-native.json"
MODEL_RUNNER_SHA256 = "78216df84c591d5c04e1fbda4c4a93d01921513ca7b619a5a0a79f1d97f25c2a"
DRAWING_RUNNER_SHA256 = "b0837c86483aa438923f9d4d1c6451b1ebc1c5fec4c5f7c447b928a25a8f7b19"


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def replay_model_frames(expected):
    path = ROOT / "conformance/object_stroke_channels_native.py"
    source_hash = hashlib.sha256(path.read_bytes()).hexdigest()
    if source_hash != MODEL_RUNNER_SHA256:
        raise RuntimeError(f"pinned Model runner changed: {source_hash}")
    sys.path[:0] = [str(ROOT / "conformance"), str(ROOT / "scratch/apk-analysis-runtime/python")]
    channels = load_module("fountain_saved_color_channels", path)
    actual = []
    for row in expected:
        frame = bytes.fromhex(row["frame_hex"])
        if hashlib.sha256(frame).hexdigest() != row["frame_sha256"]:
            raise RuntimeError(f"embedded frame hash mismatch: {row['id']}")
        if struct.unpack_from("<I", frame, 0)[0] != len(frame):
            raise RuntimeError(f"declared frame length mismatch: {row['id']}")
        if struct.unpack_from("<h", frame, 4)[0] != 1:
            raise RuntimeError(f"expected a type-1 stroke frame: {row['id']}")
        cursor = 10
        property_count = frame[cursor]
        cursor += 1
        properties = frame[cursor:cursor + property_count]
        cursor += property_count
        field_count = frame[cursor]
        cursor += 1
        fields = frame[cursor:cursor + field_count]
        if properties.hex() != row["property_mask_hex"] or fields.hex() != row["field_mask_hex"]:
            raise RuntimeError(f"embedded frame masks differ: {row['id']}")
        has_color = bool(int.from_bytes(fields, "little") & (1 << 2))
        if has_color != (row["parsed_style_color_argb"] is not None):
            raise RuntimeError(f"serialized color presence disagrees with parsed metadata: {row['id']}")
        channels.POINT_COUNT = struct.unpack_from("<H", frame, cursor + field_count)[0]
        capture = channels.native_capture(frame)
        color_call = next(call for call in capture["native_calls"]
                          if call["symbol"] == "SPen::ObjectStroke::GetColor() const")
        new_apply = capture["native_calls"][0]
        # Recover the mapping from the pinned Model ELF's exported GetColor
        # RVA, then independently check NewApply's entry.
        color_rva = 0x2e0ac0
        apply_rva = 0x2ee650
        color_address = int(color_call["entry_address"], 16)
        apply_address = int(new_apply["entry_address"], 16)
        model_base = color_address - color_rva
        if model_base != 0x4000000 or apply_address - model_base != apply_rva:
            raise RuntimeError(f"native Model entry mapping differs: {row['id']}")
        result = {
            "id": row["id"],
            "frame_sha256": hashlib.sha256(frame).hexdigest(),
            "frame_length": len(frame),
            "native_consumed_bytes": capture["consumed_bytes"],
            "native_state": capture["native_state"],
            "new_apply_rva": hex(apply_rva),
            "get_color_rva": hex(color_rva),
            "new_apply_entry_address": hex(apply_address),
            "get_color_entry_address": hex(color_address),
            "model_mapping_base_address": hex(model_base),
            "get_color_return_x0": color_call["return_x0"],
            "mapped_dsos": capture["mapped_dsos"],
            "helper_sha256": capture["helper_sha256"],
            "tool_sha256": capture["tool_sha256"],
        }
        for field in ("frame_sha256", "frame_length", "native_consumed_bytes", "native_state",
                      "new_apply_rva", "get_color_rva", "get_color_return_x0", "mapped_dsos",
                      "helper_sha256", "tool_sha256"):
            if result[field] != row[field]:
                raise RuntimeError(f"native Model replay differs for {row['id']} at {field}")
        actual.append(result)
    return actual


def replay_drawing_color_matrix(expected):
    path = ROOT / "conformance/fountain_saved_caller_native.py"
    original = path.read_text()
    source_hash = hashlib.sha256(original.encode()).hexdigest()
    if source_hash != DRAWING_RUNNER_SHA256:
        raise RuntimeError(f"pinned Drawing runner changed: {source_hash}")
    replacements = [
        ("MODE.add_argument('--check-fixture',action='store_true',help='replay and compare the native capture')\n",
         "MODE.add_argument('--check-fixture',action='store_true',help='replay and compare the native capture')\nMODE.add_argument('--capture-color-audit',action='store_true')\n"),
        ("MODE_ARGS=MODE_PARSER.parse_args()\n", "MODE_ARGS=MODE_PARSER.parse_args()\n"),
        ("for label,mode,size,advanced_value in [(f'{v14}_{case}', mode, size, '14;') for v14 in ('v14',) for case,mode,size in [('omitted','omit',None),('zero','set',0.),('small','set',.1),('normal','set',10.)]] + [(f'v16_{case}',mode,size,'18;0;100;') for case,mode,size in [('omitted','omit',None),('zero','set',0.),('small','set',.1),('normal','set',10.)]]:",
         "for label,mode,size,advanced_value,color in [(f'{version}_alpha{alpha}', 'set', 10.0, advanced, color) for version,advanced in [('v14','14;'),('v16','18;0;100;')] for alpha,color in [('omitted',None),('0',0x00336699),('128',0x80336699),('255',0xff336699)]]:"),
        ("p.put(obj+32,'Q',impl)\n call('SPen::ObjectStroke::SetPenName", "p.put(obj+32,'Q',impl)\n if color is not None:call('SPen::ObjectStroke::SetColor(unsigned int)',[obj,color])\n call('SPen::ObjectStroke::GetColor() const',[obj]); source_argb=f'0x{p.x(0)&0xffffffff:08x}'\n call('SPen::ObjectStroke::SetPenName"),
        ("set_size_addr=p.syms['SPen::Pen::SetSize(float)']; get_size_addr=p.syms['SPen::Pen::GetSize()']", "set_size_addr=p.syms['SPen::Pen::SetSize(float)']; get_size_addr=p.syms['SPen::Pen::GetSize()']; set_color_addr=p.syms['SPen::Pen::SetColor(int)']; set_data_addr=p.syms['SPen::PenDrawableRT::SetPenData(float, int)']; color_calls=[]"),
        ("if a==set_size_addr:pen_set_calls.append", "if a==set_color_addr:color_calls.append({'method':'Pen::SetColor','receiver':hex(p.x(0)),'argb':f'0x{p.x(1)&0xffffffff:08x}'})\n  elif a==set_data_addr:color_calls.append({'method':'PenDrawableRT::SetPenData','receiver':hex(p.x(0)),'size_f32_bits':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)).hex(),'argb':f'0x{p.x(1)&0xffffffff:08x}'})\n  elif a==set_size_addr:pen_set_calls.append"),
        ("records.append({'case':label,'advanced_setting':advanced_value,'style_pen_size_setter':mode,", "records.append({'case':label,'advanced_setting':advanced_value,'configured_source_argb':None if color is None else f'0x{color:08x}','native_objectstroke_getcolor_u32':source_argb,'native_color_setter_calls':color_calls,'style_pen_size_setter':mode,"),
    ]
    source = original
    for old, new in replacements:
        if source.count(old) != 1:
            raise RuntimeError(f"expected exactly one Drawing caller patch anchor: {old[:90]!r}")
        source = source.replace(old, new, 1)
    # Keep this diagnostic transformation narrow: replace the original four-size
    # postconditions with route and exact ARGB-forwarding assertions.
    start = source.index("for _row in records:\n")
    end = source.index("# Source and library pins were checked", start)
    source = source[:start] + '''for _row in records:
 _versions=[f'GLV{v}' for v in (14,16) if any(f'GLV{v}::RedrawPen' in call['entry'] for call in _row['redraw_entries'])]
 if len(_versions)!=1:raise RuntimeError(f'expected one selected FountainPen drawable: {_row["case"]}')
 _row['selected_drawable_class']=_versions[0]
 _expected_version='GLV14' if _row['advanced_setting']=='14;' else 'GLV16'
 if _versions[0]!=_expected_version or _row['drawing_return']!=1:raise RuntimeError(f'native Drawing selection failed: {_row["case"]}')
 _expected_argb=_row['configured_source_argb'] or '0xff000000'
 if _row['native_objectstroke_getcolor_u32']!=_expected_argb:raise RuntimeError(f'ObjectStroke GetColor mismatch: {_row["case"]}')
 _native=[c['argb'] for c in _row['native_color_setter_calls'] if c['method']=='Pen::SetColor']
 _rt=[c for c in _row['native_color_setter_calls'] if c['method']=='PenDrawableRT::SetPenData']
 if not _native or _native[-1]!=_expected_argb or _rt!=[{'method':'PenDrawableRT::SetPenData','receiver':_rt[0]['receiver'],'size_f32_bits':'00002041','argb':_expected_argb}]:raise RuntimeError(f'ARGB was not forwarded by native Drawing: {_row["case"]}')
 if _row['source_model_get_point_count']!=3:raise RuntimeError('unexpected source point count')
''' + source[end:]
    tail = source.index("if MODE_ARGS.write_fixture:\n")
    source = source[:tail] + "print(json.dumps(value,sort_keys=True))\n"
    old_argv = sys.argv
    output = io.StringIO()
    try:
        sys.argv = [str(path), "--capture-color-audit"]
        namespace = {"__name__": "__main__", "__file__": str(path)}
        with contextlib.redirect_stdout(output):
            exec(compile(source, str(path), "exec"), namespace)
    finally:
        sys.argv = old_argv
    lines = [line for line in output.getvalue().splitlines() if line.startswith("{")]
    if not lines:
        raise RuntimeError("patched Drawing runner produced no JSON")
    value = json.loads(lines[-1])
    compact = []
    for row in value["cases"]:
        expected_argb = row["configured_source_argb"] or "0xff000000"
        compact.append({
            "case": row["case"],
            "advanced_setting": row["advanced_setting"],
            "configured_source_argb": row["configured_source_argb"],
            "native_objectstroke_getcolor_u32": row["native_objectstroke_getcolor_u32"],
            "selected_drawable_class": row["selected_drawable_class"],
            "native_color_setter_calls": row["native_color_setter_calls"],
            "source_getcolor_matches_configured_or_default": row["native_objectstroke_getcolor_u32"] == expected_argb,
            "rt_setpendata_forwards_argb_and_size10": True,
        })
    if expected is not None and compact != expected:
        raise RuntimeError("native eight-case Drawing color matrix differs from fixture")
    return compact, value["provenance"]


def main():
    if len(sys.argv) == 1:
        mode = "--check-fixture"
    elif len(sys.argv) == 2 and sys.argv[1] in ("--write-fixture", "--check-fixture"):
        mode = sys.argv[1]
    else:
        raise SystemExit("usage: fountain_saved_color_native.py [--write-fixture|--check-fixture]")
    wrapper_hash = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    fixture = json.loads(FIXTURE.read_text())
    handoff_expected = None if mode == "--write-fixture" else fixture["synthetic_drawing_color_handoff"]
    drawing, drawing_provenance = replay_drawing_color_matrix(handoff_expected)
    model = replay_model_frames(fixture["real_serialized_frames"])
    if mode == "--write-fixture":
        fixture["provenance"]["wrapper_sha256"] = wrapper_hash
        fixture["model_replay"] = model
        fixture["synthetic_drawing_color_handoff"] = drawing
        fixture["provenance"]["drawing_runtime_provenance"] = drawing_provenance
        FIXTURE.write_text(json.dumps(fixture, indent=2, sort_keys=True) + "\n")
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
        return
    if fixture["provenance"]["wrapper_sha256"] != wrapper_hash:
        raise RuntimeError("color oracle wrapper source changed")
    if drawing_provenance != fixture["provenance"]["drawing_runtime_provenance"]:
        raise RuntimeError("Drawing runtime provenance changed")
    if model != fixture["model_replay"] or drawing != fixture["synthetic_drawing_color_handoff"]:
        raise RuntimeError("native replay mismatch")
    if fixture["provenance"]["drawing_runner_sha256"] != DRAWING_RUNNER_SHA256:
        raise RuntimeError("Drawing runner pin mismatch")
    if fixture["provenance"]["model_runner_sha256"] != MODEL_RUNNER_SHA256:
        raise RuntimeError("Model runner pin mismatch")
    print(f"verified {len(model)} corpus frame replays and {len(drawing)} synthetic Drawing color cases")


if __name__ == "__main__":
    main()
