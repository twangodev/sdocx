#!/usr/bin/env python3
"""Replay one persisted FountainPen V14 object through native Drawing.

Default mode compares a fresh native execution with the checked-in fixture;
--write-fixture is the only mode that changes it. The 1,998-byte saved frame
comes from fountain-saved-color-native.json. The 122-byte StringID table and
only 121 bytes of object prefix plus 30 bytes of page header are retained here,
not the source archive.
"""
import ast
import argparse
import hashlib
import json
import os
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "conformance/fountain-persisted-drawing-native.json"
SOURCE_FIXTURE = ROOT / "conformance/fountain-saved-color-native.json"
BASE_CALLER = ROOT / "conformance/fountain_saved_caller_native.py"
SOURCE_FIXTURE_SHA256 = "de23cf21f239fc9e12794bc55777fa18406d1129162f8198b1544b8f009f932d"
BASE_CALLER_SHA256 = "b0837c86483aa438923f9d4d1c6451b1ebc1c5fec4c5f7c447b928a25a8f7b19"

parser = argparse.ArgumentParser(description="Pinned persisted FountainPen V14 Drawing oracle")
group = parser.add_mutually_exclusive_group()
group.add_argument("--write-fixture", action="store_true", help="explicitly write/refresh native capture")
group.add_argument("--check-fixture", action="store_true", help="replay and compare capture (default)")
args = parser.parse_args()

def sha(data):
    return hashlib.sha256(data).hexdigest()

def pinned(path, expected):
    actual = sha(path.read_bytes())
    if actual != expected:
        raise SystemExit(f"pinned input changed: {path.relative_to(ROOT)} ({actual})")
    return actual

def pin_runtime_file(label, path, expected):
    path = Path(path).resolve()
    actual = sha(path.read_bytes())
    if actual != expected:
        raise SystemExit(f"pinned runtime input changed: {label} ({path} {actual})")
    return {"path": str(path), "sha256": actual}

source_fixture_bytes = SOURCE_FIXTURE.read_bytes()
pinned(SOURCE_FIXTURE, SOURCE_FIXTURE_SHA256)
base_source = BASE_CALLER.read_text()
pinned(BASE_CALLER, BASE_CALLER_SHA256)
RUNNER_SHA256 = sha(Path(__file__).read_bytes())

PYTHON_SHA256 = "889c603f0d17cb54060951bcf4c4f9b8c9ebd9e52b392c70209bbb9755d797d9"
if sys.version_info[:3] != (3, 13, 5):
    raise SystemExit(f"unexpected Python runtime: {sys.version.split()[0]}")
python_runtime = pin_runtime_file("python", sys.executable, PYTHON_SHA256)
import unicorn
import unicorn.unicorn_py3.unicorn as unicorn_backend
if unicorn.__version__ != "2.1.4":
    raise SystemExit(f"unexpected Unicorn version: {unicorn.__version__}")
unicorn_root = Path(unicorn.__file__).resolve().parent
unicorn_runtime = {
    "version": unicorn.__version__,
    "files": {
        "package_init": pin_runtime_file("Unicorn package init", unicorn_root / "__init__.py",
                                          "9ad32123bca674cf205c308fd4d0d2a4db877c9dc2afac4feab7c84c8cf7043b"),
        "python_backend": pin_runtime_file("Unicorn Python backend", unicorn_backend.__file__,
                                           "62ffe84341f4288e309040e06555b81798031817d32fd19a945e250813f872f0"),
        "native_library": pin_runtime_file("Unicorn shared library", unicorn_root / "lib/libunicorn.so.2",
                                           "ddb196ec82b52e502c18e4a34478bf7b9f61c83c2ebaa95c74d8ded45a95da9c"),
    },
}
TOOL_SHA256 = {
    "nm": "ddb55c0685721bf494aa2d090056b8b32a40614ad4464b305896f527d882c3c1",
    "c++filt": "7d54ea6797a5e8c69255f711b79577efd9934c3502c6edee4bc25b7dc7d97bd2",
}
executed_tools = {}
for tool, expected in TOOL_SHA256.items():
    tool_path = shutil.which(tool)
    if tool_path is None:
        raise SystemExit(f"required caller tool is unavailable: {tool}")
    executed_tools[tool] = pin_runtime_file(tool, tool_path, expected)
