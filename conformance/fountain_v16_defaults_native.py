#!/usr/bin/env python3
"""Native Fountain GLV16 size and tolerance handoff oracle.

The pinned Fountain saved-caller runner is executed in memory with narrowly
checked source substitutions. It is not modified; its native Drawing path,
runtime adapters and native library pins are reused. V16 AddPoint arguments are
recorded at the runner's existing RTV5 PLT host callback boundary; this is not
execution of the RTV5 function body or GPU rasterization.
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
FIXTURE = ROOT / "conformance/fountain-v16-defaults-native.json"
RUNNER_SHA256 = "b0837c86483aa438923f9d4d1c6451b1ebc1c5fec4c5f7c447b928a25a8f7b19"
MODE = argparse.ArgumentParser(description=__doc__)
MODE.add_argument("--write-fixture", action="store_true")
MODE.add_argument("--check-fixture", action="store_true")
ARGS = MODE.parse_args()
if ARGS.write_fixture and ARGS.check_fixture:
    MODE.error("choose --write-fixture or --check-fixture")
if hashlib.sha256(CALLER.read_bytes()).hexdigest() != RUNNER_SHA256:
    raise SystemExit("pinned Fountain saved-caller runner changed")
SIZE_SOURCE_FIXTURE = ROOT / "conformance/fountain-v14-real-native.json"
SIZE_SOURCE_SHA256 = "c6204b33441eb77a53932e1c475e931e0f9688a8ad40788d05a7f41c58400ca3"
if hashlib.sha256(SIZE_SOURCE_FIXTURE.read_bytes()).hexdigest() != SIZE_SOURCE_SHA256:
    raise SystemExit("pinned positive-size example source fixture changed")

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
    "rt=native_drawing_runtime.create_runtime();p=rt.p; p.dots=[]; LIBS=rt.LIBS; env=rt.env",
    "rt=native_drawing_runtime.create_runtime();p=rt.p; p.dots=[]; LIBS=rt.LIBS; env=rt.env\nv16_rtv5_addpoint_host_boundary=[]\n_v16_original_rtv5=p.callbacks['SPen::FountainPenStrokeDrawableRTV5::AddPoint(SPen::Vector3<float>, float)']\ndef _capture_v16_rtv5_addpoint():\n _words=[struct.pack('<I',p.u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))).hex() for i in range(4)]\n v16_rtv5_addpoint_host_boundary.append({'receiver':hex(p.x(0)),'float_words_s0_s3':_words})\n _v16_original_rtv5()\np.callbacks['SPen::FountainPenStrokeDrawableRTV5::AddPoint(SPen::Vector3<float>, float)']=_capture_v16_rtv5_addpoint",
    "capture-RTV5-host-import-boundary",
)
replace_once(
    "for label,mode,size,advanced_value in [(f'{v14}_{case}', mode, size, '14;') for v14 in ('v14',) for case,mode,size in [('omitted','omit',None),('zero','set',0.),('small','set',.1),('normal','set',10.)]] + [(f'v16_{case}',mode,size,'18;0;100;') for case,mode,size in [('omitted','omit',None),('zero','set',0.),('small','set',.1),('normal','set',10.)]]:",
    "for label,mode,size,advanced_value,tolerance in [('size_omitted_tol_omitted','omit',None,'18;0;100;',None),('size_zero_tol_zero','set',0.,'18;0;100;',0.),('size_small_tol_native','set',.1,'18;0;100;',0.2549668848514557),('size_saved_tol_omitted','set',5.938240051269531,'18;0;100;',None),('size10_tol_2_75','set',10.,'18;0;100;',2.75)]:\n v16_rtv5_addpoint_host_boundary.clear()",
    "v16-case-matrix",
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
    " redraw_names=['SPen::FountainPenStrokeDrawableGLV14::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)','SPen::FountainPenStrokeDrawableGLV16::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)']\n tolerance_trace=[]; pen_tolerance_calls=[]; tip_trace=[]; v16_redraw_calls=[]; tolerance_getter_addr=p.syms['SPen::ObjectStroke::GetInitialToloerance() const']; tolerance_store_addr=fpbase+0x78784; pen_tolerance_addr=p.syms['SPen::PenTolerance::SetTolerance(float, float)']; tip_length_addr=p.syms['SPen::PointTipManager::GetTipLength()']; v16_redraw_addr=p.syms['SPen::FountainPenStrokeDrawableGLV16::redraw(SPen::ObjectStroke const*, SPen::RectF*, bool)']",
    "native-tolerance-trace-addresses",
)
replace_once(
    "  elif a in redraw_addrs:redraws.append({'entry':redraw_addrs[a],'drawable':hex(p.x(0)),'stroke':hex(p.x(1))})",
    "  elif a==pen_tolerance_addr:pen_tolerance_calls.append({'receiver':hex(p.x(0)),'arg_s0_f32_bits':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)).hex(),'arg_s1_f32_bits':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S1)).hex(),'lr':hex(u.reg_read(R.UC_ARM64_REG_LR))})\n  elif a==tip_length_addr:\n   _manager=p.x(0); _tipdata=p.get(_manager,'Q')[0]; _tipvalue=p.get(_tipdata,'Q')[0] if _tipdata else 0; tip_trace.append({'phase':'entry','manager':hex(_manager),'data':hex(_tipdata) if _tipdata else None,'value_ptr':hex(_tipvalue) if _tipvalue else None,'lr':hex(u.reg_read(R.UC_ARM64_REG_LR))})\n  elif a==tip_length_addr+8:tip_trace[-1]['phase']='return';tip_trace[-1]['return_x0']=hex(p.x(0)) if p.x(0) else None\n  elif a==v16_redraw_addr:v16_redraw_calls.append({'receiver':hex(p.x(0)),'stroke':hex(p.x(1)),'rect':hex(p.x(2)),'flag_w3':p.x(3)})\n  elif a==tolerance_getter_addr:tolerance_trace.append({'event':'ObjectStroke::GetInitialToloerance','phase':'entry','this':hex(p.x(0)),'lr':hex(u.reg_read(R.UC_ARM64_REG_LR))})\n  elif a==tolerance_getter_addr+0xc:tolerance_trace.append({'event':'ObjectStroke::GetInitialToloerance','phase':'return','result_s0':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)).hex(),'lr':hex(u.reg_read(R.UC_ARM64_REG_LR))})\n  elif a==tolerance_store_addr:\n   _drawable=p.x(19); tolerance_trace.append({'event':'GLV16 RedrawPen post-tolerance store','drawable':hex(_drawable),'drawable_plus44_f32_bits':struct.pack('<I',p.get(_drawable+44,'I')[0]).hex(),'lr':hex(u.reg_read(R.UC_ARM64_REG_LR))})\n  elif a in redraw_addrs:redraws.append({'entry':redraw_addrs[a],'drawable':hex(p.x(0)),'stroke':hex(p.x(1))})",
    "getter-return-store-observation",
)
replace_once(
    "records.append({'case':label,",
    "records.append({'case':label,'style_initial_tolerance_setter':'omit' if tolerance is None else 'set','style_initial_tolerance_input_f32_bits':tolerance_bits,'model_set_initial_tolerance_return_w0':tolerance_set_return,'model_get_initial_tolerance_f32_bits':tolerance_get_bits,'model_setting_data_plus356_f32_bits':setting_data_bits,'tolerance_trace':tolerance_trace,'pen_tolerance_set_calls':pen_tolerance_calls,'tip_length_trace':tip_trace,'v16_redraw_calls':v16_redraw_calls,'v16_rtv5_addpoint_host_boundary':list(v16_rtv5_addpoint_host_boundary),",
    "per-case-tolerance-fields",
)
# Discard only the original fixture's case-specific assertions/output tail; the
# observations above are unchanged native records from the pinned caller.
tail_start = source.index("for _row in records:\n")
source = source[:tail_start] + """
value={'profile':[1920,1080],'source_input':{'pen_name':'com.samsung.android.sdk.pen.pen.preload.FountainPen','points':[[0,0],[0.1,0],[0.2,0]],'pressure':[0.3,0.6,1.0],'timestamps':[100,110,120],'tilt':[0,0,0],'orientation':[0,0,0],'tool_type':2,'stroke_type':0,'curve_enabled':True,'visible':True},'cases':records}
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
    namespace={'__file__':str(CALLER),'__name__':'__main__'}
    exec(compile(source,str(CALLER),'exec'),namespace,namespace)
