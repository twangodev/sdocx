#!/usr/bin/env python3
"""Native Fountain GLV16 event-time and width-history oracle.

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
FIXTURE = ROOT / "conformance/fountain-v16-event-time-native.json"
RUNNER_SHA256 = "b0837c86483aa438923f9d4d1c6451b1ebc1c5fec4c5f7c447b928a25a8f7b19"
MODE = argparse.ArgumentParser(description=__doc__)
MODE.add_argument("--write-fixture", action="store_true")
MODE.add_argument("--check-fixture", action="store_true")
ARGS = MODE.parse_args()
if ARGS.write_fixture and ARGS.check_fixture:
    MODE.error("choose --write-fixture or --check-fixture")
if hashlib.sha256(CALLER.read_bytes()).hexdigest() != RUNNER_SHA256:
    raise SystemExit("pinned Fountain saved-caller runner changed")
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
    "for label,mode,size,advanced_value,tolerance,points,pressures,timestamps in [('offset_no_line','set',10.,'18;0;100;',2.75,[0.,0.,.1,0.,.2,0.],[.3,.6,1.],[100,110,120]),('zero_no_line','set',10.,'18;0;100;',2.75,[0.,0.,.1,0.,.2,0.],[.3,.6,1.],[0,10,20]),('negative_no_line','set',10.,'18;0;100;',2.75,[0.,0.,.1,0.,.2,0.],[.3,.6,1.],[-100,-90,-80]),('duplicate_no_line','set',10.,'18;0;100;',2.75,[0.,0.,.1,0.,.2,0.],[.3,.6,1.],[100,100,120]),('reverse_no_line','set',10.,'18;0;100;',2.75,[0.,0.,.1,0.,.2,0.],[.3,.6,1.],[120,110,100]),('offset_accepted_line','set',10.,'18;0;100;',2.75,[0.,0.,4.,0.,8.,0.],[.3,.6,1.],[100,110,120])]:\n v16_rtv5_addpoint_host_boundary.clear()",
    "v16-case-matrix",
)
replace_once(
    "p.put(pts,'6f',0,0,10,0,20,2);p.put(prs,'3f',.3,.6,1.);p.put(ts,'3i',100,110,120);p.put(tilt,'3f',0,0,0);p.put(ori,'3f',0,0,0)",
    "p.put(pts,'6f',*points);p.put(prs,'3f',*pressures);p.put(ts,'3i',*timestamps);p.put(tilt,'3f',0,0,0);p.put(ori,'3f',0,0,0)",
    "source-motion-channels",
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
new_hook = " v16_width_trace=[]; native_pending_returns={}; motion_ctor_trace=[]; v16_flow_entries=[]\n flow_specs=[('startPen','SPen::FountainPenStrokeDrawableGLV16::startPen(SPen::MotionEvent const*, SPen::RectF*, bool)'),('drawLine','SPen::FountainPenStrokeDrawableGLV16::drawLine(float, float, float, float, long long, int, SPen::RectF*, bool)'),('endPen','SPen::FountainPenStrokeDrawableGLV16::endPen(SPen::MotionEvent const*, SPen::RectF*, bool)')]; flow_addrs={p.syms[n]:tag for tag,n in flow_specs}\n width_helper_addr=p.syms['SPen::WidthSmoothManager::getSmoothedWidthFromList(float, float, long long, float, SPen::WidthSmoothManager::CALL_FROM_TYPE)']; event_time_addr=p.syms['SPen::MotionEvent::GetEventTime() const']; assert event_time_addr==0x20c0634, hex(event_time_addr); event_ctor_addr=p.syms['SPen::MotionEvent::MotionEvent(long long, int, int, SPen::MotionEvent::PointerProperties*, SPen::MotionEvent::PointerCoords*, int, int, int)']\n def native_width_hook(u,a,z,d):\n  if a in flow_addrs:\n   v16_flow_entries.append({'method':flow_addrs[a],'pc':hex(a),'lr':hex(u.reg_read(R.UC_ARM64_REG_LR)),'args_x0_x7':[hex(p.x(i)) for i in range(8)],'args_s0_s3':[struct.pack('<I',u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))).hex() for i in range(4)]})\n  elif a==event_ctor_addr:\n   _lr=u.reg_read(R.UC_ARM64_REG_LR); _entry={'kind':'MotionEvent::MotionEvent','phase':'entry','this':hex(p.x(0)),'args_x0_x7':[hex(p.x(i)) for i in range(8)],'lr':hex(_lr)}; motion_ctor_trace.append(_entry); native_pending_returns.setdefault(_lr,[]).append(_entry)\n  elif a==event_time_addr:\n   _lr=u.reg_read(R.UC_ARM64_REG_LR); _entry={'kind':'MotionEvent::GetEventTime','this':hex(p.x(0)),'lr':hex(_lr),'phase':{0x76bdc:'startPen',0x773ac:'endPen_event_time',0x77478:'endPen_event_time'}.get(_lr,'other'),'return_x0':None}; v16_width_trace.append(_entry); native_pending_returns.setdefault(_lr,[]).append(_entry)\n  elif a==width_helper_addr:\n   _this=p.x(0); _lr=u.reg_read(R.UC_ARM64_REG_LR); _entry={'kind':'getSmoothedWidthFromList','manager':hex(_this),'mode':p.get(_this+44,'i')[0],'time_x1':p.x(1),'time_x1_i64':p.x(1) if p.x(1)<(1<<63) else p.x(1)-(1<<64),'call_type_w2':p.x(2),'args_s0_s3':[struct.pack('<I',u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))).hex() for i in range(4)],'lr':hex(_lr),'phase':{0x76c78:'startPen',0x78c0c:'redraw_initial',0x78310:'drawLine',0x77494:'endPen'}.get(_lr,'other'),'result_s0':None}; v16_width_trace.append(_entry); native_pending_returns.setdefault(_lr,[]).append(_entry)\n  elif a in native_pending_returns and native_pending_returns[a]:\n   _entry=native_pending_returns[a].pop()\n   if not native_pending_returns[a]: native_pending_returns.pop(a)\n   if _entry['kind']=='MotionEvent::MotionEvent': _entry['phase']='return'; _entry['object_raw_96_bytes']=bytes(u.mem_read(int(_entry['this'],16),96)).hex()\n   elif _entry['kind']=='MotionEvent::GetEventTime': _entry['return_x0']=p.x(0); _entry['return_x0_i64']=p.x(0) if p.x(0)<(1<<63) else p.x(0)-(1<<64)\n   else: _entry['result_s0']=struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)).hex()\n native_hook=p.u.hook_add(UC_HOOK_CODE,native_width_hook)\n trace_hook=p.u.hook_add(UC_HOOK_CODE,trace)"
replace_once(
    " trace_hook=p.u.hook_add(UC_HOOK_CODE,trace)",
    new_hook,
    "event-time-width-native-hooks",
)
replace_once(
    "  elif a in redraw_addrs:redraws.append({'entry':redraw_addrs[a],'drawable':hex(p.x(0)),'stroke':hex(p.x(1))})",
    "  elif a==pen_tolerance_addr:pen_tolerance_calls.append({'receiver':hex(p.x(0)),'arg_s0_f32_bits':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)).hex(),'arg_s1_f32_bits':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S1)).hex(),'lr':hex(u.reg_read(R.UC_ARM64_REG_LR))})\n  elif a==tip_length_addr:\n   _manager=p.x(0); _tipdata=p.get(_manager,'Q')[0]; _tipvalue=p.get(_tipdata,'Q')[0] if _tipdata else 0; tip_trace.append({'phase':'entry','manager':hex(_manager),'data':hex(_tipdata) if _tipdata else None,'value_ptr':hex(_tipvalue) if _tipvalue else None,'lr':hex(u.reg_read(R.UC_ARM64_REG_LR))})\n  elif a==tip_length_addr+8:tip_trace[-1]['phase']='return';tip_trace[-1]['return_x0']=hex(p.x(0)) if p.x(0) else None\n  elif a==v16_redraw_addr:v16_redraw_calls.append({'receiver':hex(p.x(0)),'stroke':hex(p.x(1)),'rect':hex(p.x(2)),'flag_w3':p.x(3)})\n  elif a==tolerance_getter_addr:tolerance_trace.append({'event':'ObjectStroke::GetInitialToloerance','phase':'entry','this':hex(p.x(0)),'lr':hex(u.reg_read(R.UC_ARM64_REG_LR))})\n  elif a==tolerance_getter_addr+0xc:tolerance_trace.append({'event':'ObjectStroke::GetInitialToloerance','phase':'return','result_s0':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)).hex(),'lr':hex(u.reg_read(R.UC_ARM64_REG_LR))})\n  elif a==tolerance_store_addr:\n   _drawable=p.x(19); tolerance_trace.append({'event':'GLV16 RedrawPen post-tolerance store','drawable':hex(_drawable),'drawable_plus44_f32_bits':struct.pack('<I',p.get(_drawable+44,'I')[0]).hex(),'lr':hex(u.reg_read(R.UC_ARM64_REG_LR))})\n  elif a in redraw_addrs:redraws.append({'entry':redraw_addrs[a],'drawable':hex(p.x(0)),'stroke':hex(p.x(1))})",
    "getter-return-store-observation",
)
replace_once(
    " p.u.hook_del(trace_hook)",
    " p.u.hook_del(trace_hook); p.u.hook_del(native_hook)",
    "remove-event-time-hooks-after-case",
)
replace_once(
    "records.append({'case':label,",
    "records.append({'case':label,'source_timestamps_i32':timestamps,'source_points_f32_bits':[struct.pack('<f',v).hex() for v in points],'source_pressures_f32_bits':[struct.pack('<f',v).hex() for v in pressures],'motion_event_constructor_trace':motion_ctor_trace,'v16_flow_entries':v16_flow_entries,'style_initial_tolerance_setter':'omit' if tolerance is None else 'set','style_initial_tolerance_input_f32_bits':tolerance_bits,'model_set_initial_tolerance_return_w0':tolerance_set_return,'model_get_initial_tolerance_f32_bits':tolerance_get_bits,'model_setting_data_plus356_f32_bits':setting_data_bits,'tolerance_trace':tolerance_trace,'pen_tolerance_set_calls':pen_tolerance_calls,'tip_length_trace':tip_trace,'v16_redraw_calls':v16_redraw_calls,'v16_width_trace':v16_width_trace,'v16_rtv5_addpoint_host_boundary':list(v16_rtv5_addpoint_host_boundary),",
    "per-case-tolerance-fields",
)
# Discard only the original fixture's case-specific assertions/output tail; the
# observations above are unchanged native records from the pinned caller.
tail_start = source.index("for _row in records:\n")
source = source[:tail_start] + """
value={'profile':[1920,1080],'pen_name':'com.samsung.android.sdk.pen.pen.preload.FountainPen','tool_type':2,'stroke_type':0,'curve_enabled':True,'pen_size_f32_bits':'00002041','initial_tolerance_f32_bits':'00003040','cases':records}
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