source_fixture = json.loads(source_fixture_bytes)
saved = next(row for row in source_fixture["real_serialized_frames"] if row["id"] == "real_v14_handwritten_flat1445")
base_assignments = {node.targets[0].id: ast.literal_eval(node.value)
                    for node in ast.walk(ast.parse(base_source))
                    if isinstance(node, ast.Assign) and len(node.targets) == 1
                    and isinstance(node.targets[0], ast.Name)
                    and node.targets[0].id in {"HELPER_SHA256", "LIBRARY_SHA256"}}
if set(base_assignments) != {"HELPER_SHA256", "LIBRARY_SHA256"}:
    raise SystemExit("base caller helper/library pin assignments drift")
base_assignments["LIBRARY_SHA256"]["ObliquePen"] = "9b76bdb709db9f56c5d35a1a615fe41c57ceef73c559c91efaf5bf436f90e0ee"

# These short slices were extracted from the original page/object. They make
# the exact NewApplyBinary argument reconstructible while avoiding an archive
# dependency. The frame itself remains sourced from the independently pinned
# saved-color fixture above.
PAGE_HEADER_HEX = "e3000000800000000400000000047104000000000000380700009e1e0000"
OBJECT_PREFIX_HEX = "7900000000006900000002be010400600000a00f0000240062376365633337362d343665302d313165652d623966382d3837306332326361363334357e0e724c1d040600000000800fc58c4000000080926d9640000000e0c4f78c4000000000bd8c96400000000000fff472101b040600380700009e1e0000"
page_header = bytes.fromhex(PAGE_HEADER_HEX)
object_prefix = bytes.fromhex(OBJECT_PREFIX_HEX)
frame = bytes.fromhex(saved["frame_hex"])
if len(page_header) != 30:
    raise SystemExit("persisted page header width evidence is invalid")
saved_page_width = int.from_bytes(page_header[22:26], "little")
if saved_page_width != 1848:
    raise SystemExit("persisted page header width evidence is invalid")
if len(object_prefix) != 121 or len(frame) != 1998:
    raise SystemExit("persisted object prefix/frame length changed")
object_payload = object_prefix + frame
if sha(object_payload) != "f9666805d1390cfbb7606d6dbdb62bc0c95408e51174058faf1146bfbba66f83":
    raise SystemExit("reconstructed persisted object payload hash mismatch")
manager_hex = "020000000000330063006f006d002e00730061006d00730075006e0067002e0061006e00640072006f00690064002e00730064006b002e00700065006e002e00700065006e002e007000720065006c006f00610064002e0046006f0075006e007400610069006e00500065006e00010000000300310034003b00"
manager_bytes = bytes.fromhex(manager_hex)
if sha(manager_bytes) != "ecd7cefed7929088a0cb42d950e23e87e51e6c7f968c742f6c1ed10504a42df0":
    raise SystemExit("persisted StringID table hash mismatch")

# Adapt the already-pinned caller in memory with exact anchors: replace only
# its synthetic per-case ObjectStroke setters with the original native frame,
# expose the saved-width and 1920 control, and return the captured records to
# this wrapper. The underlying native caller/runtime code remains unchanged.
old_loop = "for label,mode,size,advanced_value in [(f'{v14}_{case}', mode, size, '14;') for v14 in ('v14',) for case,mode,size in [('omitted','omit',None),('zero','set',0.),('small','set',.1),('normal','set',10.)]] + [(f'v16_{case}',mode,size,'18;0;100;') for case,mode,size in [('omitted','omit',None),('zero','set',0.),('small','set',.1),('normal','set',10.)]]:\n"
if base_source.count(old_loop) != 1:
    raise SystemExit("base caller case-loop anchor drift")