finally:
    sys.stdout=old_stdout
text=''.join(writer.lines)
prefix='TOLERANCE_CAPTURE_JSON:'
payload=[line[len(prefix):] for line in text.splitlines() if line.startswith(prefix)]
if len(payload)!=1: raise SystemExit('native wrapper did not emit exactly one capture payload')
captured=json.loads(payload[0])

expected_names=['size_omitted_tol_omitted','size_zero_tol_zero','size_small_tol_native','size_saved_tol_omitted','size10_tol_2_75']
if [row['case'] for row in captured['cases']]!=expected_names: raise SystemExit('unexpected native case set/order')
expected_tolerances = {
    "size_omitted_tol_omitted": "00000000",
    "size_zero_tol_zero": "00000000",
    "size_small_tol_native": "058b823e",
    "size_saved_tol_omitted": "00000000",
    "size10_tol_2_75": "00003040",
}
expected_callbacks = {
    "size_omitted_tol_omitted": [
        ["00000000", "00000000", "cdcccc3d", "0000803f"],
        ["cdcc4c3e", "00000000", "cdcccc3d", "0000803f"],
    ],
    "size_zero_tol_zero": [
        ["00000000", "00000000", "cdcccc3d", "0000803f"],
        ["cdcc4c3e", "00000000", "cdcccc3d", "0000803f"],
    ],
    "size_small_tol_native": [["00000000", "00000000", "cdcccc3d", "0000803f"]],
    "size_saved_tol_omitted": [
        ["00000000", "00000000", "b29e9f3f", "0000803f"],
        ["cdcc4c3e", "00000000", "b29e9f3f", "0000803f"],
    ],
    "size10_tol_2_75": [["00000000", "00000000", "0000403f", "0000803f"]],
}
for row in captured['cases']:
    expected_tolerance = row['style_initial_tolerance_input_f32_bits']
    if expected_tolerance != expected_tolerances[row['case']]:
        raise SystemExit(f"unexpected tolerance input bits: {row['case']}")
    if row['model_get_initial_tolerance_f32_bits'] != expected_tolerance:
        raise SystemExit(f"Model tolerance differs from the requested/default field: {row['case']}")
    if row['style_initial_tolerance_setter'] == 'set' and row['model_set_initial_tolerance_return_w0'] != 1:
        raise SystemExit(f"native tolerance setter failed: {row['case']}")
    expected_model_size = {'size_omitted_tol_omitted':'00000000','size_zero_tol_zero':'00000000','size_small_tol_native':'cdcccc3d','size_saved_tol_omitted':'1006be40','size10_tol_2_75':'00002041'}[row['case']]
    expected_size = {'size_omitted_tol_omitted':'cdcccc3e','size_zero_tol_zero':'cdcccc3e','size_small_tol_native':'cdcccc3e','size_saved_tol_omitted':'1006be40','size10_tol_2_75':'00002041'}[row['case']]
    if row['source_model_get_pen_size_f32_bits'] != expected_model_size:
        raise SystemExit(f"Model GetPenSize differs: {row['case']}")
    if row['native_pen_getsize_f32_bits'] != expected_size:
        raise SystemExit(f"native Pen GetSize differs: {row['case']}")
    _f32word=lambda v:f'{struct.unpack("<I",struct.pack("<f",v))[0]:08x}'
    expected_arrays={'points_f32_bits':[_f32word(v) for v in (0.,0.,.1,0.,.2,0.)],'pressure_f32_bits':[_f32word(v) for v in (.3,.6,1.)],'timestamps_i32':[100,110,120],'tilt_f32_bits':['00000000']*3,'orientation_f32_bits':['00000000']*3}
    if row['source_model_get_point_count'] != 3 or row['source_arrays'] != expected_arrays: raise SystemExit(f"native Model source arrays differ: {row['case']}")
    if row['rtv_addpoint_entries']: raise SystemExit(f"unexpected native AddPoint body observations: {row['case']}")
    classes=[('GLV14' if 'GLV14::RedrawPen' in item['entry'] else 'GLV16' if 'GLV16::RedrawPen' in item['entry'] else 'unknown') for item in row['redraw_entries']]
    if row['drawing_return']!=1 or classes!=['GLV16']: raise SystemExit(f"native GLV16 selection failed: {row['case']}")
    row['selected_drawable_class']=classes[0]
    if len(row['tolerance_trace'])!=3: raise SystemExit(f"incomplete native tolerance path: {row['case']}")
    trace=row['tolerance_trace']
    if trace[0]['phase']!='entry' or trace[1]['phase']!='return' or trace[2]['event']!='GLV16 RedrawPen post-tolerance store': raise SystemExit(f"tolerance getter/store path not executed: {row['case']}")
    if trace[1]['result_s0']!=row['model_get_initial_tolerance_f32_bits'] or trace[2]['drawable_plus44_f32_bits']!=row['model_get_initial_tolerance_f32_bits']: raise SystemExit(f"native tolerance handoff mismatch: {row['case']}")
    if row['model_setting_data_plus356_f32_bits']!=row['model_get_initial_tolerance_f32_bits']: raise SystemExit(f"model tolerance storage mismatch: {row['case']}")
    if len(row['v16_redraw_calls'])!=2: raise SystemExit(f"expected two native GLV16 redraw passes: {row['case']}")
    callbacks = [item['float_words_s0_s3'] for item in row['v16_rtv5_addpoint_host_boundary']]
    if callbacks != expected_callbacks[row['case']]:
        raise SystemExit(f"native-generated RTV5 host-boundary arguments differ: {row['case']}")
    if len(row['tip_length_trace'])!=1 or row['tip_length_trace'][0]['return_x0']!='0x64': raise SystemExit(f"unexpected native GetTipLength gate result: {row['case']}")
    if len(row['pen_tolerance_set_calls'])!=2: raise SystemExit(f"expected both native PenTolerance calls: {row['case']}")
    for call in row['pen_tolerance_set_calls']:
        if call['arg_s0_f32_bits']!=row['model_get_initial_tolerance_f32_bits'] or call['arg_s1_f32_bits']!='00000000': raise SystemExit(f"PenTolerance inputs differ from Model tolerance: {row['case']}")
    if row['pen_tolerance_set_calls'][0]['lr']!='0x7878c' or row['pen_tolerance_set_calls'][1]['lr']!='0x7885c': raise SystemExit(f"unexpected GLV16 RedrawPen PenTolerance call sites: {row['case']}")
