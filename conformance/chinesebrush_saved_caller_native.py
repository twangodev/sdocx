"""Replayable ChineseBrush saved-caller matrix using a pinned BrushPen adapter."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CALLER = ROOT / "conformance/brushpen_saved_caller_native.py"
CALLER_SHA256 = "66f4790e8eeb1ca867f3273a796303b6ffb108286916e1e245fc063cbd744ae0"
OBLIQUE_HELPER_SHA256 = "1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2"
ELF_PLT_SHA256 = "816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b"
FOUNTAIN_HELPER_SHA256 = "40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30"
CHINESE_SHA256 = "25d06b71e525f129e53c3734b957cdc9279ff9bd8dbca19948351c5e5ad237f5"
FIXTURE = ROOT / "conformance/chinesebrush-saved-caller-native.json"
CASES = [("omitted", None, 1, "V1"), ("empty", "", 3, "V3"), ("zero", "0;", 1, "V1"), ("one", "1;", 1, "V1"), ("two", "2;", 2, "V2"), ("three", "3;", 3, "V3"), ("four", "4;", 4, "V3")]
LIBRARY_PINS = {"Base":"e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb", "ChineseBrush":CHINESE_SHA256, "Composer":"52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f", "Drawing":"788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd", "FountainPen":"1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec", "Graphics":"aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621", "Model":"4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a", "PenCommon":"afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d", "libc++_shared.so":"4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4"}
HELPER_PINS = {"oblique_canvas_native.py":OBLIQUE_HELPER_SHA256,"elf_plt.py":ELF_PLT_SHA256,"fountain_native.py":FOUNTAIN_HELPER_SHA256}
NATIVE_HELPER_PINS = {"oblique_canvas_native":OBLIQUE_HELPER_SHA256,"elf_plt":ELF_PLT_SHA256,"fountain_native":FOUNTAIN_HELPER_SHA256}


def _sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def _run_matrix():
    parser = __import__("argparse").ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--write-fixture", action="store_true", help="explicitly write or refresh the native fixture")
    mode.add_argument("--check-fixture", action="store_true", help="check all seven native captures (the default)")
    args = parser.parse_args()
    captures = {}
    for name, setting, _, _ in CASES:
        argv = [sys.executable, str(Path(__file__).resolve()), "--_capture", name]
        result = subprocess.run(argv, cwd=ROOT, text=True, capture_output=True, check=True)
        try:
            captures[name] = json.loads(result.stdout)
            captures[name]["native_helper_sha256"]["fountain_native"] = FOUNTAIN_HELPER_SHA256
        except json.JSONDecodeError as error:
            raise RuntimeError(f"capture {name} did not produce one JSON document: {result.stdout[:300]!r} {result.stderr[:300]!r}") from error
    rows = []
    source_arrays = None
    for name, setting, version, drawable in CASES:
        value = captures[name]
        assert value["input"]["advanced_setting_ascii"] == setting, (name, "setting")
        assert value["native_selection"]["version"] == version, (name, "version")
        targets = value["native_selection"]["redraw_target_symbols"]
        assert len(targets) == 1 and f"ChineseBrushStrokeDrawableGL{drawable}::RedrawPen" in targets[0], (name, targets)
        assert value["input"]["native_model_getter_snapshot"]["pen_name"].endswith(".ChineseBrush")
        assert value["input"]["native_model_getter_snapshot"]["tool_type"] == 2
        producer = value["producer"]
        stream = bytes.fromhex(producer["buffer_hex"])
        assert producer["callback_count"] == 5 and producer["instance_float_count"] == 20 and len(stream) == 80
        assert hashlib.sha256(stream).hexdigest() == producer["buffer_sha256"]
        assert len(producer["append_rows"]) == 5 and all(len(row["float_args"]) == 5 for row in producer["append_rows"])
        arrays = value["input"]["source_arrays"]
        assert all(row["count"] == 3 for row in arrays.values())
        if source_arrays is None:
            source_arrays = arrays
        else:
            assert source_arrays == arrays
        assert value["native_libraries_sha256"] == LIBRARY_PINS, (name, "mapped DSO pins")
        assert value["native_helper_sha256"] == NATIVE_HELPER_PINS, (name, "helper pins")
        rows.append({"case":name,"setting":setting,"getVersion":version,"selected_drawable":drawable,"addpoint_count":5,"f32_word_count":20,"stream_sha256":producer["buffer_sha256"],"capture_sha256":hashlib.sha256(json.dumps(value,indent=2,sort_keys=True).encode()+b"\n").hexdigest()})
    manifest = {"schema":"chinese-brush-native-drawing-matrix-v1", "provenance":{"runner_sha256":_sha(Path(__file__)),"canonical_caller_sha256":CALLER_SHA256,"canonical_caller":"brushpen_saved_caller_native.py","helper_source_sha256":HELPER_PINS,"native_libraries_sha256":LIBRARY_PINS}, "cases":captures, "case_index":rows, "source_arrays":source_arrays, "boundary":"Synthetic Model ObjectStroke source tuple; native Model getters, ObjectDrawing, ChineseBrush factory/version dispatch, GL redraw and RTV1 AddPoint execute in pinned DSOs. Renderer/GLES/PDF/device execution is outside this probe."}
    encoded = json.dumps(manifest, indent=2, sort_keys=True) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)} ({len(captures)} cases)")
    elif not FIXTURE.exists() or json.loads(FIXTURE.read_text()) != manifest:
        raise SystemExit("ChineseBrush saved-caller fixture mismatch; use --write-fixture to intentionally refresh")
    else:
        print(f"verified {len(captures)} ChineseBrush saved-caller cases")

if "--_capture" not in sys.argv:
    _run_matrix()
    raise SystemExit(0)

_capture_index = sys.argv.index("--_capture")
if _capture_index + 1 >= len(sys.argv):
    raise SystemExit("--_capture requires a case name")
_capture_name = sys.argv[_capture_index + 1]
_capture_case = next((row for row in CASES if row[0] == _capture_name), None)
if _capture_case is None:
    raise SystemExit(f"unknown capture case: {_capture_name}")
_capture_setting = _capture_case[1]
# Run the transformed canonical script with only its native capture mode.
sys.argv = [str(CALLER), "--print-capture"] + (["--omit-advanced-setting"] if _capture_setting is None else ["--advanced-setting", _capture_setting])

def replace_once(source: str, old: str, new: str, *, count: int = 1) -> str:
    actual = source.count(old)
    if actual != count:
        raise RuntimeError(f"patch anchor count {actual}, expected {count}: {old[:100]!r}")
    return source.replace(old, new, count)


def remove_between(source: str, begin: str, end: str) -> str:
    if source.count(begin) != 1 or source.count(end) != 1:
        raise RuntimeError(f"nonunique patch range: {begin!r}, {end!r}")
    left = source.index(begin)
    right = source.index(end, left)
    return source[:left] + source[right:]


if hashlib.sha256(CALLER.read_bytes()).hexdigest() != CALLER_SHA256:
    raise SystemExit("pinned canonical BrushPen caller changed")
if hashlib.sha256((ROOT / "conformance/oblique_canvas_native.py").read_bytes()).hexdigest() != OBLIQUE_HELPER_SHA256:
    raise SystemExit("pinned oblique canvas helper changed")
if hashlib.sha256((ROOT / "conformance/elf_plt.py").read_bytes()).hexdigest() != ELF_PLT_SHA256:
    raise SystemExit("pinned PLT helper changed")
if hashlib.sha256((ROOT / "conformance/fountain_native.py").read_bytes()).hexdigest() != FOUNTAIN_HELPER_SHA256:
    raise SystemExit("pinned Fountain helper changed")

source = CALLER.read_text()
source = source.replace("BrushPen", "ChineseBrush")
source = source.replace("brushpen", "chinesebrush")
source = replace_once(source, "acf59300dbdf53942680761a6cfe57e5164e0e6c45076d30fc56b9efa04ea0ce", CHINESE_SHA256)
source = replace_once(source, "FIXTURE=ROOT/'conformance/chinesebrush-saved-caller-native.json'", "FIXTURE=None")
source = replace_once(source,
    "parser.add_argument('--advanced-setting',default='',help='explicit synthetic ObjectStroke advanced setting (empty selects native GLV10 in this capture)')",
    "parser.add_argument('--advanced-setting',default='',help='explicit synthetic ObjectStroke advanced setting')\nparser.add_argument('--omit-advanced-setting',action='store_true',help='do not call ObjectStroke::SetAdvancedPenSetting')")
source = replace_once(source, 'args=parser.parse_args()', "args=parser.parse_args()\nsetting_key='omitted' if args.omit_advanced_setting else ('empty' if args.advanced_setting == '' else {'0;':'zero','1;':'one','2;':'two','3;':'three','4;':'four'}.get(args.advanced_setting,args.advanced_setting))\nFIXTURE=None")

# A null setting is a distinct native input. Avoid calling the native getter for
# that state because it dereferences an uninitialized ObjectStroke String.
source = replace_once(source,
    "p.call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[obj,adv]); p.call('SPen::ObjectStroke::GetAdvancedPenSetting() const',[obj]); adv_read=p.x(0);",
    "(None if args.omit_advanced_setting else p.call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[obj,adv])); adv_read=(adv if args.omit_advanced_setting else (p.call('SPen::ObjectStroke::GetAdvancedPenSetting() const',[obj]) or p.x(0)));", count=1)
source = replace_once(source, "'advanced_setting_ascii':args.advanced_setting", "'advanced_setting_ascii':None if args.omit_advanced_setting else args.advanced_setting")
source = replace_once(source,
    "'advanced_setting':bytes(p.u.mem_read(adv_buf,adv_len+1)).split(b'\\0',1)[0].decode('ascii')",
    "'advanced_setting':None if args.omit_advanced_setting else bytes(p.u.mem_read(adv_buf,adv_len+1)).split(b'\\0',1)[0].decode('ascii'),'advanced_setting_getter_called':not args.omit_advanced_setting")
source = replace_once(source,
    "p.call('SPen::ObjectStroke::GetAdvancedPenSetting() const',[obj]); advanced_getter=p.x(0);",
    "advanced_getter=(0 if args.omit_advanced_setting else (p.call('SPen::ObjectStroke::GetAdvancedPenSetting() const',[obj]) or p.x(0)));", count=1)

# The copied runtime-normalization hook belongs to BrushPen. The ChineseBrush
# probe retains the actual Draw/AddPoint trace and omits those unrelated offsets.
source = remove_between(source, "def log_runtime_normalization(u,a,z,d):", "def log_vcall(u,a,z,d):")
source = replace_once(source, "p.u.hook_add(UC_HOOK_CODE,log_runtime_normalization)", "# ChineseBrush does not use BrushPen normalization instrumentation")
source = remove_between(source, " if len(runtime_normalization_events) != 2", " if len(constructor_settings_snapshots) != 1")
source = remove_between(source, " for row in runtime_normalization_events:", " make_entries=[row for row in native_draw_entries")
source = remove_between(source, " make_entries=[row for row in native_draw_entries", " if screen_resolution_calls !=")
source = replace_once(source, " if selected_version != 10 or not any('GLV10::RedrawPen' in name for name in redraw_names):\n  raise RuntimeError(f'empty advanced setting did not select ChineseBrush GLV10: {selected_version}, {redraw_names}')",
    " expected_gl_version=min(max(selected_version,1),3)\n if not any(f'ChineseBrushStrokeDrawableGLV{expected_gl_version}::RedrawPen' in name for name in redraw_names):\n  raise RuntimeError(f'native ChineseBrush version dispatch mismatch: {selected_version}, {redraw_names}')")
source = replace_once(source, "print('after setcanvas fields',[hex(struct.unpack('<Q',p.u.mem_read(d+i,8))[0]) for i in range(0,128,8)])", "print('after setcanvas fields',hex(d))")
source = replace_once(source, "'drawable_fields',[hex(struct.unpack('<Q',p.u.mem_read(d+i,8))[0]) for i in range(0,128,8)]", "'drawable',hex(d)")
source = replace_once(source, "snapshot['alternation_plus_100_u8']=u.mem_read(this+100,1)[0]", "snapshot['drawable_offset_100_u8']=u.mem_read(this+100,1)[0]")
source = replace_once(source, "('tolerance_plus_44_f32_bits',44),('spacing_plus_88_f32_bits',88),('residual_plus_92_f32_bits',92),('tool_threshold_plus_96_f32_bits',96)", "('drawable_offset_44_u32_le',44),('drawable_offset_88_u32_le',88),('drawable_offset_92_u32_le',92),('drawable_offset_96_u32_le',96)")
source = replace_once(source, "record['drawline_plus44_threshold_f32_bits']=bytes(u.mem_read(this+44,4)).hex()", "record['drawline_offset_44_bytes']=bytes(u.mem_read(this+44,4)).hex()")
source = replace_once(source, "record['drawline_plus96_tool_threshold_f32_bits']=bytes(u.mem_read(this+96,4)).hex()", "record['drawline_offset_96_bytes']=bytes(u.mem_read(this+96,4)).hex()")
source = replace_once(source, "'plus44_tolerance_f32_bits':record['drawline_plus44_threshold_f32_bits'],'plus96_tool_threshold_f32_bits':record['drawline_plus96_tool_threshold_f32_bits']", "'offset_44_bytes':record['drawline_offset_44_bytes'],'offset_96_bytes':record['drawline_offset_96_bytes']")
source = replace_once(source, "if any(name.startswith('SPen::ChineseBrushStrokeDrawableGL::RedrawPen(SPen::ObjectStroke const*') for name in method_trace_names_by_pc[a]):", "if any('SPen::ChineseBrushStrokeDrawableGLV' in name and 'RedrawPen(SPen::ObjectStroke const*' in name for name in method_trace_names_by_pc[a]):")
source = replace_once(source, "'limits_plus32_plus36_i32':list(limits),'limits_bytes':u.mem_read(settings_ptr+32,8).hex()", "'pen_object_plus32_plus36_i32':list(limits),'pen_object_plus32_plus36_bytes':u.mem_read(settings_ptr+32,8).hex()")
source = replace_once(source, "constructor_settings_snapshots[0]['limits_plus32_plus36_i32']", "constructor_settings_snapshots[0]['pen_object_plus32_plus36_i32']")
source = replace_once(source, "'float_args':[hex(u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))) for i in range(4)]", "'float_args':[hex(u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))) for i in range(5)]")
source = replace_once(source, " if len(stream) != 1144 or len(chinesebrush_buffer_events) != 26:\n  raise RuntimeError(f'unexpected Drawing output size/callback count: {len(stream)}, {len(chinesebrush_buffer_events)}')",
    " if len(stream) != 80 or len(chinesebrush_buffer_events) != 5:\n  raise RuntimeError(f'unexpected ChineseBrush Drawing output size/callback count: {len(stream)}, {len(chinesebrush_buffer_events)}')")
source = replace_once(source,
    "'runtime_normalization_contract':{'library':'libSPenChineseBrush.so','function_rva':'0x5c2f4','settings_pointer_load_rva':'0x5c428','limits_load_signed_pair_rva':'0x5c430','signed_min_csel_rva':'0x5c43c','speed_elapsed_division_rva':'0x5c440','zero_minimum_fallback_branch_rva':'0x5c44c','zero_minimum_factor_one_rva':'0x5c464','1440_over_min_division_rva':'0x5c45c','normalization_multiply_rva':'0x5c468','post_clamp_makeDrawPoint_s3_copy_rva':'0x5c630','zero_minimum_runtime_branch_exercised':False},'runtime_normalization_events':runtime_normalization_events,",
    "")
source = replace_once(source, "'case_id':'chinesebrush-empty-advanced-setting-tool2'", "'case_id':f'chinesebrush-{setting_key}-tool2'")
source = replace_once(source,
    "'Pen/model input arrays and IContext/canvas/resource/queue are explicit synthetic host inputs; native Model getters, ObjectDrawing route, ChineseBrush factory/admission, MotionEvent getter path, GLV10 geometry and RTV1 AddPoint execute in the pinned native DSOs.'",
    "'Native ObjectStroke input arrays and IContext/canvas/resource/queue are synthetic host inputs; native Model getters, ObjectDrawing route, ChineseBrush factory/version selection, MotionEvent getter path, selected GLV1/2/3 redraw and RTV1 AddPoint execute in pinned DSOs.'")
source = replace_once(source,
    "'The empty advanced setting is explicitly supplied to native ObjectStroke::SetAdvancedPenSetting; this is not an SDK-deserialized app stroke. No Samsung device or user-captured stroke is claimed.'",
    "('No advanced-setting setter was called for the omitted case.' if args.omit_advanced_setting else 'The supplied advanced setting is an explicit synthetic ObjectStroke input.') + ' This is not an SDK-deserialized app stroke; no Samsung device or user-captured stroke is claimed.'")

# Check this patch never smuggles checkout-specific paths into executed bytes.
if str(ROOT) in source or str(CALLER) in source:
    raise SystemExit("transformed caller unexpectedly embeds an absolute checkout path")
namespace = {"__name__": "__main__", "__file__": str(CALLER), "__package__": None}
exec(compile(source, str(CALLER), "exec"), namespace)