adapted = base_source.replace(old_loop, "for label,profile_width,advanced_value in [('saved_page_width',saved_page_width,'14;'),('host_width_control',1920,'14;')]:\n", 1)
getter_loop = " for _field,_getter in [('points','SPen::ObjectStroke::GetPoint() const'),('pressure','SPen::ObjectStroke::GetPressure() const'),('timestamps','SPen::ObjectStroke::GetTimeStamp() const'),('tilt','SPen::ObjectStroke::GetTilt() const'),('orientation','SPen::ObjectStroke::GetOrientation() const')]:\n  call(_getter,[obj]); model_arrays[_field]=p.x(0)\n"
if adapted.count(getter_loop) != 1:
    raise SystemExit("base caller array-getter anchor drift")
adapted = adapted.replace(getter_loop, " # Native getters are captured below after applying the persisted frame.\n", 1)
start = " obj=p.alloc(256);impl=p.alloc(512);call('SPen::ObjectStroke::ObjectStroke()',[obj]);call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)',[impl,obj]);p.put(obj+32,'Q',impl)\n"
end = " call('SPen::ObjectStroke::GetPointCount() const',[obj]); model_count=p.x(0)\n"
if adapted.count(start) != 1 or adapted.count(end) != 1:
    raise SystemExit("base caller ObjectStroke setup anchors drift")
lo = adapted.index(start)
hi = adapted.index(end, lo) + len(end)
native_input = ''' # Apply the original serialized object and its original StringID table.
 rt.run_init(rt.MODEL,rt.BASE)
 _manager=p.alloc(8); call('SPen::StringIDManager::StringIDManager()',[_manager]); call('SPen::StringIDManager::Construct()',[_manager])
 _mb=p.alloc(len(manager_bytes)); p.u.mem_write(_mb,manager_bytes); call('SPen::StringIDManager::ApplyBinary(unsigned char*, int)',[_manager,_mb,len(manager_bytes)])
 p.callbacks['uuid_create']=lambda:p.x(0,0); p.callbacks['uuid_make']=lambda:p.x(0,0); p.callbacks['uuid_export']=lambda:p.x(0,0); p.callbacks['uuid_destroy']=lambda:p.x(0,0)
 _ctx=p.alloc(0x800); call('SPen::ModelContext::ModelContext()',[_ctx]); call('SPen::ModelContext::SetStringIDManager(SPen::StringIDManager*)',[_ctx,_manager])
 obj=p.alloc(256); call('SPen::ObjectStroke::ObjectStroke()',[obj]); call('SPen::ObjectStroke::Construct()',[obj]); impl=p.get(obj+32,'Q')[0]
 _payload=p.alloc(len(object_payload)); p.u.mem_write(_payload,object_payload)
 call('SPen::ObjectStroke::NewApplyBinary(unsigned char*, int, SPen::Orientation, int)',[obj,_payload,len(object_payload),0,profile_width],limit=5000000)
 _consumed=p.x(0)
 if _consumed!=len(object_payload): raise RuntimeError(f'NewApplyBinary consumed {_consumed}, expected {len(object_payload)}')
 call('SPen::ObjectStroke::OnAttach(SPen::ModelContext*, int)',[obj,_ctx,0])
 model_arrays={}
 for _field,_getter in [('points','SPen::ObjectStroke::GetPoint() const'),('pressure','SPen::ObjectStroke::GetPressure() const'),('timestamps','SPen::ObjectStroke::GetTimeStamp() const'),('tilt','SPen::ObjectStroke::GetTilt() const'),('orientation','SPen::ObjectStroke::GetOrientation() const')]:
  call(_getter,[obj]); model_arrays[_field]=p.x(0)
 call('SPen::ObjectStroke::GetPointCount() const',[obj]); model_count=p.x(0)
 _native_names={}
 for _nm in ('SPen::ObjectStroke::GetPenName() const','SPen::ObjectStroke::GetAdvancedPenSetting() const'):
  call(_nm,[obj]); _ptr=p.x(0); call('SPen::String::GetUTF8Size() const',[_ptr]); _n=p.x(0); _out=p.alloc(_n+1); call('SPen::String::GetUTF8(char*, int) const',[_ptr,_out,_n+1]); _native_names[_nm]=bytes(p.u.mem_read(_out,_n+1))[:_n].decode().rstrip('\\0')
 def _gi(_name):
  call(_name,[obj]); return p.x(0)&0xffffffff
 def _gf(_name):
  call(_name,[obj]); return struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex()
 _native_state={'color_argb':_gi('SPen::ObjectStroke::GetColor() const'),'curve_enabled':_gi('SPen::ObjectStroke::IsCurveEnabled() const'),'fixed_width_enabled':_gi('SPen::ObjectStroke::IsFixedWidthEnabled() const'),'fixed_width_bits':_gf('SPen::ObjectStroke::GetFixedWidth() const'),'size_bits':_gf('SPen::ObjectStroke::GetPenSize() const'),'stroke_type':_gi('SPen::ObjectStroke::GetStrokeType() const'),'tool_type':_gi('SPen::ObjectStroke::GetToolType() const')}
 advanced_value=_native_names['SPen::ObjectStroke::GetAdvancedPenSetting() const']
 mode='saved'; size=None
'''
adapted = adapted[:lo] + native_input + adapted[hi:]
if adapted.count("elif a==0x200f2380:p.x(0,1920)") != 1:
    raise SystemExit("base caller IContext width callback anchor drift")