captured['schema']='sdocx-fountain-v16-defaults-v1'
captured['provenance']={'positive_size_example_source_fixture':'conformance/fountain-v14-real-native.json','positive_size_example_source_fixture_sha256':'c6204b33441eb77a53932e1c475e931e0f9688a8ad40788d05a7f41c58400ca3','runner':'conformance/fountain_saved_caller_native.py','runner_sha256':RUNNER_SHA256,'runner_executed_source_sha256':hashlib.sha256(source.encode()).hexdigest(),'helpers':{'native_drawing_runtime.py':'65dd0a4294332b68410d9436762c4f0ad84a0ad4b2a21a742e7b098e089fba91','oblique_canvas_native.py':'1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2','fountain_native.py':'40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30','elf_plt.py':'816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b'},'libraries':{'FountainPen':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec','PenCommon':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d','DefaultPen':'1b37056c81646c407e9e0697c78e31b50ffab0bdb896966d1e89feb02385544f','Drawing':'788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd','Model':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a','Base':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb','Graphics':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621','Composer':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f','libc++_shared':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'},'in_memory_patch_anchors':patches,'wrapper_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'native_contract':{'redrawpen_rva':'0x7851c','model_tolerance_getter_call_rva':'0x78774','drawable_plus44_store_rva':'0x78780','pen_tolerance_callsite_lr':['0x7878c','0x7885c'],'argument_flow':'Model getter S0 is stored in GLV16 drawable+44, then supplied as S0 to both native PenTolerance::SetTolerance(float,float) calls; S1 is +0.0.'},'native_runtime':'the pinned caller selects the real FountainPen GLV16 class and executes its ObjectStroke RedrawPen and redraw paths. Native RTV5 calls are intercepted at the pinned runner PLT host callback; the original collector remains active. IContext, allocator, queue, and canvas callbacks are host adapters; no RTV5 body or GPU raster claim.','observation':'GLV16 Model tolerance getter result is traced into drawable+44; both native PenTolerance::SetTolerance calls receive that field. Native-generated RTV5 S0-S3 args are retained at the PLT host boundary.'}
encoded=json.dumps(captured,indent=2,sort_keys=True)+'\n'
if ARGS.write_fixture:
    FIXTURE.write_text(encoded)
    print(f'wrote {FIXTURE.relative_to(ROOT)} ({len(captured["cases"])} cases)')
else:
    if not FIXTURE.exists() or FIXTURE.read_text()!=encoded: raise SystemExit('native Fountain V16 defaults fixture mismatch; use --write-fixture explicitly')
    print(f'verified {len(captured["cases"])} native Fountain V16 defaults cases')