captured['schema']='sdocx-fountain-v16-event-time-probe-v1'
captured['provenance']={'runner':'conformance/fountain_saved_caller_native.py','runner_sha256':RUNNER_SHA256,'executed_source_sha256':hashlib.sha256(source.encode()).hexdigest(),'observation':'Native Drawing GLV16 route. MotionEvent constructor/getter and WidthSmoothManager observations are raw native hooks. RTV5 AddPoint S0-S3 are captured at the pinned caller PLT host callback boundary, not native RTV5 body execution or raster output.'}
expected_names=['offset_no_line','zero_no_line','negative_no_line','duplicate_no_line','reverse_no_line','offset_accepted_line']
if [row['case'] for row in captured['cases']] != expected_names:
    raise SystemExit('unexpected native V16 event-time case set/order')
expected_times={'offset_no_line':[100,110,120],'zero_no_line':[0,10,20],'negative_no_line':[-100,-90,-80],'duplicate_no_line':[100,100,120],'reverse_no_line':[120,110,100],'offset_accepted_line':[100,110,120]}
expected_initial_width='0000c03f'
expected_end_mode2={'offset_no_line':'0000c03f','zero_no_line':'0000a040','negative_no_line':'0000a040','duplicate_no_line':'0000c03f','reverse_no_line':'0000c03f'}
expected_terminal_callback_z={'offset_no_line':'0000403f','zero_no_line':'00002040','negative_no_line':'00002040','duplicate_no_line':'0000403f','reverse_no_line':'0000403f','offset_accepted_line':'67660640'}
for row in captured['cases']:
    name=row['case']; timestamps=expected_times[name]
    points=[0.,0.,4.,0.,8.,0.] if name=='offset_accepted_line' else [0.,0.,.1,0.,.2,0.]
    expected_point_bits=[f'{struct.unpack("<I",struct.pack("<f",v))[0]:08x}' for v in points]
    if row['source_arrays']['timestamps_i32'] != timestamps or row['source_arrays']['pressure_f32_bits'] != ['3e99999a','3f19999a','3f800000']:
        raise SystemExit(f'native Model source timestamps/pressures differ: {name}')
    if row['source_arrays']['points_f32_bits'] != expected_point_bits:
        raise SystemExit(f'native Model source point coordinates differ: {name}')
    if row['source_model_get_point_count'] != 3 or row['source_arrays']['tilt_f32_bits'] != ['00000000']*3 or row['source_arrays']['orientation_f32_bits'] != ['00000000']*3:
        raise SystemExit(f'native Model optional source arrays differ: {name}')
    if row['drawing_return'] != 1 or len(row['redraw_entries']) != 1 or 'GLV16::RedrawPen' not in row['redraw_entries'][0]['entry']:
        raise SystemExit(f'actual native Drawing did not select GLV16: {name}')
    if row['source_model_get_pen_size_f32_bits'] != '00002041' or row['native_pen_getsize_f32_bits'] != '00002041' or row['model_get_initial_tolerance_f32_bits'] != '00003040':
        raise SystemExit(f'V16 size/tolerance inputs changed: {name}')
    if len(row['motion_event_constructor_trace']) != 2 or any(x['phase'] != 'return' for x in row['motion_event_constructor_trace']):
        raise SystemExit(f'incomplete native MotionEvent constructor observations: {name}')
    if [int(x['args_x0_x7'][1],16) for x in row['motion_event_constructor_trace']] != [timestamps[0] & ((1<<64)-1)]*2:
        raise SystemExit(f'native MotionEvent constructor downTime differs: {name}')
    expected_relative=(timestamps[-1]-timestamps[0]) & ((1<<64)-1)
    event_returns=[x for x in row['v16_width_trace'] if x['kind']=='MotionEvent::GetEventTime']
    expected_relative_signed=timestamps[-1]-timestamps[0]
    if not event_returns or any(x['return_x0'] != expected_relative or x['return_x0_i64'] != expected_relative_signed for x in event_returns):
        raise SystemExit(f'native MotionEvent GetEventTime differs from captured relative time: {name}')
    helpers=[x for x in row['v16_width_trace'] if x['kind']=='getSmoothedWidthFromList']
    if len(helpers) != 4 or sorted(x['mode'] for x in helpers) != [1,1,2,2]:
        raise SystemExit(f'incomplete native width-smoother mode trace: {name}')
    for mode in (1,2):
        mode_rows=[x for x in helpers if x['mode']==mode]
        seed=next((x for x in mode_rows if x['phase']=='redraw_initial'),None)
        if seed is None or seed['args_s0_s3'][2] != expected_initial_width or seed['result_s0'] != expected_initial_width:
            raise SystemExit(f'native width-history seed differs: {name} mode={mode}')
    if name in expected_end_mode2:
        terminal=[x for x in helpers if x['phase']=='endPen']
        if len(terminal)!=2 or any(x['args_s0_s3'][2]!='0000a040' or x['time_x1_i64']!=expected_relative_signed for x in terminal):
            raise SystemExit(f'native endPen raw width queries differ: {name}')
        if next(x for x in terminal if x['mode']==2)['result_s0'] != expected_end_mode2[name]:
            raise SystemExit(f'native endPen mode-2 width-history result differs: {name}')
    callbacks=row['v16_rtv5_addpoint_host_boundary']
    if len(callbacks) != (11 if name=='offset_accepted_line' else 1):
        raise SystemExit(f'unexpected native host-boundary AddPoint count: {name}')
    if callbacks[-1]['float_words_s0_s3'][2] != expected_terminal_callback_z[name]:
        raise SystemExit(f'unexpected terminal native RTV5 callback word: {name}')
    flow=[x['method'] for x in row['v16_flow_entries']]
    if flow.count('drawLine') != 2 or flow.count('endPen') != 2 or flow.count('startPen') != 0:
        raise SystemExit(f'unexpected source redraw path (gate rejection vs accepted drawLine): {name}')
    line_queries=[x for x in helpers if x['phase']=='drawLine']
    if len(line_queries) != (2 if name=='offset_accepted_line' else 0):
        raise SystemExit(f'unexpected drawLine width-helper admission: {name}')
    if name=='offset_accepted_line':
        line_rows=[x for x in helpers if x['phase']=='drawLine']
        if len(line_rows)!=2 or any(x['time_x1']!=110 or x['args_s0_s3'][2]!='67668640' or x['result_s0']!='67668640' for x in line_rows):
            raise SystemExit('accepted native drawLine width query differs')