adapted = adapted.replace("elif a==0x200f2380:p.x(0,1920)", "elif a==0x200f2380:p.x(0,profile_width)")
adapted = adapted.replace("[obj,_bp,len(_payload),0,1848]", "[obj,_bp,len(_payload),0,profile_width]")
if "[obj,_bp,len(_payload),0,1848]" in adapted:
    raise SystemExit("fixed width still present in native apply call")
row_append = " records.append({'case':label,'advanced_setting':advanced_value,"
if adapted.count(row_append) != 1:
    raise SystemExit("base caller record anchor drift")
adapted = adapted.replace(row_append, " records.append({'case':label,'profile_width':profile_width,'i_context_width':profile_width,'native_apply_consumed_bytes':_consumed,'native_pen_name':_native_names['SPen::ObjectStroke::GetPenName() const'],'native_state':_native_state,'advanced_setting':advanced_value,", 1)
short_arrays = "'points_f32_bits':[f'{v:08x}' for v in struct.unpack('<6I',bytes(p.u.mem_read(model_arrays['points'],24)))],'pressure_f32_bits':[f'{v:08x}' for v in struct.unpack('<3I',bytes(p.u.mem_read(model_arrays['pressure'],12)))],'timestamps_i32':list(struct.unpack('<3i',bytes(p.u.mem_read(model_arrays['timestamps'],12)))),'tilt_f32_bits':[f'{v:08x}' for v in struct.unpack('<3I',bytes(p.u.mem_read(model_arrays['tilt'],12)))],'orientation_f32_bits':[f'{v:08x}' for v in struct.unpack('<3I',bytes(p.u.mem_read(model_arrays['orientation'],12))) ]"
full_arrays = "'points_f32_bits':[f'{v:08x}' for v in struct.unpack('<%dI'%(model_count*2),bytes(p.u.mem_read(model_arrays['points'],model_count*8)))],'pressure_f32_bits':[f'{v:08x}' for v in struct.unpack('<%dI'%model_count,bytes(p.u.mem_read(model_arrays['pressure'],model_count*4)))],'timestamps_i32':list(struct.unpack('<%di'%model_count,bytes(p.u.mem_read(model_arrays['timestamps'],model_count*4)))),'tilt_f32_bits':[f'{v:08x}' for v in struct.unpack('<%dI'%model_count,bytes(p.u.mem_read(model_arrays['tilt'],model_count*4)))],'orientation_f32_bits':[f'{v:08x}' for v in struct.unpack('<%dI'%model_count,bytes(p.u.mem_read(model_arrays['orientation'],model_count*4))) ]"
if adapted.count(short_arrays) != 1:
    raise SystemExit("base caller short getter-array anchor drift")
