#!/usr/bin/env python3
"""Replay native source-to-RTV1 GLV1 cases with pinned local Samsung DSOs."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
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
FIXTURE = SCRIPT_DIR / "magicpen-curved-source-events-native.json"
LINEAGE_FIXTURES = {
    "original_eight_case_fixture_sha256": "00b0a6220fe675ef2281556c68a923275211abd5b61116faf394b5e75984659a",
    "rejected_terminal_four_case_fixture_sha256": "e5fe0823ffe75e2c0944abe396aa0b173c644d0307eb4f76bd8b23a2f8d0215f",
    "previous_twelve_case_fixture_sha256": "3694d91f841362c77deff494fc57b17b7629510118ba92f0b44947440ac7cbf9",
}
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

RUNTIME_PINS = {
    "python_executable_sha256": "889c603f0d17cb54060951bcf4c4f9b8c9ebd9e52b392c70209bbb9755d797d9",
    "unicorn_package_init_sha256": "9ad32123bca674cf205c308fd4d0d2a4db877c9dc2afac4feab7c84c8cf7043b",
    "unicorn_python_module_sha256": "e823b17649d0610c6f52b665902ac9dc46962785feebcac8465a0d7c477f25b7",
    "unicorn_native_library_sha256": "ddb196ec82b52e502c18e4a34478bf7b9f61c83c2ebaa95c74d8ded45a95da9c",
}


def runtime_closure() -> dict:
    import unicorn

    executable = Path(sys.executable).resolve()
    package_dir = Path(unicorn.__file__).resolve().parent
    paths = {
        "python_executable": executable,
        "unicorn_package_init": package_dir / "__init__.py",
        "unicorn_python_module": package_dir / "unicorn.py",
        "unicorn_native_library": package_dir / "lib/libunicorn.so.2",
    }
    hashes = {key: sha256(path.read_bytes()) for key, path in paths.items()}
    expected = dict(zip(paths.keys(), RUNTIME_PINS.values()))
    if hashes != expected:
        raise RuntimeError(f"native runner runtime closure changed: {hashes}")
    if platform.python_version() != "3.13.5" or unicorn.__version__ != "2.1.4":
        raise RuntimeError("native runner Python/Unicorn version changed")
    return {
        "python": {"path": str(executable), "version": platform.python_version(), "sha256": hashes["python_executable"]},
        "unicorn": {
            "version": unicorn.__version__,
            "package_path": str(package_dir.relative_to(ROOT)),
            "init_sha256": hashes["unicorn_package_init"],
            "python_module_sha256": hashes["unicorn_python_module"],
            "native_library_path": str(paths["unicorn_native_library"].relative_to(ROOT)),
            "native_library_sha256": hashes["unicorn_native_library"],
        },
    }


def preflight_existing_fixture(write_fixture: bool) -> None:
    runtime_closure()
    if write_fixture:
        return
    if not FIXTURE.exists():
        raise RuntimeError("curved fixture is missing; use --write-fixture after review")
    fixture = json.loads(FIXTURE.read_text())
    recorded = fixture.get("provenance", {})
    actual_runner = sha256(Path(__file__).read_bytes())
    if recorded.get("runner_sha256") != actual_runner:
        raise RuntimeError("curved native fixture runner SHA does not match this script")
    if recorded.get("runtime_closure") != runtime_closure():
        raise RuntimeError("curved native fixture runtime closure differs")

CASES = [
    ("tool2_baseline", [(0.0, 0.0), (10.0, 0.0), (20.0, 2.0)], [.3, .6, 1.0], [100, 110, 120], 10, "set", True),
    ("tool2_3_nonsymmetric", [(1.25, -2.5), (6.0, 1.0), (12.5, 7.25)], [.15, .55, .9], [100, 113, 137], 10, "set", True),
    ("tool2_5_nonsymmetric", [(2.0, 3.0), (8.0, -1.0), (14.0, 7.0), (31.0, 4.0), (42.0, 12.0)], [.15, .35, .65, .95, .55], [100, 117, 139, 181, 220], 10, "set", True),
    ("tool2_7_nonsymmetric", [(0.0, 0.0), (2.0, 1.0), (4.0, 0.0), (7.0, 3.0), (10.0, 1.0), (14.0, 4.0), (18.0, 2.0)], [.10, .20, .35, .50, .70, .85, 1.0], [100, 111, 127, 143, 162, 181, 203], 10, "set", True),
    ("tool2_8_duplicate_subthreshold", [(0.0, 0.0), (2.0, 1.0), (4.0, 0.0), (4.0, 0.0), (4.4, .3), (9.0, 2.0), (14.0, 4.0), (18.0, 2.0)], [.10, .20, .35, .50, .70, .85, .65, 1.0], [100, 111, 127, 143, 162, 181, 199, 224], 10, "set", True),
    ("tool2_4_boundary", [(0.,0.),(5.,2.),(11.,-1.),(19.,6.)], [.2,.4,.7,1.], [100,117,139,181], 10,"set",True),
    ("tool2_6_boundary", [(1.,2.),(4.,-1.),(9.,3.),(15.,8.),(23.,4.),(31.,10.)], [.1,.25,.45,.65,.85,1.], [100,111,127,143,162,181], 10,"set",True),
    ("tool2_9_boundary", [(0.,0.),(2.,1.),(5.,-2.),(9.,4.),(13.,8.),(18.,3.),(24.,9.),(30.,7.),(39.,12.)], [.1,.2,.3,.4,.5,.6,.8,.9,1.], [100,111,127,143,162,181,199,224,250], 10,"set",True),
    ("tool2_3_terminal_rejected", [(0.,0.),(7.,2.),(7.8,2.6)], [.10,.35,.95], [100,113,171], 10,"set",True),
    ("tool2_5_terminal_origin_rejected", [(-10.,2.),(-6.,3.),(-3.,5.),(-.6,.8),(0.,0.)], [.10,.3,.55,.2,.97], [100,111,127,143,199], 10,"set",True),
    ("tool2_6_terminal_rejected", [(0.,0.),(3.,1.),(7.,3.),(14.,8.),(20.,5.),(20.9,5.7)], [.10,.25,.45,.65,.85,.98], [100,111,127,143,162,199], 10,"set",True),
    ("tool2_9_duplicate_terminal_rejected", [(0.,0.),(2.,1.),(4.,0.),(4.,0.),(4.4,.3),(9.,2.),(14.,4.),(18.,2.),(18.7,2.8)], [.10,.20,.35,.50,.70,.85,.65,.55,.96], [100,111,127,143,162,181,199,224,260], 10,"set",True),
    ("tool2_3_duplicate_terminal_same_xy_pressure", [(0.,0.),(7.,2.),(7.,2.)], [.10,.40,.98], [100,113,171], 10,"set",True),
    ("curve_all_duplicates_varied_pressure", [(0.,0.),(0.,0.),(0.,0.),(0.,0.),(0.,0.)], [0.,.25,.5,.75,1.], [100,111,127,143,162], 10,"set",True),
    ("curve_all_subthreshold_no_admitted_line", [(0.,0.),(.5,.4),(1.,.8),(1.49,1.49),(.2,1.2)], [1.,.8,.6,.4,.2], [100,111,127,143,162], 10,"set",True),
    ("curve_2point_axis_subthreshold_1_499", [(0.,0.),(1.499,0.)], [0.,1.], [100,111], 10,"set",True),
    ("curve_2point_diagonal_axes_subthreshold_1_49", [(0.,0.),(1.49,1.49)], [0.,1.], [100,111], 10,"set",True),
    ("curve_2point_axis_equality_1_5", [(0.,0.),(1.5,0.)], [0.,1.], [100,111], 10,"set",True),
    ("curve_reject_first_then_admit", [(0.,0.),(.5,.5),(2.,0.),(5.,2.)], [.1,.9,.0,1.], [100,111,127,143], 10,"set",True),
    ("curve_reject_tail_pressure_zero_size_min", [(0.,0.),(3.,0.),(3.4,.4)], [.1,1.,0.], [100,111,127], .4,"set",True),
    ("curve_pressure_zero_one_size_min", [(0.,0.),(5.,0.),(10.,2.)], [0.,1.,0.], [100,111,127], .4,"set",True),
    ("curve_pressure_zero_one_size_max", [(0.,0.),(5.,0.),(10.,2.)], [0.,1.,0.], [100,111,127], 800.,"set",True),
    ("curve_pressure_edges_size_default", [(0.,0.),(2.,0.),(4.,1.),(7.,3.),(10.,1.)], [0.,1.,0.,1.,0.], [100,111,127,143,162], 10,"set",True),
    ("curve_terminal_factor_02_pressure_zero", [(0.,0.),(2.,0.),(4.,0.),(40.,0.),(42.,0.)], [.2,.3,.4,.5,0.], [100,111,127,143,162], 10,"set",True),
    ("curve_terminal_factor_02_pressure_one", [(0.,0.),(2.,0.),(4.,0.),(40.,0.),(42.,0.)], [.2,.3,.4,.5,1.], [100,111,127,143,162], 10,"set",True),
]



def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def f32word(value: float) -> str:
    return f"{struct.unpack('<I', struct.pack('<f', value))[0]:08x}"



def elf_vaddr_bytes(path: Path, va: int, length: int) -> bytes:
    data = path.read_bytes()
    if data[:6] != b"\x7fELF\x02\x01":
        raise RuntimeError("unexpected ELF encoding")
    eh = struct.unpack_from("<HHIQQQIHHHHHH", data, 16)
    phoff, phentsize, phnum = eh[4], eh[8], eh[9]
    for index in range(phnum):
        entry = struct.unpack_from("<IIQQQQQQ", data, phoff + index * phentsize)
        p_type, _flags, offset, vaddr, _paddr, filesz, _memsz, _align = entry
        if p_type == 1 and vaddr <= va and va + length <= vaddr + filesz:
            start = offset + va - vaddr
            return data[start:start + length]
    raise RuntimeError(f"ELF VA {va:#x} is outside PT_LOAD file bytes")


INSTRUCTION_CONTRACT = {
    "library": "scratch/apk-analysis-native/arm64-v8a/libSPenMagicPen.so",
    "library_sha256": "e4927da583e96142d072bb3edd977334aa34a58bb723ab1f9aaf531f3bd670a2",
    "functions": {
        "redrawLine": {
            "rva": "0x286d8",
            "description": "getRepeat result is loop count; loop draws exactly repeat getPosTan samples and excludes path endpoint.",
            "instructions_le_hex": {
                "0x2896c": "f503002a", "0x28998": "bf060071", "0x289a0": "cb060054",
                "0x289d4": "7b4b0094", "0x289ec": "814a0094", "0x28a5c": "b5060071", "0x28a70": "a1faff54"
            }
        },
        "redrawEndPen": {
            "rva": "0x28b30",
            "description": "getRepeat result is incremented before the terminal loop; it draws repeat+1 getPosTan samples including endpoint.",
            "instructions_le_hex": {
                "0x28df0": "84490094", "0x28e48": "16040011", "0x28e50": "0041201e",
                "0x28e60": "584a0094", "0x28e78": "5e490094", "0x28ee8": "d6060071", "0x28efc": "a1faff54"
            }
        }
    },
    "boundary_rule": "When an inclusive path sampler is flattened into one XYZ stream, each shared segment endpoint is stored once and assigned to the next source callback. The native line callback is half-open and the final end callback is closed; ranges are contiguous and preserve the full stream."
}


def verify_instruction_contract() -> dict:
    path = ROOT / INSTRUCTION_CONTRACT["library"]
    if sha256(path.read_bytes()) != INSTRUCTION_CONTRACT["library_sha256"]:
        raise RuntimeError("MagicPen DSO changed from pinned curved boundary contract")
    for function in INSTRUCTION_CONTRACT["functions"].values():
        for address, expected in function["instructions_le_hex"].items():
            actual = elf_vaddr_bytes(path, int(address, 16), 4).hex()
            if actual != expected:
                raise RuntimeError(f"native instruction mismatch at {address}: {actual} != {expected}")
    return INSTRUCTION_CONTRACT

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
source_line_events=[];source_end_events=[];source_line_active=[];source_end_active=[];redraw_phases=[];redraw_active=[];source_addpoint_events=[];source_addpoint_addr=p.syms['SPen::MagicPenStrokeDrawableRTV1::AddPoint(SPen::Vector2<float>, float)']
def _source_f32(obj,offset):return struct.unpack('<f',bytes(p.u.mem_read(obj+offset,4)))[0]
def _source_state(obj):return {'width_plus144_f32':_source_f32(obj,144),'raw_distance_plus148_f32':_source_f32(obj,148),'last_x_plus296_f32':_source_f32(obj,296),'last_y_plus300_f32':_source_f32(obj,300),'control_x_plus312_f32':_source_f32(obj,312),'control_y_plus316_f32':_source_f32(obj,316),'pending_init_flag_plus156':p.get(obj+156,'B')[0],'alternating_baseline_flag_plus157':p.get(obj+157,'B')[0]}
def _source_args(u):return [f'{u.reg_read(getattr(R,f"UC_ARM64_REG_S{i}"))&0xffffffff:08x}' for i in range(3)]
def _source_hook(u,a,z,d):
 if a in redraw_addrs:
  redraw_active.append({'phase_index':len(redraw_phases),'entry':redraw_addrs[a],'lr':u.reg_read(R.UC_ARM64_REG_LR),'addpoint_start':len(source_addpoint_events)})
 elif a==source_addpoint_addr:
  _words=[f'{u.reg_read(getattr(R,f"UC_ARM64_REG_S{i}"))&0xffffffff:08x}' for i in range(3)]
  source_addpoint_events.append({'entry':add_addrs[a],'receiver':hex(p.x(0)),'float_words_s0_s2':_words})
 elif a==redraw_line_addr:
  obj=p.x(0);lr=u.reg_read(R.UC_ARM64_REG_LR)
  source_line_active.append({'lr':lr,'obj':obj,'phase_index':redraw_active[-1]['phase_index'] if redraw_active else None,'args_f32_u32_hex':_source_args(u),'before':_source_state(obj),'addpoint_start':len(source_addpoint_events)})
 elif a==redraw_end_addr:
  obj=p.x(0);lr=u.reg_read(R.UC_ARM64_REG_LR)
  source_end_active.append({'lr':lr,'obj':obj,'phase_index':redraw_active[-1]['phase_index'] if redraw_active else None,'args_f32_u32_hex':_source_args(u),'before':_source_state(obj),'addpoint_start':len(source_addpoint_events)})
 elif source_line_active and a==source_line_active[-1]['lr']:
  e=source_line_active.pop();obj=e['obj'];e['after']=_source_state(obj);e['addpoint_end']=len(source_addpoint_events);e['addpoint_range']=[e['addpoint_start'],e['addpoint_end']];e['addpoint_count']=e['addpoint_end']-e['addpoint_start'];source_line_events.append(e)
 elif source_end_active and a==source_end_active[-1]['lr']:
  e=source_end_active.pop();obj=e['obj'];e['after']=_source_state(obj);e['addpoint_end']=len(source_addpoint_events);e['addpoint_range']=[e['addpoint_start'],e['addpoint_end']];e['addpoint_count']=e['addpoint_end']-e['addpoint_start'];source_end_events.append(e)
 elif redraw_active and a==redraw_active[-1]['lr']:
  e=redraw_active.pop();e['addpoint_end']=len(source_addpoint_events);e['addpoint_range']=[e['addpoint_start'],e['addpoint_end']];e['addpoint_count']=e['addpoint_end']-e['addpoint_start'];redraw_phases.append(e)'''
    event_replace = event_anchor + "\n " + event_hook.replace("\n", "\n ")
    injected += "assert source.count(" + repr(event_anchor) + ") == 1; source = source.replace(" + repr(event_anchor) + ", " + repr(event_replace) + ", 1)\n"
    injected += "assert source.count('trace_hook=p.u.hook_add(UC_HOOK_CODE,trace)') == 1; source=source.replace('trace_hook=p.u.hook_add(UC_HOOK_CODE,trace)', 'source_trace_hook=p.u.hook_add(UC_HOOK_CODE,_source_hook);trace_hook=p.u.hook_add(UC_HOOK_CODE,trace)', 1)\n"
    injected += "assert source.count('p.u.hook_del(trace_hook)') == 1; source=source.replace('p.u.hook_del(trace_hook)', 'p.u.hook_del(source_trace_hook);p.u.hook_del(trace_hook)', 1)\n"
    event_record_anchor = "records.append({'case':label,"
    event_record_replacement = "records.append({'case':label,'redraw_phases':redraw_phases,'source_addpoint_events':source_addpoint_events,'source_event_trace':{'curved_line_calls':source_line_events,'curved_end_calls':source_end_events},"
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
        phase_rows = row.get("redraw_phases", [])
        if len(phase_rows) != 1 or phase_rows[0]["addpoint_range"] != [0, len(row["rtv_addpoint_entries"])] or "GLV1::RedrawPen" not in phase_rows[0]["entry"]:
            raise RuntimeError(f"RedrawPen phase does not span exactly one native stream for {name}")
        trace = row["source_event_trace"]
        lines = trace["curved_line_calls"]
        terminals = trace["curved_end_calls"]
        if len(lines) != len(points) - 1 or len(terminals) != 1:
            raise RuntimeError(f"curved callback count differs from source point count for {name}")
        all_events = []
        for source_index, event in enumerate(lines, start=1):
            all_events.append((event, source_index, "line"))
        all_events.append((terminals[0], len(points) - 1, "terminal"))
        add_calls = row["rtv_addpoint_entries"]
        source_adds = row.get("source_addpoint_events", [])
        traced_adds = row["rtv_addpoint_entries"]
        traced_add_words = [[f"{int.from_bytes(bytes.fromhex(word), 'little'):08x}" for word in call["float_words_s0_s2"]] for call in traced_adds]
        if len(source_adds) != len(traced_adds) or any(a["float_words_s0_s2"] != b for a, b in zip(source_adds, traced_add_words)):
            raise RuntimeError(f"source hook and base AddPoint traces differ for {name}")
        if source_adds[0]["float_words_s0_s2"][:2] != [f32word(v) for v in points[0]]:
            raise RuntimeError(f"first native AddPoint position differs from raw source point zero for {name}")
        prior_end = 0
        initial_owner = next((kind for event, _, kind in all_events if event["addpoint_range"][0] <= 0 < event["addpoint_range"][1]), None)
        if initial_owner is None:
            raise RuntimeError(f"no callback interval owns the first AddPoint for {name}")
        for event, source_index, kind in all_events:
            start, end = event["addpoint_range"]
            if event["phase_index"] != phase_rows[0]["phase_index"] or start != prior_end or end < start:
                raise RuntimeError(f"callback AddPoint ranges are not contiguous/in-phase for {name}")
            if event["addpoint_count"] != end - start:
                raise RuntimeError(f"callback AddPoint range count differs for {name}")
            point_bits = [f32word(v) for v in points[source_index]]
            pressure_bit = f32word(pressure[source_index])
            if event["args_f32_u32_hex"] != [*point_bits, pressure_bit]:
                raise RuntimeError(f"native {kind} callback does not map ordinally to source event {source_index} for {name}: args={event['args_f32_u32_hex']} expected={[*point_bits, pressure_bit]}")
            event["source_event"] = {
                "index_zero_based": source_index,
                "point_f32_u32_hex": point_bits,
                "pressure_f32_u32_hex": pressure_bit,
                "timestamp_i32": times[source_index],
                "tilt_f32_u32_hex": "00000000",
                "orientation_f32_u32_hex": "00000000",
            }
            event["addpoint_prefix_range"] = [0, end]
            event["addpoint_records"] = source_adds[start:end]
            prior_end = end
        if prior_end != len(source_adds):
            raise RuntimeError(f"source callbacks do not account for the complete AddPoint stream for {name}")
        if row["source_arrays"] != expected_arrays:
            raise RuntimeError(f"source getter arrays differ for {name}")
        if name == "tool2_8_duplicate_subthreshold":
            by_source = {event["source_event"]["index_zero_based"]: event for event in lines}
            if any(by_source[index]["addpoint_count"] != 0 for index in (3, 4)):
                raise RuntimeError("duplicate and adjacent source callbacks unexpectedly emitted AddPoints")
        clean_trace = {
            key: [{k: v for k, v in event.items() if k not in ("lr", "obj")} for event in events]
            for key, events in trace.items()
        }
        clean_phases = [{k: v for k, v in phase.items() if k != "lr"} for phase in phase_rows]
        clean_adds = [{"entry": event["entry"], "float_words_s0_s2_u32_hex": event["float_words_s0_s2"]} for event in source_adds]
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
                "redraw_phase": clean_phases[0],
                "native_addpoint": {
                    "record_count": row["rtv_addpoint_record_count"],
                    "argument_words_f32_u32_hex": [f"{int.from_bytes(bytes.fromhex(word), 'little'):08x}" for call in row["rtv_addpoint_entries"] for word in call["float_words_s0_s2"]],
                    "captured_addpoint_records": clean_adds,
                    "source_event_trace": clean_trace,
                    "initial_addpoint_callback_owner": initial_owner,
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
    instruction_contract = verify_instruction_contract()
    return {
        "schema": "sdocx-magicpen-glv1-curved-source-events-v1",
        "scope": "Synthetic Model ObjectStroke getters/setters -> native Drawing selection -> one GLV1 RedrawPen -> curved redrawLine/redrawEndPen source callbacks -> RTV1 AddPoint and complete vector. Raw source indices are matched ordinally against actual getter arrays; callback AddPoint ranges are measured from native entry traces. No SDK deserialization or paint output.",
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
            "lineage_reference_hashes_not_read_at_replay": LINEAGE_FIXTURES,
            "capture_source": "Hash-pinned MagicPen saved-caller runner, parameterized by this script. A Unicorn code hook is added immediately before ObjectDrawing::drawObjectStroke and removed immediately after it; only that invocation contributes to phase and callback observations. Wrapper SHA-256 uses a stable <FIXTURE_OUTPUT> placeholder. Each callback includes its raw source ordinal and its exact AddPoint half-open range/records.",
            "native_instruction_contract": instruction_contract,
            "runtime_closure": runtime_closure(),
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
    preflight_existing_fixture(args.write_fixture)
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
