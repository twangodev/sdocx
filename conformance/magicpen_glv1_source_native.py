#!/usr/bin/env python3
"""Replay native source-to-RTV1 GLV1 cases with pinned local Samsung DSOs."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile

SCRIPT_DIR = Path(__file__).resolve().parent
ROOT = next(
    parent
    for parent in SCRIPT_DIR.parents
    if (parent / "Cargo.toml").is_file() and (parent / "conformance").is_dir()
)
BASE = ROOT / "conformance/magicpen_saved_caller_native.py"
FIXTURE = SCRIPT_DIR / "magicpen-glv1-source-native.json"
BASE_SHA256 = "cd2d613e202e35be261316a433533cda1c2f0ee76db150c4cca20963c80ba59c"
LIBRARIES = {
    "Base": "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb",
    "Composer": "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f",
    "DefaultPen": "1b37056c81646c407e9e0697c78e31b50ffab0bdb896966d1e89feb02385544f",
    "Drawing": "788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd",
    "FountainPen": "1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec",
    "Graphics": "aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621",
    "MagicPen": "e4927da583e96142d072bb3edd977334aa34a58bb723ab1f9aaf531f3bd670a2",
    "Model": "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a",
    "ObliquePen": "9b76bdb709db9f56c5d35a1a615fe41c57ceef73c559c91efaf5bf436f90e0ee",
    "PenCommon": "afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d",
    "Renderer": "f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed",
    "libc++_shared": "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4",
}
HELPERS = {
    "elf_plt.py": "816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b",
    "fountain_native.py": "40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30",
    "native_drawing_runtime.py": "65dd0a4294332b68410d9436762c4f0ad84a0ad4b2a21a742e7b098e089fba91",
    "oblique_canvas_native.py": "1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2",
}

CASES = [
    ("baseline", [(0, 0), (10, 0), (20, 2)], [.3, .6, 1], [100, 110, 120], 10, "set", True),
    ("pressure", [(0, 0), (10, 0), (20, 2)], [.1, .25, .9], [100, 110, 120], 10, "set", True),
    ("time", [(0, 0), (10, 0), (20, 2)], [.3, .6, 1], [100, 250, 900], 10, "set", True),
    ("time_500", [(0, 0), (10, 0), (20, 2)], [.3, .6, 1], [100, 600, 1100], 10, "set", True),
    ("time_1000", [(0, 0), (10, 0), (20, 2)], [.3, .6, 1], [100, 1100, 2100], 10, "set", True),
    ("time_2000", [(0, 0), (10, 0), (20, 2)], [.3, .6, 1], [100, 2100, 4100], 10, "set", True),
    ("time_5000", [(0, 0), (10, 0), (20, 2)], [.3, .6, 1], [100, 5100, 10100], 10, "set", True),
    ("time_10000", [(0, 0), (10, 0), (20, 2)], [.3, .6, 1], [100, 10100, 20100], 10, "set", True),
    ("time_slow", [(0, 0), (10, 0), (20, 2)], [.3, .6, 1], [100, 2100, 4100], 10, "set", True),
    ("time_very_slow", [(0, 0), (10, 0), (20, 2)], [.3, .6, 1], [100, 20100, 40100], 10, "set", True),
    ("size0", [(0, 0), (10, 0), (20, 2)], [.3, .6, 1], [100, 110, 120], 0, "set", True),
    ("size1", [(0, 0), (10, 0), (20, 2)], [.3, .6, 1], [100, 110, 120], 1, "set", True),
    ("size4", [(0, 0), (10, 0), (20, 2)], [.3, .6, 1], [100, 110, 120], 4, "set", True),
    ("size20", [(0, 0), (10, 0), (20, 2)], [.3, .6, 1], [100, 110, 120], 20, "set", True),
    ("size_omitted", [(0, 0), (10, 0), (20, 2)], [.3, .6, 1], [100, 110, 120], 0, "omit", True),
    ("size_negative", [(0, 0), (10, 0), (20, 2)], [.3, .6, 1], [100, 110, 120], -1, "set", True),
    ("size_below_floor", [(0, 0), (10, 0), (20, 2)], [.3, .6, 1], [100, 110, 120], .399, "set", True),
    ("size_at_floor", [(0, 0), (10, 0), (20, 2)], [.3, .6, 1], [100, 110, 120], .4, "set", True),
    ("size_above_floor", [(0, 0), (10, 0), (20, 2)], [.3, .6, 1], [100, 110, 120], .401, "set", True),
    ("two_points", [(0, 0), (10, 0)], [.3, 1], [100, 110], 10, "set", True),
    ("spacing", [(0, 0), (1, 0), (2, 0)], [.3, .6, 1], [100, 110, 120], 10, "set", True),
    ("four_points", [(0, 0), (5, 0), (10, 2), (20, 2)], [.2, .4, .8, 1], [100, 110, 120, 130], 10, "set", True),
    ("curve_off", [(0, 0), (10, 0), (20, 2)], [.3, .6, 1], [100, 110, 120], 10, "set", False),
    ("capture_four_points_arbitrary", [(2, 3), (8, -1), (14, 7), (31, 4)], [.15, .35, .65, .95], [100, 117, 139, 181], 10, "set", True),
    ("capture_five_points_terminal_registers", [(2, 3), (8, -1), (14, 7), (31, 4), (42, 12)], [.15, .35, .65, .95, .55], [100, 117, 139, 181, 220], 10, "set", True),
    ("capture_six_points_state", [(2, 3), (8, -1), (14, 7), (31, 4), (42, 12), (55, -6)], [.15, .35, .65, .95, .55, .85], [100, 117, 139, 181, 220, 275], 10, "set", True),

    # Nine native Drawing controls, recaptured under the explicit 1; setting.
    ('seven_irregular', [(0.0, 0.0), (2.0, 1.0), (4.0, 0.0), (7.0, 3.0), (10.0, 1.0), (14.0, 4.0), (18.0, 2.0)], [0.10000000149011612, 0.20000000298023224, 0.3499999940395355, 0.5, 0.699999988079071, 0.8500000238418579, 1.0], [100, 111, 127, 143, 162, 181, 203], 10, 'set', True),
    ('seven_duplicate_mid', [(0.0, 0.0), (2.0, 1.0), (4.0, 0.0), (4.0, 0.0), (10.0, 1.0), (14.0, 4.0), (18.0, 2.0)], [0.10000000149011612, 0.20000000298023224, 0.3499999940395355, 0.5, 0.699999988079071, 0.8500000238418579, 1.0], [100, 111, 127, 143, 162, 181, 203], 10, 'set', True),
    ('seven_subthreshold', [(0.0, 0.0), (2.0, 1.0), (4.0, 0.0), (4.400000095367432, 0.30000001192092896), (5.0, 0.4000000059604645), (9.0, 2.0), (18.0, 2.0)], [0.05000000074505806, 0.8999999761581421, 0.15000000596046448, 0.800000011920929, 0.20000000298023224, 1.0, 0.30000001192092896], [100, 107, 119, 133, 151, 177, 211], 10, 'set', True),
    ('eight_duplicate_pressure', [(0.0, 0.0), (2.0, 1.0), (4.0, 0.0), (7.0, 3.0), (7.0, 3.0), (12.0, 4.0), (16.0, 0.0), (21.0, 2.0)], [0.15000000596046448, 0.949999988079071, 0.10000000149011612, 0.800000011920929, 0.25, 0.6499999761581421, 0.4000000059604645, 1.0], [100, 109, 123, 141, 160, 177, 199, 224], 10, 'set', True),
    ('eight_subthreshold', [(0.0, 0.0), (1.0, 0.20000000298023224), (2.4000000953674316, 0.5), (3.0, 0.6000000238418579), (3.200000047683716, 0.699999988079071), (7.0, 1.0), (13.0, 4.0), (20.0, 2.0)], [0.8999999761581421, 0.10000000149011612, 0.800000011920929, 0.20000000298023224, 0.699999988079071, 0.30000001192092896, 0.949999988079071, 0.4000000059604645], [100, 108, 117, 128, 140, 155, 173, 194], 10, 'set', True),
    ('size_zero', [(0.0, 0.0), (2.0, 1.0), (4.0, 0.0), (7.0, 3.0), (10.0, 1.0), (14.0, 4.0), (18.0, 2.0)], [0.10000000149011612, 0.20000000298023224, 0.3499999940395355, 0.5, 0.699999988079071, 0.8500000238418579, 1.0], [100, 111, 127, 143, 162, 181, 203], 0, 'set', True),
    ('size_floor', [(0.0, 0.0), (2.0, 1.0), (4.0, 0.0), (7.0, 3.0), (10.0, 1.0), (14.0, 4.0), (18.0, 2.0)], [0.10000000149011612, 0.20000000298023224, 0.3499999940395355, 0.5, 0.699999988079071, 0.8500000238418579, 1.0], [100, 111, 127, 143, 162, 181, 203], 0.4, 'set', True),
    ('size_800', [(0.0, 0.0), (2.0, 1.0), (4.0, 0.0), (7.0, 3.0), (10.0, 1.0), (14.0, 4.0), (18.0, 2.0)], [0.10000000149011612, 0.20000000298023224, 0.3499999940395355, 0.5, 0.699999988079071, 0.8500000238418579, 1.0], [100, 111, 127, 143, 162, 181, 203], 800, 'set', True),
    ('size_over800', [(0.0, 0.0), (2.0, 1.0), (4.0, 0.0), (7.0, 3.0), (10.0, 1.0), (14.0, 4.0), (18.0, 2.0)], [0.10000000149011612, 0.20000000298023224, 0.3499999940395355, 0.5, 0.699999988079071, 0.8500000238418579, 1.0], [100, 111, 127, 143, 162, 181, 203], 801, 'set', True),

]


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def f32word(value: float) -> str:
    return f"{struct.unpack('<I', struct.pack('<f', value))[0]:08x}"


def replace_once(source: str, old: str, new: str, label: str) -> str:
    count = source.count(old)
    if count != 1:
        raise RuntimeError(f"patch {label}: expected one anchor, found {count}")
    return source.replace(old, new, 1)


def adapted_wrapper(case, output: Path) -> tuple[str, dict]:
    name, points, pressure, times, size, size_mode, curve = case
    wrapper = BASE.read_text()
    if sha256(wrapper.encode()) != BASE_SHA256:
        raise RuntimeError("pinned MagicPen saved-caller runner changed")
    wrapper = replace_once(wrapper, "ROOT = Path(__file__).resolve().parents[1]", f"ROOT = Path({str(ROOT)!r})", "root path")
    wrapper = replace_once(wrapper, "FIXTURE=ROOT/'conformance/magicpen-saved-caller-native.json'", f"FIXTURE=Path({str(output)!r})", "per-case output fixture")

    old_alloc = "pts=p.alloc(24);prs=p.alloc(12);ts=p.alloc(12);tilt=p.alloc(12);ori=p.alloc(12)"
    new_alloc = (
        f"points={points!r};pressures={pressure!r};times={times!r};count=len(points);"
        "pts=p.alloc(count*8);prs=p.alloc(count*4);ts=p.alloc(count*4);tilt=p.alloc(count*4);ori=p.alloc(count*4)"
    )
    old_put = "p.put(pts,'6f',0,0,10,0,20,2);p.put(prs,'3f',.3,.6,1.);p.put(ts,'3i',100,110,120);p.put(tilt,'3f',0,0,0);p.put(ori,'3f',0,0,0)"
    new_put = "p.put(pts,'f'*(count*2),*(v for xy in points for v in xy));p.put(prs,'f'*count,*pressures);p.put(ts,'i'*count,*times);p.put(tilt,'f'*count,*([0.]*count));p.put(ori,'f'*count,*([0.]*count))"
    expected_arrays = {
        "points_f32_bits": [f32word(v) for xy in points for v in xy],
        "pressure_f32_bits": [f32word(v) for v in pressure],
        "timestamps_i32": times,
        "tilt_f32_bits": ["00000000"] * len(points),
        "orientation_f32_bits": ["00000000"] * len(points),
    }
    expected_meta = {"tool_type": 2, "stroke_type": 0, "curve_enabled": bool(curve), "color_argb": "0xff000000"}
    old_arrays_check = "if _row['source_arrays']!={'points_f32_bits':['00000000','00000000','41200000','00000000','41a00000','40000000'],'pressure_f32_bits':['3e99999a','3f19999a','3f800000'],'timestamps_i32':[100,110,120],'tilt_f32_bits':['00000000']*3,'orientation_f32_bits':['00000000']*3}:raise RuntimeError('native ObjectStroke channel getters differ from configured input')"
    old_meta_check = "if _row['native_model_metadata']!={'tool_type':2,'stroke_type':0,'curve_enabled':True,'color_argb':'0xff000000'}:raise RuntimeError('native ObjectStroke metadata getters differ from configured input')"
    replacements = [
        (old_alloc, new_alloc, "source arrays allocation"),
        (old_put, new_put, "source arrays values"),
        ("[obj,pts,prs,ts,tilt,ori,3])", "[obj,pts,prs,ts,tilt,ori,count])", "source count"),
        ("call('SPen::ObjectStroke::SetCurveEnabled(bool)',[obj,1])", f"call('SPen::ObjectStroke::SetCurveEnabled(bool)',[obj,{int(curve)}])", "curve setter"),
        ("struct.unpack('<6I',bytes(p.u.mem_read(model_arrays['points'],24)))", "struct.unpack('<'+'I'*(model_count*2),bytes(p.u.mem_read(model_arrays['points'],model_count*8)))", "point getter extent"),
        ("struct.unpack('<3I',bytes(p.u.mem_read(model_arrays['pressure'],12)))", "struct.unpack('<'+'I'*model_count,bytes(p.u.mem_read(model_arrays['pressure'],model_count*4)))", "pressure getter extent"),
        ("struct.unpack('<3I',bytes(p.u.mem_read(model_arrays['tilt'],12)))", "struct.unpack('<'+'I'*model_count,bytes(p.u.mem_read(model_arrays['tilt'],model_count*4)))", "tilt getter extent"),
        ("struct.unpack('<3I',bytes(p.u.mem_read(model_arrays['orientation'],12)))", "struct.unpack('<'+'I'*model_count,bytes(p.u.mem_read(model_arrays['orientation'],model_count*4)))", "orientation getter extent"),
        ("struct.unpack('<3i',bytes(p.u.mem_read(model_arrays['timestamps'],12)))", "struct.unpack('<'+'i'*model_count,bytes(p.u.mem_read(model_arrays['timestamps'],model_count*4)))", "timestamp getter extent"),
        (old_arrays_check, f"if _row['source_arrays']!={expected_arrays!r}:raise RuntimeError('source getters differ for {name}')", "getter assertion"),
        ("if _row['source_model_get_point_count']!=3:raise RuntimeError('source ObjectStroke point count changed')", f"if _row['source_model_get_point_count']!={len(points)}:raise RuntimeError('point count differs for {name}')", "getter count assertion"),
        (old_meta_check, f"if _row['native_model_metadata']!={expected_meta!r}:raise RuntimeError('metadata getters differ for {name}')", "metadata assertion"),
        ("mode='set'; size=10.0", f"mode={size_mode!r}; size={size!r}", "size input"),
        ("'points':[[0,0],[10,0],[20,2]],'pressure':[0.3,0.6,1.0],'timestamps':[100,110,120]", f"'points':{points!r},'pressure':{pressure!r},'timestamps':{times!r},'size':{size!r},'curve_enabled':{curve!r}", "case metadata"),
        ("ROOT=Path(__file__).resolve().parents[1]; os.chdir(ROOT)", f"ROOT=Path({str(ROOT)!r}); os.chdir(ROOT)", "native runtime root"),
    ]
    injected = "\n# Deterministic source-channel adaptation with native getter assertions.\n"
    injected += "for _old, _new, _label in " + repr(replacements) + ":\n"
    injected += " if source.count(_old) != 1: raise RuntimeError(f'patch {_label}: expected one source anchor, found {source.count(_old)}')\n"
    injected += " source = source.replace(_old, _new, 1)\n"
    event_anchor = "redraw_names=['SPen::MagicPenStrokeDrawableGLV1::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)','SPen::MagicPenStrokeDrawableGLV2::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)']\n redraw_addrs={p.syms[name]:name for name in redraw_names}\n add_names=['SPen::MagicPenStrokeDrawableRTV1::AddPoint(SPen::Vector2<float>, float)'];add_addrs={p.syms[name]:name for name in add_names}"
    event_hook = r'''redraw_line_addr=p.syms['SPen::MagicPenStrokeDrawableGLV1::redrawLine(float, float, float, SPen::RectF*, long long)']
redraw_end_addr=p.syms['SPen::MagicPenStrokeDrawableGLV1::redrawEndPen(float, float, float, SPen::RectF*, long long)']
source_line_events=[];source_end_events=[];source_line_active=[];source_end_active=[]
def _source_f32(obj,offset):return struct.unpack('<f',bytes(p.u.mem_read(obj+offset,4)))[0]
def _source_state(obj):return {'width_plus144_f32':_source_f32(obj,144),'raw_distance_plus148_f32':_source_f32(obj,148),'last_x_plus296_f32':_source_f32(obj,296),'last_y_plus300_f32':_source_f32(obj,300),'control_x_plus312_f32':_source_f32(obj,312),'control_y_plus316_f32':_source_f32(obj,316),'pending_init_flag_plus156':p.get(obj+156,'B')[0],'alternating_baseline_flag_plus157':p.get(obj+157,'B')[0]}
def _source_hook(u,a,z,d):
 if a==redraw_line_addr:
  obj=p.x(0);lr=u.reg_read(R.UC_ARM64_REG_LR)
  source_line_active.append({'lr':lr,'obj':obj,'args_f32_u32_hex':[struct.pack('<I',u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))).hex() for i in range(3)],'before':_source_state(obj),'addpoint_start':len(add_calls)})
 elif a==redraw_end_addr:
  obj=p.x(0);lr=u.reg_read(R.UC_ARM64_REG_LR)
  source_end_active.append({'lr':lr,'obj':obj,'args_f32_u32_hex':[struct.pack('<I',u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))).hex() for i in range(3)],'before':_source_state(obj),'addpoint_start':len(add_calls)})
 elif source_line_active and a==source_line_active[-1]['lr']:
  e=source_line_active.pop();obj=e['obj'];e['after']=_source_state(obj);e['addpoint_count']=len(add_calls)-e['addpoint_start'];e['threshold_admitted']=(abs(_source_f32(0,0))<0) ;source_line_events.append(e)
 elif source_end_active and a==source_end_active[-1]['lr']:
  e=source_end_active.pop();obj=e['obj'];e['after']=_source_state(obj);e['addpoint_count']=len(add_calls)-e['addpoint_start'];source_end_events.append(e)
p.u.hook_add(UC_HOOK_CODE,_source_hook)'''
    event_hook = event_hook.replace("e['threshold_admitted']=(abs(_source_f32(0,0))<0)", "e['threshold_admitted']=(abs(struct.unpack('<f',bytes.fromhex(e['args_f32_u32_hex'][0]))[0]-e['before']['last_x_plus296_f32'])>=1.5 or abs(struct.unpack('<f',bytes.fromhex(e['args_f32_u32_hex'][1]))[0]-e['before']['last_y_plus300_f32'])>=1.5)")
    event_replace = event_anchor + "\n " + event_hook.replace("\n", "\n ")
    injected += "assert source.count(" + repr(event_anchor) + ") == 1; source = source.replace(" + repr(event_anchor) + ", " + repr(event_replace) + ", 1)\n"
    event_record_anchor = "records.append({'case':label,"
    event_record_replacement = "records.append({'case':label,'source_event_trace':{'redraw_line_calls':source_line_events,'redraw_end_calls':source_end_events},"
    injected += "assert source.count(" + repr(event_record_anchor) + ") == 1; source = source.replace(" + repr(event_record_anchor) + ", " + repr(event_record_replacement) + ", 1)\n"
    marker = 'exec(compile(source, str(BASE), "exec"), {"__name__": "__main__", "__file__": str(Path(__file__).resolve())})'
    wrapper = replace_once(wrapper, marker, injected + marker, "source adapter insertion")
    return wrapper, expected_arrays

def capture(case) -> dict:
    with tempfile.TemporaryDirectory(prefix="sdocx-magicpen-glv1-", dir=ROOT / "tmp") as tmp:
        tmpdir = Path(tmp)
        output = tmpdir / "capture.json"
        wrapper, expected_arrays = adapted_wrapper(case, output)
        stable_wrapper_sha256 = sha256(wrapper.replace(str(output), "<FIXTURE_OUTPUT>").encode())
        script = tmpdir / "capture.py"
        script.write_text(wrapper)
        env = os.environ.copy()
        env["PYTHONPATH"] = os.pathsep.join(
            [
                str(ROOT / "scratch/apk-analysis-runtime/python"),
                str(ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin"),
                str(ROOT / "conformance"),
            ]
        )
        subprocess.run([sys.executable, str(script), "--write-fixture"], cwd=ROOT, env=env, check=True)
        result = json.loads(output.read_text())
        row = next(r for r in result["cases"] if r["case"] == "v1")
        name, points, pressure, times, size, _, curve = case
        if row["source_arrays"] != expected_arrays:
            raise RuntimeError(f"native getter arrays differ for {name}")
        if row["source_model_get_pen_size_f32_bits"] != struct.pack("<f", size if size >= 0 else 0).hex():
            raise RuntimeError(f"native source-size getter differs for {name}")
        if row["native_model_metadata"] != {"tool_type": 2, "stroke_type": 0, "curve_enabled": curve, "color_argb": "0xff000000"}:
            raise RuntimeError(f"native metadata getter differs for {name}")
        requested_size = struct.unpack("<f", struct.pack("<f", size if size >= 0 else 0))[0]
        actual_pen_size = min(800.0, max(0.4, requested_size))
        expected_pen_bits = struct.pack("<f", actual_pen_size).hex()
        if row["native_pen_getsize_f32_bits"] != expected_pen_bits:
            raise RuntimeError(f"selected native Pen GetSize differs for {name}: {row['native_pen_getsize_f32_bits']} != {expected_pen_bits}")
        if row["selected_drawable_class"] != "GLV1" or row["drawing_return"] != 1:
            raise RuntimeError(f"Drawing did not select/run GLV1 for {name}")
        native = row["native_rtv1_vector"]
        raw = bytes.fromhex(native["bytes_le_hex"])
        if len(raw) != native["float_count"] * 4 or sha256(raw) != native["sha256"]:
            raise RuntimeError(f"incomplete native RTV1 buffer for {name}")
        words = [f"{int.from_bytes(raw[i:i+4], 'little'):08x}" for i in range(0, len(raw), 4)]
        if len(words) % 3:
            raise RuntimeError(f"incomplete XYZ records for {name}")
        return {
            "id": name,
            "input": {
                "points_f32_u32_hex": expected_arrays["points_f32_bits"],
                "pressure_f32_u32_hex": expected_arrays["pressure_f32_bits"],
                "timestamps_i32": times,
                "size_f32_u32_hex": f"{struct.unpack('<I', struct.pack('<f', size if size >= 0 else 0))[0]:08x}",
                "curve_enabled": curve,
                "tool_type": 2,
                "stroke_type": 0,
                "advanced_setting": "1;",
            },
            "native": {"record_count": len(words) // 3, "xyz_f32_u32_hex": words, "sha256": sha256(raw)},
            "capture": {
                "source_case": row["case"],
                "profile": result["profile"],
                "base_caller_sha256": result["provenance"]["base_caller_sha256"],
                "wrapper_sha256": stable_wrapper_sha256,
                "libraries": result["provenance"]["libraries"],
                "helpers": result["provenance"]["helpers"],
                "native_scope": row["evidence_scope"],
                "source_getters": {
                    "point_count": row["source_model_get_point_count"],
                    "arrays": row["source_arrays"],
                    "metadata": row["native_model_metadata"],
                    "source_pen_size_f32_u32_hex": row["source_model_get_pen_size_f32_bits"],
                    "selected_pen_get_size_f32_u32_hex": row["native_pen_getsize_f32_bits"],
                    "selected_drawable_class": row["selected_drawable_class"],
                },
                "native_addpoint": {
                    "record_count": row["rtv_addpoint_record_count"],
                    "argument_words_f32_u32_hex": [word for call in row["rtv_addpoint_entries"] for word in call["float_words_s0_s2"]],
                    "source_event_trace": row["source_event_trace"],
                    "post_return_vector_bytes_le_hex": row["native_rtv1_vector"]["bytes_le_hex"],
                },
            },
        }


def generated_fixture() -> dict:
    cases = [capture(case) for case in CASES]
    if any(case["capture"]["libraries"] != LIBRARIES for case in cases):
        raise RuntimeError("native-library pins differ from the reviewed closure")
    if any(case["capture"]["helpers"] != HELPERS for case in cases):
        raise RuntimeError("native helper source pins differ from the reviewed closure")
    if len({case["capture"]["base_caller_sha256"] for case in cases}) != 1:
        raise RuntimeError("base caller changed during capture matrix")
    return {
        "schema": "sdocx-magicpen-glv1-source-native-v1",
        "scope": "Synthetic Model ObjectStroke source getters -> Drawing-selected GLV1 -> RTV1 vector. No SDK deserialization, GLV2, or paint output. Expected XYZ words are native assertion outputs.",
        "encoding": {
            "source_point_and_pressure_words": "8-digit hexadecimal uint32 word; interpret as IEEE-754 f32 bits",
            "native_xyz_words": "8-digit hexadecimal uint32 word; native vector bytes decoded little-endian into f32 words",
            "size_word": "8-digit hexadecimal uint32 word; decoded from source getter byte encoding",
        },
        "provenance": {
            "runner_sha256": sha256(Path(__file__).read_bytes()),
            "base_caller_sha256": cases[0]["capture"]["base_caller_sha256"],
            "libraries": LIBRARIES,
            "helpers": HELPERS,
            "capture_source": "Hash-pinned MagicPen saved-caller runner, parameterized by this script; wrapper SHA-256 uses a stable <FIXTURE_OUTPUT> placeholder for the temporary output path. Each case records its adapted wrapper digest and actual getter/output capture.",
        },
        "cases": cases,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--write-fixture", action="store_true")
    modes.add_argument("--check-fixture", action="store_true")
    parser.add_argument("--case", help="check only this captured case; writing always regenerates the full matrix")
    args = parser.parse_args()
    if args.case and args.write_fixture:
        parser.error("--case cannot be combined with --write-fixture; fixture writes always capture the full matrix")
    if args.case:
        match = next((case for case in CASES if case[0] == args.case), None)
        if match is None:
            parser.error(f"unknown case {args.case!r}")
        capture_row = capture(match)
        expected = json.loads(FIXTURE.read_text())
        expected_row = next((case for case in expected["cases"] if case["id"] == args.case), None)
        if expected_row != capture_row:
            raise SystemExit(f"MagicPen GLV1 source fixture mismatch for {args.case}")
        print(f"verified native GLV1 source case {args.case}")
        return
    current = generated_fixture()
    encoded = json.dumps(current, indent=2) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
    else:
        if not FIXTURE.exists() or FIXTURE.read_text() != encoded:
            raise SystemExit("MagicPen GLV1 source fixture mismatch; use --write-fixture after review")
    print(f"verified {len(current['cases'])} native GLV1 source cases")


if __name__ == "__main__":
    main()