captured['schema']='sdocx-fountain-v16-event-time-v1'
captured['provenance']={'runner':'conformance/fountain_saved_caller_native.py','runner_sha256':RUNNER_SHA256,'runner_executed_source_sha256':hashlib.sha256(source.encode()).hexdigest(),'helpers':{'native_drawing_runtime.py':'65dd0a4294332b68410d9436762c4f0ad84a0ad4b2a21a742e7b098e089fba91','oblique_canvas_native.py':'1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2','fountain_native.py':'40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30','elf_plt.py':'816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b'},'libraries':{'FountainPen':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec','PenCommon':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d','DefaultPen':'1b37056c81646c407e9e0697c78e31b50ffab0bdb896966d1e89feb02385544f','Drawing':'788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd','Model':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a','Base':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb','Graphics':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621','Composer':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f','libc++_shared':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'},'in_memory_patch_anchors':patches,'wrapper_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'native_contract':{'MotionEvent_GetEventTime_rva':'Base+0xc0634','MotionEvent_GetEventTime_instructions':['ldr x8,[x0]','ldr x9,[x8,#48]','ldr x8,[x8,#16]','ldr x9,[x9]','ldr x9,[x9]','sub x0,x9,x8','ret'],'WidthSmoothManager_getSmoothedWidthFromList_rva':'PenCommon+0x58f18','native_width_callsite_lr':{'startPen':'0x76c78','redraw_initial_seed':'0x78c0c','drawLine':'0x78310','endPen':'0x77494'},'time_semantics':'Native GetEventTime returns the latest event timestamp minus MotionEvent downTime as a signed 64-bit subtraction.'},'host_boundary':'Actual native ObjectStroke Model arrays, Drawing selection, GLV16 RedrawPen/redraw, MotionEvent constructor/GetEventTime, WidthSmoothManager helper and its mode/time/raw input/result execute. RTV5 arguments are captured at the pinned runner PLT host callback boundary; no RTV5 body/GPU raster claim.'}
encoded=json.dumps(captured,indent=2,sort_keys=True)+'\n'
if ARGS.write_fixture:
    FIXTURE.write_text(encoded)
    print(f'wrote {FIXTURE.relative_to(ROOT)} ({len(captured["cases"])} cases)')
else:
    if not FIXTURE.exists() or FIXTURE.read_text()!=encoded:
        raise SystemExit('native Fountain V16 event-time fixture mismatch; use --write-fixture explicitly')
    print(f'verified {len(captured["cases"])} native Fountain V16 event-time cases')
