#!/usr/bin/env python3
"""Native Fountain V14 saved-size and tolerance contract oracle.

The pinned Fountain saved-caller runner is executed in memory with narrowly
checked source substitutions. It is not modified; its native Drawing path,
runtime adapters, library pins, and six-lane RTV4 observations are reused.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import sys
import struct

ROOT = Path(__file__).resolve().parents[1]
CALLER = ROOT / "conformance/fountain_saved_caller_native.py"
FIXTURE = ROOT / "conformance/fountain-v14-defaults-native.json"
RUNNER_SHA256 = "b0837c86483aa438923f9d4d1c6451b1ebc1c5fec4c5f7c447b928a25a8f7b19"
CORPUS_FIXTURE = ROOT / "conformance/fountain-v14-real-native.json"
CORPUS_SHA256 = "c6204b33441eb77a53932e1c475e931e0f9688a8ad40788d05a7f41c58400ca3"
SERIALIZED_FRAME_FIXTURE = ROOT / "conformance/fountain-saved-color-native.json"
SERIALIZED_FRAME_SHA256 = "de23cf21f239fc9e12794bc55777fa18406d1129162f8198b1544b8f009f932d"
MODE = argparse.ArgumentParser(description=__doc__)
MODE.add_argument("--write-fixture", action="store_true")
MODE.add_argument("--check-fixture", action="store_true")
ARGS = MODE.parse_args()
if ARGS.write_fixture and ARGS.check_fixture:
    MODE.error("choose --write-fixture or --check-fixture")
if hashlib.sha256(CALLER.read_bytes()).hexdigest() != RUNNER_SHA256:
    raise SystemExit("pinned Fountain saved-caller runner changed")
if hashlib.sha256(CORPUS_FIXTURE.read_bytes()).hexdigest() != CORPUS_SHA256:
    raise SystemExit("pinned normalized real-stroke fixture changed")
corpus_stroke = json.loads(CORPUS_FIXTURE.read_text())["source_stroke"]
corpus_identity = json.loads(CORPUS_FIXTURE.read_text())["source_identity"]
corpus_style = corpus_stroke["rendering"]["style"]
if hashlib.sha256(SERIALIZED_FRAME_FIXTURE.read_bytes()).hexdigest() != SERIALIZED_FRAME_SHA256:
    raise SystemExit("pinned serialized ObjectStroke native-state fixture changed")
serialized_frames = json.loads(SERIALIZED_FRAME_FIXTURE.read_text())["real_serialized_frames"]
v14_serialized_frame = next(frame for frame in serialized_frames if frame["id"] == "real_v14_handwritten_flat1445")
if v14_serialized_frame["native_state"]["size_bits"] != "1006be40":
    raise SystemExit("real serialized V14 ObjectStroke size getter evidence changed")

source = CALLER.read_text()
patches = []

def replace_once(old, new, label):
    global source
    n = source.count(old)
    if n != 1:
        raise SystemExit(f"patch anchor {label!r}: expected 1 occurrence, got {n}")
    source = source.replace(old, new, 1)
    patches.append({"anchor": label, "count": n, "old_sha256": hashlib.sha256(old.encode()).hexdigest(), "new_sha256": hashlib.sha256(new.encode()).hexdigest()})

replace_once(
    "MODE_ARGS=MODE_PARSER.parse_args()",
    "MODE_ARGS=MODE_PARSER.parse_args(['--write-fixture'])",
    "caller-argparse",
)
replace_once(
    "for label,mode,size,advanced_value in [(f'{v14}_{case}', mode, size, '14;') for v14 in ('v14',) for case,mode,size in [('omitted','omit',None),('zero','set',0.),('small','set',.1),('normal','set',10.)]] + [(f'v16_{case}',mode,size,'18;0;100;') for case,mode,size in [('omitted','omit',None),('zero','set',0.),('small','set',.1),('normal','set',10.)]]:",
    "for label,mode,size,advanced_value,tolerance,public_width in [('size_omitted_tol_omitted','omit',None,'14;',None,99.),('size_zero_tol_omitted','set',0.,'14;',None,99.),('size_subminimum_tol_native','set',.1,'14;',0.2549668848514557,99.),('size_oversized_tol_omitted','set',900.,'14;',None,99.),('size_corpus_numeric_width_tol_omitted','set',5.938240051269531,'14;',None,5.938240051269531),('size_corpus_style_pair','set',5.938240051269531,'14;',0.2549668848514557,5.938240051269531)]:",
    "v14-case-matrix",
)
replace_once(
    "p.put(pts,'6f',0,0,10,0,20,2);p.put(prs,'3f',.3,.6,1.);",
    "p.put(pts,'6f',0,0,.1,0,.2,0);p.put(prs,'3f',.3,.6,1.);",
    "close-spaced-source-points",
)
replace_once(
    " call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj,pts,prs,ts,tilt,ori,3])\n model_arrays={}",
 " call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj,pts,prs,ts,tilt,ori,3])\n tolerance_bits=None; tolerance_set_return=None\n if tolerance is not None:\n  tolerance_bits=struct.pack('<f',tolerance).hex()\n  tolerance_set_return=call('SPen::ObjectStroke::SetInitialToloerance(float)',[obj],[tolerance])\n else:\n  tolerance_bits='00000000'\n call('SPen::ObjectStroke::GetInitialToloerance() const',[obj]); tolerance_get_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex()\n setting_data=p.get(obj+32,'Q')[0]\n setting_data_bits=struct.pack('<I',p.get(setting_data+356,'I')[0]).hex() if setting_data else None\n model_arrays={}",
    "tolerance-set-getter-capture",
)
replace_once(
    " redraw_names=['SPen::FountainPenStrokeDrawableGLV14::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)','SPen::FountainPenStrokeDrawableGLV16::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)']",
    " redraw_names=['SPen::FountainPenStrokeDrawableGLV14::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)','SPen::FountainPenStrokeDrawableGLV16::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)']\n tolerance_trace=[]; tolerance_getter_addr=p.syms['SPen::ObjectStroke::GetInitialToloerance() const']; tolerance_store_addr=fpbase+0x72a7c",
    "native-tolerance-trace-addresses",
)
replace_once(
    "  elif a in redraw_addrs:redraws.append({'entry':redraw_addrs[a],'drawable':hex(p.x(0)),'stroke':hex(p.x(1))})",
    "  elif a==tolerance_getter_addr:tolerance_trace.append({'event':'ObjectStroke::GetInitialToloerance','phase':'entry','this':hex(p.x(0)),'lr':hex(u.reg_read(R.UC_ARM64_REG_LR))})\n  elif a==tolerance_getter_addr+0xc:tolerance_trace.append({'event':'ObjectStroke::GetInitialToloerance','phase':'return','result_s0':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)).hex(),'lr':hex(u.reg_read(R.UC_ARM64_REG_LR))})\n  elif a==tolerance_store_addr:\n   _drawable=p.x(19); tolerance_trace.append({'event':'GLV14 RedrawPen post-tolerance store','drawable':hex(_drawable),'drawable_plus44_f32_bits':struct.pack('<I',p.get(_drawable+44,'I')[0]).hex(),'lr':hex(u.reg_read(R.UC_ARM64_REG_LR))})\n  elif a in redraw_addrs:redraws.append({'entry':redraw_addrs[a],'drawable':hex(p.x(0)),'stroke':hex(p.x(1))})",
    "getter-return-store-observation",
)
replace_once(
    "records.append({'case':label,",
    "records.append({'case':label,'adapter_public_pen_width_metadata_f32_bits':struct.pack('<I',struct.unpack('<I',struct.pack('<f',public_width))[0]).hex(),'adapter_public_pen_width_is_native_input':False,'style_saved_size_setter':mode,'style_saved_size_input_f32_bits':None if size is None else struct.pack('<f',size).hex(),'style_initial_tolerance_setter':'omit' if tolerance is None else 'set','style_initial_tolerance_input_f32_bits':tolerance_bits,'model_set_initial_tolerance_return_w0':tolerance_set_return,'model_get_initial_tolerance_f32_bits':tolerance_get_bits,'model_setting_data_plus356_f32_bits':setting_data_bits,'tolerance_trace':tolerance_trace,",
    "per-case-tolerance-fields",
)
# Discard only the original fixture's case-specific assertions/output tail; the
# observations above are unchanged native records from the pinned caller.
tail_start = source.index("for _row in records:\n")
source = source[:tail_start] + """
value={'profile':[1920,1080],'corpus_reference':{'fixture':'conformance/fountain-v14-real-native.json','case_id':'real_v14_flat1445_global_grid','source_identity':corpus_identity,'rendering_style':corpus_style,'style_has_pen_size':'pen_size' in corpus_style,'capture_boundary':'This retained numeric native-output fixture does not encode ObjectStroke construction or the Model::SetPenSize call. It therefore does not prove which serialized field populates ObjectStroke pen size.'},'source_input':{'pen_name':'com.samsung.android.sdk.pen.pen.preload.FountainPen','points':[[0,0],[0.1,0],[0.2,0]],'pressure':[0.3,0.6,1.0],'timestamps':[100,110,120],'tilt':[0,0,0],'orientation':[0,0,0],'tool_type':2,'stroke_type':0,'curve_enabled':True,'visible':True,'public_pen_width_context':'For the corpus-sized case this equals source_identity.pen_width from fountain-v14-real-native.json. The other rows intentionally carry public adapter metadata 99.0 while native Model ObjectStroke.SetPenSize receives the per-row saved style value; there is no native public pen_width setter in this Drawing caller.'},'cases':records}
print('TOLERANCE_CAPTURE_JSON:'+json.dumps(value,sort_keys=True,separators=(',',':')))
"""

class CaptureWriter:
    def __init__(self): self.lines=[]
    def write(self, value): self.lines.append(value)
    def flush(self): pass

writer=CaptureWriter()
old_stdout=sys.stdout
sys.stdout=writer
try:
    os.chdir(ROOT)
    namespace={'__file__':str(CALLER),'__name__':'__main__','corpus_identity':corpus_identity,'corpus_style':corpus_style}
    exec(compile(source,str(CALLER),'exec'),namespace,namespace)
finally:
    sys.stdout=old_stdout
text=''.join(writer.lines)
prefix='TOLERANCE_CAPTURE_JSON:'
payload=[line[len(prefix):] for line in text.splitlines() if line.startswith(prefix)]
if len(payload)!=1: raise SystemExit('native wrapper did not emit exactly one capture payload')
captured=json.loads(payload[0])

expected_names=['size_omitted_tol_omitted','size_zero_tol_omitted','size_subminimum_tol_native','size_oversized_tol_omitted','size_corpus_numeric_width_tol_omitted','size_corpus_style_pair']
if [row['case'] for row in captured['cases']]!=expected_names: raise SystemExit('unexpected native case set/order')
for row in captured['cases']:
    expected_tolerance = row['style_initial_tolerance_input_f32_bits']
    if row['model_get_initial_tolerance_f32_bits'] != expected_tolerance:
        raise SystemExit(f"Model tolerance differs from the requested/default field: {row['case']}")
    if row['style_initial_tolerance_setter'] == 'set' and row['model_set_initial_tolerance_return_w0'] != 1:
        raise SystemExit(f"native tolerance setter failed: {row['case']}")
    expected_model_size = {'size_omitted_tol_omitted':'00000000','size_zero_tol_omitted':'00000000','size_subminimum_tol_native':'cdcccc3d','size_oversized_tol_omitted':'00006144','size_corpus_numeric_width_tol_omitted':'1006be40','size_corpus_style_pair':'1006be40'}[row['case']]
    expected_pen_size = {'size_omitted_tol_omitted':'cdcccc3e','size_zero_tol_omitted':'cdcccc3e','size_subminimum_tol_native':'cdcccc3e','size_oversized_tol_omitted':'00004844','size_corpus_numeric_width_tol_omitted':'1006be40','size_corpus_style_pair':'1006be40'}[row['case']]
    if row['source_model_get_pen_size_f32_bits'] != expected_model_size:
        raise SystemExit(f"native Model size differs from the configured saved style: {row['case']}")
    if row['native_pen_getsize_f32_bits'] != expected_pen_size:
        raise SystemExit(f"native Pen size differs from the expected clamp: {row['case']}")
    expected_setsize_input = {'size_omitted_tol_omitted':'00000000','size_zero_tol_omitted':'00000000','size_subminimum_tol_native':'cdcccc3d','size_oversized_tol_omitted':'00006144','size_corpus_numeric_width_tol_omitted':'1006be40','size_corpus_style_pair':'1006be40'}[row['case']]
    if len(row['native_pen_setsize_calls']) != 2 or row['native_pen_setsize_calls'][1]['input_s0'] != expected_setsize_input or row['native_pen_setsize_calls'][1]['return_w0'] != 1:
        raise SystemExit(f"native Pen::SetSize did not receive the source Model value: {row['case']}")
    expected_tolerance = {'size_omitted_tol_omitted':'00000000','size_zero_tol_omitted':'00000000','size_subminimum_tol_native':'058b823e','size_oversized_tol_omitted':'00000000','size_corpus_numeric_width_tol_omitted':'00000000','size_corpus_style_pair':'058b823e'}[row['case']]
    if row['model_get_initial_tolerance_f32_bits'] != expected_tolerance:
        raise SystemExit(f"native tolerance default/setter value differs: {row['case']}")
    if row['style_initial_tolerance_setter'] == 'omit' and row['tolerance_trace'][1]['result_s0'] != '00000000':
        raise SystemExit(f"omitted tolerance does not default to zero: {row['case']}")
    expected_arrays = {
        'points_f32_bits': [f'{v:08x}' for v in struct.unpack('<6I', struct.pack('<6f', 0,0,.1,0,.2,0))],
        'pressure_f32_bits': [f'{v:08x}' for v in struct.unpack('<3I', struct.pack('<3f', .3,.6,1.))],
        'timestamps_i32': [100,110,120],
        'tilt_f32_bits': ['00000000'] * 3,
        'orientation_f32_bits': ['00000000'] * 3,
    }
    expected_public_width = '1006be40' if row['case'].startswith('size_corpus_') else '0000c642'
    if row['adapter_public_pen_width_metadata_f32_bits'] != expected_public_width or row['adapter_public_pen_width_is_native_input']:
        raise SystemExit(f"public-width context metadata is unexpected or was marked as native input: {row['case']}")
    if row['source_arrays'] != expected_arrays:
        raise SystemExit(f"native Model arrays differ from the requested source channels: {row['case']}")
    if not row['rtv_addpoint_entries'] or any(len(item['float_words_s0_s5']) != 6 for item in row['rtv_addpoint_entries']): raise SystemExit(f"missing complete RTV4 AddPoint records: {row['case']}")
    classes=[('GLV14' if 'GLV14::RedrawPen' in item['entry'] else 'GLV16' if 'GLV16::RedrawPen' in item['entry'] else 'unknown') for item in row['redraw_entries']]
    if row['drawing_return']!=1 or classes!=['GLV14']: raise SystemExit(f"native GLV14 selection failed: {row['case']}")
    row['selected_drawable_class']=classes[0]
    if row['native_pen_getsize_f32_bits'] != expected_pen_size: raise SystemExit(f"unexpected Pen::GetSize: {row['case']}")
    if len(row['tolerance_trace'])!=3: raise SystemExit(f"incomplete native tolerance path: {row['case']}")
    trace=row['tolerance_trace']
    if trace[0]['phase']!='entry' or trace[1]['phase']!='return' or trace[2]['event']!='GLV14 RedrawPen post-tolerance store': raise SystemExit(f"tolerance getter/store path not executed: {row['case']}")
    if trace[1]['result_s0']!=row['model_get_initial_tolerance_f32_bits'] or trace[2]['drawable_plus44_f32_bits']!=row['model_get_initial_tolerance_f32_bits']: raise SystemExit(f"native tolerance handoff mismatch: {row['case']}")
    if row['model_setting_data_plus356_f32_bits']!=row['model_get_initial_tolerance_f32_bits']: raise SystemExit(f"model tolerance storage mismatch: {row['case']}")
if captured['corpus_reference']['style_has_pen_size']:
    raise SystemExit('expected the retained normalized V14 source fixture to omit style.pen_size')
captured['serialized_native_model_reference']={'fixture':'conformance/fountain-saved-color-native.json','fixture_sha256':SERIALIZED_FRAME_SHA256,'capture_id':v14_serialized_frame['id'],'source_sha256':v14_serialized_frame['source_sha256'],'page_sha256':v14_serialized_frame['page_sha256'],'frame_sha256':v14_serialized_frame['frame_sha256'],'new_apply_rva':v14_serialized_frame['new_apply_rva'],'native_state':{'size_bits':v14_serialized_frame['native_state']['size_bits'],'fixed_width_bits':v14_serialized_frame['native_state']['fixed_width_bits'],'point_count':v14_serialized_frame['native_state']['point_count'],'tool_type':v14_serialized_frame['native_state']['tool_type']},'relationship':'Actual native ObjectStrokeBinaryHandler::NewApplyBinary consumes the embedded corpus frame and the recorded native Model state reports size=5.938240051269531. This complements the separate actual Drawing size/tolerance controls. The retained numeric redraw fixture omits style.pen_size, so it must not be mistaken for a full ObjectStroke serialization.'}
captured['schema']='sdocx-fountain-v14-saved-size-defaults-v1'
captured['provenance']={'real_numeric_stroke_fixture':'conformance/fountain-v14-real-native.json','real_numeric_stroke_fixture_sha256':CORPUS_SHA256,'serialized_native_model_fixture':'conformance/fountain-saved-color-native.json','serialized_native_model_fixture_sha256':SERIALIZED_FRAME_SHA256,'runner':'conformance/fountain_saved_caller_native.py','runner_sha256':RUNNER_SHA256,'runner_executed_source_sha256':hashlib.sha256(source.encode()).hexdigest(),'helpers':{'native_drawing_runtime.py':'65dd0a4294332b68410d9436762c4f0ad84a0ad4b2a21a742e7b098e089fba91','oblique_canvas_native.py':'1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2','fountain_native.py':'40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30','elf_plt.py':'816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b'},'libraries':{'FountainPen':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec','PenCommon':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d','DefaultPen':'1b37056c81646c407e9e0697c78e31b50ffab0bdb896966d1e89feb02385544f','Drawing':'788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd','Model':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a','Base':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb','Graphics':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621','Composer':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f','libc++_shared':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'},'in_memory_patch_anchors':patches,'wrapper_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'native_runtime':'the pinned caller initializes the real FountainPen plugin and Drawing path; IContext, allocator, queue, and canvas callbacks remain host adapters; no raster claim','observation':'Actual Drawing selects GLV14; native Model GetPenSize and Pen GetSize record saved-style input and runtime clamp. Model tolerance getter is traced into GLV14 drawable+44. Complete RTV4 six-float callback records are retained. The public pen_width context is adapter metadata only; this native caller has no separate public-width input channel.'}
encoded=json.dumps(captured,indent=2,sort_keys=True)+'\n'
if ARGS.write_fixture:
    FIXTURE.write_text(encoded)
    print(f'wrote {FIXTURE.relative_to(ROOT)} ({len(captured["cases"])} cases)')
else:
    if not FIXTURE.exists() or FIXTURE.read_text()!=encoded: raise SystemExit('native Fountain V14 saved-size fixture mismatch; use --write-fixture explicitly')
    print(f'verified {len(captured["cases"])} native Fountain V14 saved-size cases')