adapted = adapted.replace(short_arrays, full_arrays, 1)
capture_anchor = "for _row in records:\n"
if adapted.count(capture_anchor) != 1:
    raise SystemExit("base caller post-capture anchor drift")
adapted = adapted[:adapted.index(capture_anchor)] + "CAPTURED_RECORDS=records\n"
adapted = adapted.replace("FIXTURE=ROOT/'conformance/fountain-saved-caller-native.json'", "FIXTURE=ROOT/'tmp-fountain-persisted-unused.json'")

old_argv = sys.argv
sys.argv = [str(BASE_CALLER)]
namespace = {"__name__": "__main__", "__file__": str(BASE_CALLER),
             "manager_bytes": manager_bytes, "object_payload": object_payload,
             "saved_page_width": saved_page_width}
try:
    exec(compile(adapted, str(BASE_CALLER), "exec"), namespace)
finally:
    sys.argv = old_argv
records = namespace["CAPTURED_RECORDS"]

def arrays_from_record(row):
    points = row["source_arrays"]["points_f32_bits"]
    pressure = row["source_arrays"]["pressure_f32_bits"]
    timestamps = row["source_arrays"]["timestamps_i32"]
    tilt = row["source_arrays"]["tilt_f32_bits"]
    orientation = row["source_arrays"]["orientation_f32_bits"]
    return {"points_f32_bits": points, "pressure_f32_bits": pressure,
            "timestamps_i32": timestamps, "tilt_f32_bits": tilt,
            "orientation_f32_bits": orientation}

cases = []
for row in records:
    expected_width = saved_page_width if row["case"] == "saved_page_width" else 1920
    if row["profile_width"] != expected_width or row["native_apply_consumed_bytes"] != len(object_payload):
        raise SystemExit(f"persisted apply/profile mismatch: {row['case']}")
    if row["native_pen_name"] != "com.samsung.android.sdk.pen.pen.preload.FountainPen" or row["advanced_setting"] != "14;":
        raise SystemExit(f"native saved style getters mismatch: {row['case']} name={row['native_pen_name']!r} advanced={row['advanced_setting']!r}")
    expected_state = {key: saved["native_state"][key] for key in (
        "color_argb", "curve_enabled", "fixed_width_bits", "fixed_width_enabled",
        "size_bits", "stroke_type", "tool_type")}
    if expected_width == saved_page_width and row["native_state"] != {key: expected_state[key] for key in (
            "color_argb", "curve_enabled", "fixed_width_bits", "fixed_width_enabled",
            "size_bits", "stroke_type", "tool_type")}:
        raise SystemExit(f"native applied ObjectStroke getters differ from saved-frame oracle: {row['case']} got={row['native_state']} expected={expected_state}")
    expected_callbacks = 24 if expected_width == 1848 else 25
    if row["source_model_get_point_count"] != 161 or len(row["rtv_addpoint_entries"]) != expected_callbacks:
        raise SystemExit(f"native Drawing count mismatch: {row['case']}")
    drawable_class = "GLV14" if len(row["redraw_entries"]) == 1 and "GLV14" in row["redraw_entries"][0]["entry"] else None
    if drawable_class != "GLV14":
        raise SystemExit(f"wrong native V14 Drawing route: {row['case']}")
    if any(len(item["float_words_s0_s5"]) != 6 for item in row["rtv_addpoint_entries"]):
        raise SystemExit("incomplete Drawing callback record")
    if len(row["source_arrays"]["points_f32_bits"]) != 322 or any(
            len(row["source_arrays"][key]) != 161 for key in (
                "pressure_f32_bits", "timestamps_i32", "tilt_f32_bits", "orientation_f32_bits")):
        raise SystemExit(f"native ObjectStroke array getter lengths mismatch: {row['case']}")
    cases.append({
        "id": row["case"], "explicit_new_apply_width": row["profile_width"],
        "i_context_width": row["i_context_width"],
        "native_apply_consumed_bytes": row["native_apply_consumed_bytes"],
        "native_pen_name": row["native_pen_name"],
        "native_advanced_setting": row["advanced_setting"],
        "native_point_count": row["source_model_get_point_count"],
        "native_state_after_apply": row["native_state"],
        "source_getter_arrays": arrays_from_record(row),
        "drawing_return": row["drawing_return"],
        "selected_drawable_class": drawable_class,
        "redraw_entries": row["redraw_entries"],
        "drawing_addpoint_records": row["rtv_addpoint_entries"],
        "loader_calls": row["loader_calls"],
    })

