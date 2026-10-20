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
FIXTURE = SCRIPT_DIR / "magicpen-nocurve-source-events-native.json"
BASE_SHA256 = "cd2d613e202e35be261316a433533cda1c2f0ee76db150c4cca20963c80ba59c"
BASELINE_RUNNER = ROOT / "conformance/magicpen_glv1_source_native.py"
BASELINE_FIXTURE = ROOT / "conformance/magicpen-glv1-source-native.json"
BASELINE_RUNNER_SHA256 = "b76a02a7b37438ddc303b06858e952bbc3fd6d770044b03e95465b64931f42c0"
BASELINE_FIXTURE_SHA256 = "a67b0260313b1e5ede189a32864da5dc9e3eaab4909f832308560186459cc34b"
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
    ("duplicate_after_accepted", [(0, 0), (2, 0), (2, 0)], [.2, .4, .6], [100, 110, 120], 10, "set", False),
    ("axis_x_exact_1_5", [(0, 0), (2, 0), (3.5, 0)], [.2, .4, .6], [100, 110, 120], 10, "set", False),
    ("two_point_short", [(0, 0), (2, 0)], [.2, .4], [100, 110], 10, "set", False),
    ("all_after_first_rejected", [(0, 0), (2, 0), (2.5, .25), (3.0, .5), (3.25, .75)], [.2, .4, .6, .8, 1.0], [100, 110, 120, 130, 140], 10, "set", False),
    ("pressure_outside_range", [(0, 0), (2, 0), (4, 0), (6, 0)], [-.25, 0.0, 1.25, 1.5], [100, 110, 120, 130], 10, "set", False),
    ("axis_below_f32_predecessor", [(0,0),(2,0),(3.499999761581421,0)], [.1,.4,.6], [100,110,120], 10, "set", False),
    ("all_rejected_after_first_3pt", [(0,0),(2,0),(2.5,0)], [.1,.4,.6], [100,110,120], 10, "set", False),
    ("high_endpoint_pressure", [(0,0),(10,0),(20,0)], [.3,.6,1.25], [100,110,120], 10, "set", False),
    ("high_middle_pressure", [(0,0),(10,0),(20,0)], [.3,1.25,.6], [100,110,120], 10, "set", False),
    ("negative_moved_pressure", [(0,0),(10,0),(20,0)], [.3,-.25,.6], [100,110,120], 10, "set", False),
    ("negative_start_pressure", [(0,0),(10,0),(20,0)], [-.25,.6,.8], [100,110,120], 10, "set", False),
    ("tiny_rejected_terminal", [(0,0),(2,0),(2.08,0)], [.2,.4,.6], [100,110,120], 10, "set", False),
    ("first_rejected_then_admitted", [(0,0),(.5,0),(2,0)], [.1,.2,.7], [100,110,120], 10, "set", False),
    ("two_subthreshold_terminal_nonzero", [(0,0),(.5,0),(1,0)], [.1,.4,.8], [100,110,120], 10, "set", False),
    ("all_rejected_final_origin", [(0,0),(.5,.25),(0,0)], [.1,.4,.8], [100,110,120], 10, "set", False),
    ("duplicate_all_points", [(1,1),(1,1),(1,1)], [.1,.4,.8], [100,110,120], 10, "set", False),
    ("repeat_bucket_width_50_len24", [(0,0),(24.26,0),(48.52,0)], [.1,.1,.1], [100,111,122], 500, "set", False),
    ("repeat_bucket_width_80_len24", [(0,0),(24.26,0),(48.52,0)], [.1,.1,.1], [100,111,122], 800, "set", False),
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
        ("p.u.hook_del(trace_hook)", "p.u.hook_del(source_trace_hook);p.u.hook_del(trace_hook)", "remove case-local source hook"),
    ]
    injected = "\n# Deterministic source-channel adaptation with native getter assertions.\n"
    injected += "for _old, _new, _label in " + repr(replacements) + ":\n"
    injected += " if source.count(_old) != 1: raise RuntimeError(f'patch {_label}: expected one source anchor, found {source.count(_old)}')\n"
    injected += " source = source.replace(_old, _new, 1)\n"
    event_anchor = "redraw_names=['SPen::MagicPenStrokeDrawableGLV1::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)','SPen::MagicPenStrokeDrawableGLV2::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)']\n redraw_addrs={p.syms[name]:name for name in redraw_names}\n add_names=['SPen::MagicPenStrokeDrawableRTV1::AddPoint(SPen::Vector2<float>, float)'];add_addrs={p.syms[name]:name for name in add_names}"
    event_hook = r'''redraw_line_addr=p.syms['SPen::MagicPenStrokeDrawableGLV1::redrawLine_NoCurve(float, float, float, SPen::RectF*)']
redraw_end_addr=p.syms['SPen::MagicPenStrokeDrawableGLV1::redrawEndPen_NoCurve(float, float, float, SPen::RectF*)']
source_line_events=[];source_end_events=[];source_line_active=[];source_end_active=[];redraw_phases=[];redraw_active=[]
def _source_f32(obj,offset):return struct.unpack('<f',bytes(p.u.mem_read(obj+offset,4)))[0]
def _source_state(obj):return {'width_plus144_f32':_source_f32(obj,144),'raw_distance_plus148_f32':_source_f32(obj,148),'last_x_plus296_f32':_source_f32(obj,296),'last_y_plus300_f32':_source_f32(obj,300),'control_x_plus312_f32':_source_f32(obj,312),'control_y_plus316_f32':_source_f32(obj,316),'pending_init_flag_plus156':p.get(obj+156,'B')[0],'alternating_baseline_flag_plus157':p.get(obj+157,'B')[0]}
def _source_args(u):return [struct.pack('<I',u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))).hex() for i in range(3)]
def _source_hook(u,a,z,d):
 if a in redraw_addrs:
  redraw_active.append({'phase_index':len(redraw_phases),'entry':redraw_addrs[a],'lr':u.reg_read(R.UC_ARM64_REG_LR),'addpoint_start':len(add_calls)})
 elif a==redraw_line_addr:
  obj=p.x(0);lr=u.reg_read(R.UC_ARM64_REG_LR)
  source_line_active.append({'lr':lr,'obj':obj,'phase_index':redraw_active[-1]['phase_index'] if redraw_active else None,'args_f32_u32_hex':_source_args(u),'before':_source_state(obj),'addpoint_start':len(add_calls)})
 elif a==redraw_end_addr:
  obj=p.x(0);lr=u.reg_read(R.UC_ARM64_REG_LR)
  source_end_active.append({'lr':lr,'obj':obj,'phase_index':redraw_active[-1]['phase_index'] if redraw_active else None,'args_f32_u32_hex':_source_args(u),'before':_source_state(obj),'addpoint_start':len(add_calls)})
 elif source_line_active and a==source_line_active[-1]['lr']:
  e=source_line_active.pop();obj=e['obj'];e['after']=_source_state(obj);e['addpoint_end']=len(add_calls);e['addpoint_range']=[e['addpoint_start'],e['addpoint_end']];e['addpoint_count']=e['addpoint_end']-e['addpoint_start'];e['threshold_admitted']=(abs(struct.unpack('<f',bytes.fromhex(e['args_f32_u32_hex'][0]))[0]-e['before']['last_x_plus296_f32'])>=1.5 or abs(struct.unpack('<f',bytes.fromhex(e['args_f32_u32_hex'][1]))[0]-e['before']['last_y_plus300_f32'])>=1.5);source_line_events.append(e)
 elif source_end_active and a==source_end_active[-1]['lr']:
  e=source_end_active.pop();obj=e['obj'];e['after']=_source_state(obj);e['addpoint_end']=len(add_calls);e['addpoint_range']=[e['addpoint_start'],e['addpoint_end']];e['addpoint_count']=e['addpoint_end']-e['addpoint_start'];source_end_events.append(e)
 elif redraw_active and a==redraw_active[-1]['lr']:
  e=redraw_active.pop();e['addpoint_end']=len(add_calls);e['addpoint_range']=[e['addpoint_start'],e['addpoint_end']];e['addpoint_count']=e['addpoint_end']-e['addpoint_start'];redraw_phases.append(e)
source_trace_hook=p.u.hook_add(UC_HOOK_CODE,_source_hook)'''
    event_replace = event_anchor + "\n " + event_hook.replace("\n", "\n ")
    injected += "assert source.count(" + repr(event_anchor) + ") == 1; source = source.replace(" + repr(event_anchor) + ", " + repr(event_replace) + ", 1)\n"
    event_record_anchor = "records.append({'case':label,"
    event_record_replacement = "records.append({'case':label,'redraw_phases':redraw_phases,'source_event_trace':{'no_curve_line_calls':source_line_events,'no_curve_end_calls':source_end_events},"
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
        if row["source_event_trace"]["no_curve_line_calls"] and len(row["redraw_phases"]) != 1:
            raise RuntimeError(f"expected one selected GLV1 redraw phase for {name}")
        phase = row["redraw_phases"][0]
        if "GLV1::RedrawPen" not in phase["entry"] or phase["addpoint_range"] != [0, len(row["rtv_addpoint_entries"])]:
            raise RuntimeError(f"RedrawPen/AddPoint range does not span the native stream for {name}")
        event_trace = row["source_event_trace"]
        ordered = event_trace["no_curve_line_calls"] + event_trace["no_curve_end_calls"]
        for event in ordered:
            start, end = event["addpoint_range"]
            if event["phase_index"] != phase["phase_index"] or end < start or event["addpoint_count"] != end - start:
                raise RuntimeError(f"invalid event range/phase for {name}")
        line_ranges = [event["addpoint_range"] for event in event_trace["no_curve_line_calls"]]
        end_ranges = [event["addpoint_range"] for event in event_trace["no_curve_end_calls"]]
        if any(left[1] != right[0] for left, right in zip(line_ranges, line_ranges[1:])):
            raise RuntimeError(f"NoCurve line AddPoint ranges are not contiguous for {name}")
        line_end = line_ranges[-1][1] if line_ranges else 0
        if end_ranges and end_ranges[0][0] != line_end:
            raise RuntimeError(f"NoCurve terminal range does not follow line callbacks for {name}")
        if end_ranges and any(left[1] != right[0] for left, right in zip(end_ranges, end_ranges[1:])):
            raise RuntimeError(f"NoCurve terminal ranges are not contiguous for {name}")
        if event_trace["no_curve_line_calls"] and event_trace["no_curve_end_calls"]:
            if line_end + sum(e["addpoint_count"] for e in event_trace["no_curve_end_calls"]) != len(row["rtv_addpoint_entries"]):
                raise RuntimeError(f"line and terminal callbacks do not account for the AddPoint stream in {name}")
        callback_words = [word for call in row["rtv_addpoint_entries"] for word in call["float_words_s0_s2"]]
        expected_vector_words = []
        for offset in range(0, len(callback_words), 3):
            x_word, y_word, z_word = callback_words[offset:offset + 3]
            x_bits = int.from_bytes(bytes.fromhex(x_word), "little")
            y_bits = int.from_bytes(bytes.fromhex(y_word), "little")
            z = struct.unpack("<f", bytes.fromhex(z_word))[0]
            z_plus_half = int.from_bytes(struct.pack("<f", z + 0.5), "little")
            expected_vector_words.extend([f"{x_bits:08x}", f"{y_bits:08x}", f"{z_plus_half:08x}"])
        if expected_vector_words != words:
            raise RuntimeError(f"RTV1 AddPoint xyz args plus native z offset differ from full post-return vector for {name}")
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
                "redraw_phases": row["redraw_phases"],
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
    if sha256(BASELINE_RUNNER.read_bytes()) != BASELINE_RUNNER_SHA256:
        raise RuntimeError("committed 35-case native runner changed")
    if sha256(BASELINE_FIXTURE.read_bytes()) != BASELINE_FIXTURE_SHA256:
        raise RuntimeError("committed 35-case native fixture changed")
    env = os.environ.copy()
    env["PYTHONPATH"] = os.pathsep.join([
        str(ROOT / "scratch/apk-analysis-runtime/python"),
        str(ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin"),
        str(ROOT / "conformance"),
    ])
    subprocess.run([sys.executable, str(BASELINE_RUNNER), "--check-fixture"], cwd=ROOT, env=env, check=True)
    baseline = json.loads(BASELINE_FIXTURE.read_text())
    if baseline.get("schema") != "sdocx-magicpen-glv1-source-native-v1" or len(baseline.get("cases", [])) != 35:
        raise RuntimeError("committed baseline is not the expected 35-case source oracle")
    added = [capture(case) for case in CASES]
    if any(case["capture"]["libraries"] != LIBRARIES for case in added):
        raise RuntimeError("native-library pins differ from the reviewed closure")
    if any(case["capture"]["helpers"] != HELPERS for case in added):
        raise RuntimeError("native helper source pins differ from the reviewed closure")
    if len({case["capture"]["base_caller_sha256"] for case in added}) != 1:
        raise RuntimeError("base caller changed during capture matrix")
    ids = [case["id"] for case in baseline["cases"] + added]
    if len(set(ids)) != 53:
        raise RuntimeError(f"expected 51 unique cases, found {len(set(ids))}")
    return {
        "schema": "sdocx-magicpen-glv1-nocurve-source-events-native-v1",
        "scope": "35 unchanged committed source-sampler cases plus 18 fresh NoCurve event-boundary cases. Synthetic Model ObjectStroke setters/getters -> Drawing-selected GLV1 -> native RTV1 AddPoint and complete post-return vectors. No SDK deserialization, GLV2, texture binding, GPU draw, or paint output.",
        "encoding": {
            "source_point_and_pressure_words": "8-digit hexadecimal uint32 word; interpret as IEEE-754 f32 bits",
            "native_xyz_words": "8-digit hexadecimal uint32 word; native vector bytes decoded little-endian into f32 words",
            "size_word": "8-digit hexadecimal uint32 word; decoded from source getter byte encoding",
        },
        "provenance": {
            "runner_sha256": sha256(Path(__file__).read_bytes()),
            "baseline_runner_sha256": BASELINE_RUNNER_SHA256,
            "baseline_fixture_sha256": BASELINE_FIXTURE_SHA256,
            "base_caller_sha256": added[0]["capture"]["base_caller_sha256"],
            "libraries": LIBRARIES,
            "helpers": HELPERS,
            "capture_source": "The original 35 cases are copied byte-for-byte from the hash-pinned committed fixture after its own --check-fixture performs a fresh native replay. Each additional case runs an asserted in-memory adaptation of the hash-pinned MagicPen saved-caller wrapper. Wrapper SHA-256 uses a stable <FIXTURE_OUTPUT> placeholder. Every additional case records all source getter arrays, native metadata, per-RedrawPen invocation ranges, NoCurve line/end callback entry/return ranges, AddPoint args, and full post-return vector bytes.",
        },
        "cases": baseline["cases"] + added,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--write-fixture", action="store_true")
    modes.add_argument("--check-fixture", action="store_true")
    args = parser.parse_args()
    current = generated_fixture()
    encoded = json.dumps(current, indent=2) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
    else:
        if not FIXTURE.exists() or FIXTURE.read_text() != encoded:
            raise SystemExit("MagicPen GLV1 source fixture mismatch; use --write-fixture after review")
    print(f"verified {len(current['cases'])} MagicPen source cases (35 baseline + 18 NoCurve event controls)")


if __name__ == "__main__":
    main()
