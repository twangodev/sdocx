#!/usr/bin/env python3
"""Run one frozen NoCurve edge input through the pinned native paint oracle."""
from __future__ import annotations

import argparse
import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import struct
import sys


def discover_root() -> Path:
    for parent in Path(__file__).resolve().parents:
        if parent.name == "conformance":
            return parent.parent
        if (parent / "conformance").is_dir() and (parent / "AGENTS.md").is_file():
            return parent
    raise RuntimeError("could not locate repository root")


ROOT = discover_root()
INPUT_FIXTURE = ROOT / "conformance/magicpen-nocurve-edge-paint-native.json"
ORDINARY_CAPTURE = ROOT / "conformance/magicpen_nocurve_paint_capture.py"
ORDINARY_CAPTURE_SHA256 = "76c85701436f716cefae525c5a50ee68e062616509a57239aeea1e8e63135601"
EDGE_INPUT_SHA256 = "8711c5d434ade733cb9044b68927a20707dcd251185e57a63fd5c03d5b433ed9"
EMBEDDED_INPUTS_SHA256 = "57158b37bb39eca418f0bf67b227b24a8f25170a5242158d90ace5cb5ad9e7b7"
if hashlib.sha256(ORDINARY_CAPTURE.read_bytes()).hexdigest() != ORDINARY_CAPTURE_SHA256:
    raise SystemExit("pinned ordinary MagicPen capture helper changed")
fixture = json.loads(INPUT_FIXTURE.read_text())
if fixture.get("schema") != "magicpen-nocurve-edge-native-paint-v1" or fixture.get("inputs_sha256") != EDGE_INPUT_SHA256:
    raise SystemExit("pinned edge input fixture changed")
canonical_inputs = json.dumps(fixture.get("inputs"), sort_keys=True, separators=(",", ":")).encode()
if hashlib.sha256(canonical_inputs).hexdigest() != EMBEDDED_INPUTS_SHA256 or fixture.get("embedded_inputs_sha256") != EMBEDDED_INPUTS_SHA256:
    raise SystemExit("embedded edge inputs differ from the reviewed canonical digest")

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--_case-id", required=True)
parser.add_argument("--_scale", type=int, choices=(1, 2, 4), required=True)
parser.add_argument("--_capture-background", choices=("transparent", "opaque"), required=True)
args = parser.parse_args()
case = next((item for item in fixture["inputs"]["cases"] if item["id"] == args._case_id), None)
if case is None:
    raise SystemExit(f"unknown frozen edge case {args._case_id!r}")
if len(case["points"]) != 3 or len(case["pressure"]) != 3:
    raise SystemExit("native adapter is bounded to the frozen three-point inputs")

expected_arrays = {
    "points_f32_bits": [f"{struct.unpack('<I', struct.pack('<f', float(v)))[0]:08x}" for point in case["points"] for v in point],
    "pressure_f32_bits": [f"{struct.unpack('<I', struct.pack('<f', float(v)))[0]:08x}" for v in case["pressure"]],
    "timestamps_i32": [100, 110, 120],
    "tilt_f32_bits": ["00000000"] * 3,
    "orientation_f32_bits": ["00000000"] * 3,
}

source = ORDINARY_CAPTURE.read_text()

def sub_once(old: str, new: str, label: str) -> None:
    global source
    count = source.count(old)
    if count != 1:
        raise SystemExit(f"ordinary capture adaptation {label!r}: expected one anchor, got {count}")
    source = source.replace(old, new, 1)