value = {
    "schema": "sdocx-fountain-persisted-drawing-native-v1",
    "provenance": {
        "source_fixture": {"path": "conformance/fountain-saved-color-native.json", "sha256": SOURCE_FIXTURE_SHA256,
                           "case": saved["id"], "frame_sha256": saved["frame_sha256"]},
        "base_caller": {"path": "conformance/fountain_saved_caller_native.py", "sha256": BASE_CALLER_SHA256},
        "runner_sha256": RUNNER_SHA256,
        "runtime": {"python": python_runtime, "unicorn": unicorn_runtime},
        "native_closure": {"pinned_helpers": base_assignments["HELPER_SHA256"],
                            "mapped_dsos": base_assignments["LIBRARY_SHA256"],
                            "mapped_oblique_pen_sha256": "9b76bdb709db9f56c5d35a1a615fe41c57ceef73c559c91efaf5bf436f90e0ee",
                            "executed_tools": executed_tools},
        "page_header": {"source_archive_sha256": saved["source_sha256"], "page_entry_sha256": saved["page_sha256"],
                        "first_30_bytes_hex": PAGE_HEADER_HEX, "width_offset": 22, "width_bytes_hex": "38070000", "width": saved_page_width},
        "object_payload": {"prefix_hex": OBJECT_PREFIX_HEX, "prefix_length": len(object_prefix),
                           "frame_length": len(frame), "length": len(object_payload), "sha256": sha(object_payload),
                           "new_apply_width_source": "u32 at page-header byte offset 22"},
        "string_id_manager": {"bytes_hex": manager_bytes.hex(), "length": len(manager_bytes), "sha256": sha(manager_bytes),
                              "entries": ["0=com.samsung.android.sdk.pen.pen.preload.FountainPen", "1=14;"],
                              "source": "Original 122-byte table extracted from the same persisted page; saved-color fixture retains the frame but not this table."},
        "boundary": "Native ObjectStroke NewApplyBinary receives the reconstructed original object payload and StringID table, then native ObjectDrawing::drawObjectStroke executes GLV14 RedrawPen and captures RTV4 AddPoint callback arguments. This fixture stops at the complete six-float callback stream: it does not execute RTV4 SendDataToGPU/Update/Draw. The 1920 control changes both the explicit NewApply width and IContext width, so it is a combined control, not an isolated argument-only comparison. No Samsung SDK outer page-loader or device raster claim.",
        "host_adapters": "Pinned caller adapters provide IContext/CanvasProbe, task queue dispatch, allocator, and synthetic dlopen/dlsym handles for the already pinned FountainPen and DefaultPen DSOs.",
    },
    "saved_object_state": {"point_count": 161, "size_f32_bits": saved["native_state"]["size_bits"],
                           "color_argb": saved["native_state"]["color_argb"], "stroke_type": saved["native_state"]["stroke_type"],
                           "tool_type": saved["native_state"]["tool_type"], "curve_enabled": saved["native_state"]["curve_enabled"]},
    "cases": cases,
}

encoded = json.dumps(value, indent=2, sort_keys=True) + "\n"
if args.write_fixture:
    FIXTURE.write_text(encoded)
    print(f"wrote {FIXTURE.relative_to(ROOT)} ({len(cases)} cases)")
else:
    if not FIXTURE.exists() or FIXTURE.read_text() != encoded:
        raise SystemExit("persisted Fountain Drawing fixture mismatch; use --write-fixture explicitly")
    print(f"verified {len(cases)} persisted Fountain Drawing cases")