# Give the reused ordinary capture code these immutable case inputs.
sub_once(
    "CLI_ARGS = cli.parse_args()",
    "CLI_ARGS = cli.parse_args()\nCASE_SPEC = json.loads(os.environ['MAGICPEN_CASE_JSON'])\nCASE_ARGB = int(CASE_SPEC['argb'], 16)",
    "case input binding",
)
# Retain the real Drawing caller and every dependency check; adapt only its
# already-asserted source values and one explicit V1 advanced-setting case.
old_loop_output = "\"for label,advanced_value,advanced_mode in [('unset',None,'omitted'),('null-arg',None,'null'),('empty','','value'),('v1','1;','value'),('v2','2;','value')]:\\n mode='set'; size=10.0\\n loader.clear()\","
new_loop_output = "f\"for label,advanced_value,advanced_mode in [('v1','1;','value')]:\\n mode='set'; size={float(CASE_SPEC['size'])!r}\\n loader.clear()\","
sub_once(old_loop_output, new_loop_output, "source size and V1 selection")
sub_once("argb = 0x80336699", "argb = CASE_ARGB", "source ARGB setter")
old_color = "color_assert = \"if _row['native_model_metadata']!={'tool_type':2,'stroke_type':0,'curve_enabled':False,'color_argb':'0x80336699'}:raise RuntimeError('native ObjectStroke metadata getters differ from configured input')\""
new_color = "color_assert = f\"if _row['native_model_metadata']!={{'tool_type':2,'stroke_type':0,'curve_enabled':False,'color_argb':'0x{CASE_ARGB:08x}'}}:raise RuntimeError('native ObjectStroke metadata getters differ from configured input')\""
sub_once(old_color, new_color, "native source style assertion")
# Replace the saved source-channel assertion inside the ordinary helper's
# generated region with the exact f32 bit words derived from the input fixture.
old_arrays = "if _row['source_arrays']!={'points_f32_bits':['00000000','00000000','41200000','00000000','41a00000','40000000'],'pressure_f32_bits':['3e99999a','3f19999a','3f800000'],'timestamps_i32':[100,110,120],'tilt_f32_bits':['00000000']*3,'orientation_f32_bits':['00000000']*3}:raise RuntimeError('native ObjectStroke channel getters differ from configured input')"
new_arrays = f"if _row['source_arrays']!={expected_arrays!r}:raise RuntimeError('native ObjectStroke channel getters differ from configured input')"
sub_once(old_arrays, new_arrays, "native getter channel assertion")
# Replace hardcoded point and pressure patch code with input-derived literals.
old_point_patch = '''point_line="p.put(pts,'6f',0,0,10,0,20,2)"
if source.count(point_line)!=1:raise SystemExit('source point setter anchor changed')
source=source.replace(point_line,"p.put(pts,'6f',0,0,10,0,20,2)",1)'''
point_values = ",".join(repr(float(v)) for xy in case["points"] for v in xy)
pressure_values = ",".join(repr(float(v)) for v in case["pressure"])
new_point_patch = f'''point_line="p.put(pts,'6f',0,0,10,0,20,2)"
if source.count(point_line)!=1:raise SystemExit('source point setter anchor changed')
source=source.replace(point_line,"p.put(pts,'6f',{point_values})",1)
source=source.replace("p.put(prs,'3f',.3,.6,1.)","p.put(prs,'3f',{pressure_values})",1)'''
sub_once(old_point_patch, new_point_patch, "source arrays setter")
sub_once("assert record['native_model_metadata']['color_argb']=='0x80336699'", "assert record['native_model_metadata']['color_argb']==f'0x{CASE_ARGB:08x}'", "paint source color")
sub_once("assert len(vector_data)==8*12", "assert len(vector_data)==record['native_rtv1_vector']['float_count']*4", "native vector length")
sub_once(
    "control_argb=0x80336699; view_scale=int(os.environ.get('MAGICPEN_SCALE','1')); assert view_scale in (1,2,4); target_width=target_height=256*view_scale; translation=(0.0,float(target_height)); sample_center=(10*view_scale,target_height-1)",
    "control_argb=CASE_ARGB; view_scale=int(os.environ.get('MAGICPEN_SCALE','1')); assert view_scale in (1,2,4); target_width=target_height=256*view_scale; translation=(0.0,float(target_height)); sample_center=(int(round(CASE_SPEC['points'][1][0]*view_scale)),target_height-1-int(round(CASE_SPEC['points'][1][1]*view_scale)))",
    "native target and sample location",
)
sub_once(
    "assert bbox, 'native drawing produced no pixels'\nassert rgba_at(target_width//2,target_height//2)==list(background_rgba), 'outside-stroke pixel did not preserve the initialized target background'\nexpected_stroke_sample=([26,51,77,128] if background_kind=='transparent' else [58,115,173,255])",
    "assert rgba_at(target_width-1,0)==list(background_rgba), 'outside-stroke pixel did not preserve the initialized target background'\nexpected_stroke_sample=None",
    "permit zero-coverage alpha-zero control",
)
sub_once("'source_case':record['case']", "'source_case':CASE_SPEC['id'],'source_inputs':CASE_SPEC", "paint input identity")
old_source_input = "'source_input':{'pen_name':'com.samsung.android.sdk.pen.pen.preload.MagicPen','points':[[0,0],[10,0],[20,2]],'pressure':[0.3,0.6,1.0],'timestamps':[100,110,120],'tilt':[0,0,0],'orientation':[0,0,0],'tool_type':2,'stroke_type':0,'curve_enabled':False,'visible':True,'color_argb':'0x80336699'}"
new_source_input = "'source_input':{'case_id':CASE_SPEC['id'],'pen_name':'com.samsung.android.sdk.pen.pen.preload.MagicPen','points':CASE_SPEC['points'],'pressure':CASE_SPEC['pressure'],'timestamps':[100,110,120],'tilt':[0,0,0],'orientation':[0,0,0],'size':CASE_SPEC['size'],'tool_type':2,'stroke_type':0,'curve_enabled':False,'visible':True,'color_argb':CASE_SPEC['argb']}"
sub_once(old_source_input, new_source_input, "captured source input metadata")
sub_once("'MagicPen five-setting caller run; last explicit-empty case selected'", "'MagicPen saved Drawing caller; explicit V1 advanced setting selected'", "native route scope")

namespace = {"__name__": "__main__", "__file__": str(ORDINARY_CAPTURE)}
old_argv = sys.argv[:]
old_case = os.environ.get("MAGICPEN_CASE_JSON")
old_scale = os.environ.get("MAGICPEN_SCALE")
os.environ["MAGICPEN_CASE_JSON"] = json.dumps(case, separators=(",", ":"))
os.environ["MAGICPEN_SCALE"] = str(args._scale)
sys.argv = [str(ORDINARY_CAPTURE), "--_capture-background", args._capture_background]
output = io.StringIO()
try:
    with contextlib.redirect_stdout(output):
        exec(compile(source, str(ORDINARY_CAPTURE), "exec"), namespace)
finally:
    sys.argv = old_argv
    for key, old in (("MAGICPEN_CASE_JSON", old_case), ("MAGICPEN_SCALE", old_scale)):
        if old is None:
            os.environ.pop(key, None)
        else:
            os.environ[key] = old
value = json.loads(output.getvalue())
paint, capture = value["paint"], value["source_capture"]
if capture["source_input"]["case_id"] != case["id"] or capture["source_input"]["points"] != case["points"] or capture["source_input"]["pressure"] != case["pressure"]:
    raise SystemExit("native Model/ObjectStroke capture differs from the frozen case input")
if paint["source_inputs"] != case or paint["native_vector_bytes"] != len(bytes.fromhex(paint["native_vector_hex"])):
    raise SystemExit("native paint input/vector payload mismatch")
print(json.dumps(value, sort_keys=True))
