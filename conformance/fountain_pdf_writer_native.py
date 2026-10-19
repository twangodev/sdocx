#!/usr/bin/env python3
"""Replay explicit Model ObjectStroke reconstruction through the native PDF pen writer.

This captures the per-pen native PDF path contract at a mocked IPDFWriter sink.
It does not parse a saved frame, create a PDF file, or establish public Notes
export reachability.

Run with the repository's pinned Unicorn/AArch64 runtime:
  PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/apk-analysis-runtime/binutils-aarch64/usr/bin \\
    LD_LIBRARY_PATH=scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu \\
    python3 conformance/fountain_pdf_writer_native.py

No arguments check the fixture; --write-fixture explicitly refreshes it.
"""
import argparse
import contextlib
import hashlib
import io
import json
import runpy
import struct
import sys
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'conformance/fountain-pdf-writer-native.json'
sys.path[:0] = [str(ROOT / 'conformance'), str(ROOT / 'scratch/apk-analysis-runtime/python')]
from unicorn import UC_HOOK_CODE
from unicorn import arm64_const as R

PINS = {
    'helpers': {
        'conformance/fountain_saved_caller_native.py': 'b0837c86483aa438923f9d4d1c6451b1ebc1c5fec4c5f7c447b928a25a8f7b19',
        'conformance/native_drawing_runtime.py': '65dd0a4294332b68410d9436762c4f0ad84a0ad4b2a21a742e7b098e089fba91',
        'conformance/oblique_canvas_native.py': '1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2',
        'conformance/fountain_native.py': '40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30',
        'conformance/elf_plt.py': '816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b',
    },
    'native_libraries': {
        'scratch/apk-analysis-native/arm64-v8a/libSPenComposer.so': '52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f',
        'scratch/apk-analysis-native/arm64-v8a/libSPenPdf.so': 'cdc62f9e02a3ef60e0dc504dbb13c4352accb811fb1ec629a7c8648954dd8f04',
        'scratch/apk-analysis-native/arm64-v8a/libSPenModel.so': '4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a',
        'scratch/apk-analysis-native/arm64-v8a/libSPenFountainPen.so': '1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec',
        'scratch/apk-analysis-native/arm64-v8a/libSPenPenCommon.so': 'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d',
        'scratch/apk-analysis-native/arm64-v8a/libSPenDefaultPen.so': '1b37056c81646c407e9e0697c78e31b50ffab0bdb896966d1e89feb02385544f',
        'scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so': '788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd',
        'scratch/apk-analysis-native/arm64-v8a/libSPenBase.so': 'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb',
        'scratch/apk-analysis-native/arm64-v8a/libSPenGraphics.so': 'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621',
        'scratch/apk-analysis-native/arm64-v8a/libSPenObliquePen.so': '9b76bdb709db9f56c5d35a1a615fe41c57ceef73c559c91efaf5bf436f90e0ee',
        'scratch/apk-analysis-native/arm64-v8a/libc++_shared.so': '4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4',
    },
    'input_fixtures': {
        'conformance/fountain-v14-real-native.json': 'c6204b33441eb77a53932e1c475e931e0f9688a8ad40788d05a7f41c58400ca3',
        'conformance/fountain-saved-caller-native.json': '552dbabd5cf1b760cf63c8fba967c7a6fe14ca124155ccc929d0587a7ffa4695',
    },
}

def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()

for group, entries in PINS.items():
    for relative, expected in entries.items():
        actual = sha(ROOT / relative)
        if actual != expected:
            raise SystemExit(f'{group} pin changed for {relative}: {actual}')

parser = argparse.ArgumentParser(description=__doc__)
mode = parser.add_mutually_exclusive_group()
mode.add_argument('--check-fixture', action='store_true', help='replay and compare the checked-in capture (default)')
mode.add_argument('--write-fixture', action='store_true', help='explicitly refresh the native capture fixture')
args = parser.parse_args()

# Reuse the existing saved-caller setup, which maps/initializes the actual
# FountainPen and DefaultPen DSOs and supplies their createPenInst loader.
_old_argv = sys.argv[:]
sys.argv = ['fountain_saved_caller_native.py', '--check-fixture']
try:
    with contextlib.redirect_stdout(io.StringIO()):
        _caller = runpy.run_path(str(PINS and ROOT / 'conformance/fountain_saved_caller_native.py'))
finally:
    sys.argv = _old_argv
rt=_caller['rt'];p=_caller['p']
composer=rt.paths['Composer']; assert __import__('hashlib').sha256(composer.read_bytes()).hexdigest()=='52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f'
# The shared runtime maps Composer's image/symbols but deliberately leaves it
# unrelocated. Relocate its local GOT and bind imported functions only to the
# already pinned closure or declared callbacks before executing its code.
import subprocess
b=composer.read_bytes(); sh=struct.unpack_from('<Q',b,40)[0]; es,sn,si=struct.unpack_from('<HHH',b,58); secs=[struct.unpack_from('<IIQQQQIIQQ',b,sh+i*es) for i in range(sn)]; names=b[secs[si][4]:secs[si][4]+secs[si][5]]; by={names[x[0]:names.find(b'\0',x[0])].decode():x for x in secs}; ds,st=by['.dynsym'],by['.dynstr']; strings=b[st[4]:st[4]+st[5]]; sy=[]
for off in range(ds[4],ds[4]+ds[5],ds[9]):
    no,info,other,ndx,val,size=struct.unpack_from('<IBBHQQ',b,off);sy.append((strings[no:strings.find(b'\0',no)].decode() if no else '',val))
for sn in ('.rela.dyn','.rela.plt'):
    sec=by.get(sn)
    if not sec:continue
    for off in range(sec[4],sec[4]+sec[5],sec[9]):
        where,info,add=struct.unpack_from('<QQq',b,off);typ,idx=info&0xffffffff,info>>32
        if typ==0x403:p.u.mem_write(rt.bases['Composer']+where,struct.pack('<Q',rt.bases['Composer']+add))
        elif typ in (0x401,0x402,0x101,0x1025,0x1026):
            name,val=sy[idx];target=rt.bases['Composer']+val if val else p.syms.get(name,0) or rt.rawsyms.get(name,0)
            if target:p.u.mem_write(rt.bases['Composer']+where,struct.pack('<Q',target+(add if typ in (0x401,0x101) else 0)))
            else:
                # Unused Android/Java entrypoints stay explicit fail-stop
                # boundaries rather than being emulated as successful.
                trap=0x200f2f00;p.u.mem_write(trap,bytes.fromhex('000020d4'));p.u.mem_write(rt.bases['Composer']+where,struct.pack('<Q',trap))
from conformance.elf_plt import resolve as resolve_plt
for off,name in resolve_plt(composer).items():
    dem=subprocess.check_output(['c++filt','-n',name],text=True).strip() if name.startswith('_Z') else name
    p.plt[rt.bases['Composer']+off]=dem
p.callbacks['SPen::ManagedObject::Unregister()']=lambda:None
p.callbacks['SPen::ManagedObject::~ManagedObject()']=lambda:None

def call(name,args=(),floats=(),limit=8_000_000):
    for i,v in enumerate(args): p.x(i,v)
    for i,v in enumerate(floats): p.f(i,v)
    p.u.reg_write(R.UC_ARM64_REG_SP,0x200ef000); p.u.reg_write(R.UC_ARM64_REG_LR,0x200f1000)
    p.u.emu_start(p.syms[name],0x200f1000,count=limit)
    if p.u.reg_read(R.UC_ARM64_REG_PC)!=0x200f1000: raise RuntimeError(f'{name} PC={p.u.reg_read(R.UC_ARM64_REG_PC):#x}')
    return p.x(0)
def string(value):
    s=p.alloc(32); call('SPen::String::String()',[s]); call('SPen::String::Construct()',[s]); b=p.alloc(len(value)+1);p.u.mem_write(b,value.encode()+b'\0');call('SPen::String::Set(char const*)',[s,b]);return s

def run(case,points,pressures,advanced,size,color, timestamps=None, tilts=None, orientations=None, tolerance=None):
    # Real ObjectStroke/ObjectStrokeImpl model records and saved-channel setters.
    obj=p.alloc(256); impl=p.alloc(512)
    call('SPen::ObjectStroke::ObjectStroke()',[obj]); call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)',[impl,obj]);p.put(obj+32,'Q',impl)
    call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj,string('com.samsung.android.sdk.pen.pen.preload.FountainPen')])
    call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[obj,string(advanced)])
    if tolerance is not None:call('SPen::ObjectStroke::SetInitialToloerance(float)',[obj],[tolerance])
    tolerance_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex() if tolerance is not None else None
    tolerance_value=call('SPen::ObjectStroke::GetInitialToloerance() const',[obj]); tolerance_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex()
    call('SPen::ObjectStroke::SetPenSize(float)',[obj],[size]); call('SPen::ObjectStroke::SetColor(unsigned int)',[obj,color])
    saved_color=call('SPen::ObjectStroke::GetColor() const',[obj])&0xffffffff
    call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj,2]);call('SPen::ObjectStroke::SetStrokeType(SPen::StrokeType)',[obj,0]);call('SPen::ObjectStroke::SetCurveEnabled(bool)',[obj,1])
    n=len(points); pts=p.alloc(8*n);prs=p.alloc(4*n);ts=p.alloc(4*n);tilt=p.alloc(4*n);ori=p.alloc(4*n)
    p.u.mem_write(pts,struct.pack('<'+'f'*(2*n),*(q for xy in points for q in xy)));p.u.mem_write(prs,struct.pack('<'+'f'*n,*pressures));p.u.mem_write(ts,struct.pack('<'+'i'*n,*(timestamps or range(100,100+n))));p.u.mem_write(tilt,struct.pack('<'+'f'*n,*(tilts or [0.]*n)));p.u.mem_write(ori,struct.pack('<'+'f'*n,*(orientations or [0.]*n)))
    call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj,pts,prs,ts,tilt,ori,n])
    # Verify Model getters expose exactly the submitted channel bytes before
    # allowing Composer's native getDrawPointData/pen-plugin path to run.
    expected_channels={'points':bytes(p.u.mem_read(pts,8*n)),'pressures':bytes(p.u.mem_read(prs,4*n)),'timestamps':bytes(p.u.mem_read(ts,4*n)),'tilts':bytes(p.u.mem_read(tilt,4*n)),'orientations':bytes(p.u.mem_read(ori,4*n))}
    native_channels={}
    getters={'points':'SPen::ObjectStroke::GetPoint() const','pressures':'SPen::ObjectStroke::GetPressure() const','timestamps':'SPen::ObjectStroke::GetTimeStamp() const','tilts':'SPen::ObjectStroke::GetTilt() const','orientations':'SPen::ObjectStroke::GetOrientation() const'}
    for key,getter in getters.items():
        ptr=call(getter,[obj]);native_channels[key]=bytes(p.u.mem_read(ptr,len(expected_channels[key])))
        if native_channels[key]!=expected_channels[key]:raise RuntimeError(f'Model getter channel mismatch: {case} {key}')
    model_count=call('SPen::ObjectStroke::GetPointCount() const',[obj])
    if model_count!=n:raise RuntimeError(f'Model point count mismatch for {case}: {model_count} != {n}')

    # Substitute only IPDFWriter/path/paint virtual endpoints. Vtable entries
    # are bound to PDFWriter/PDFPath/PDFPaint names from libSPenPdf's exported
    # vtables: writer +88 CreatePaint, +104 CreatePath, +120 DrawPath; path
    # +40 Circle(PointF,float,bool); paint +24 SetColor and +32 SetStyle.
    events=[]; writer=p.alloc(128); wvt=p.alloc(256); path=p.alloc(32);pvt=p.alloc(256);paint=p.alloc(32);avt=p.alloc(256)
    p.put(writer,'Q',wvt);p.put(path,'Q',avt);p.put(paint,'Q',pvt)
    ADDR={'create_paint':0x200f3000,'create_path':0x200f3020,'draw_path':0x200f3040,'destroy_paint':0x200f3060,'destroy_path':0x200f3080,'circle':0x200f30a0,'style':0x200f30c0,'color':0x200f30e0}
    for off,key in ((88,'create_paint'),(104,'create_path'),(120,'draw_path'),(96,'destroy_paint'),(112,'destroy_path')):p.put(wvt+off,'Q',ADDR[key])
    p.put(avt+40,'Q',ADDR['circle']);p.put(pvt+32,'Q',ADDR['style']);p.put(pvt+24,'Q',ADDR['color'])
    def hook(u,a,sz,d):
        if a not in ADDR.values():return
        name=next(k for k,v in ADDR.items() if v==a)
        if name=='create_paint':
            p.x(0,paint);events.append({'op':'IPDFWriter::CreatePaint','result':'paint'})
        elif name=='create_path':
            p.x(0,path);events.append({'op':'IPDFWriter::CreatePath','result':'path'})
        elif name=='circle':
            if p.x(0)!=path:raise RuntimeError('PDFPath::Circle receiver differs from CreatePath result')
            words=[f'{u.reg_read(getattr(R,f"UC_ARM64_REG_S{i}")):08x}' for i in range(3)]
            vals=[struct.unpack('<f',struct.pack('<I',int(word,16)))[0] for word in words]
            abi_vals=[struct.unpack('<f',struct.pack('<I',u.reg_read(getattr(R,f"UC_ARM64_REG_S{i}"))&0xffffffff))[0] for i in range(3)]
            if vals!=abi_vals or not all(__import__('math').isfinite(v) for v in vals) or vals[2]<0:raise RuntimeError(f'Circle f32 decode/ABI mismatch: {vals} != {abi_vals}')
            events.append({'op':'PDFPath::Circle(PointF,float,bool)','receiver':'path','s0_f32_bits':words[0],'s1_f32_bits':words[1],'s2_f32_bits':words[2],'point_x':vals[0],'point_y':vals[1],'radius':vals[2],'force_close_w1':p.x(1)&0xffffffff})
        elif name=='color':
            if p.x(0)!=paint:raise RuntimeError('PDFPaint::SetColor receiver differs from CreatePaint result')
            events.append({'op':'PDFPaint::SetColor(uint32)','receiver':'paint','argb_w1':hex(p.x(1)&0xffffffff)})
        elif name=='style':
            if p.x(0)!=paint:raise RuntimeError('PDFPaint::SetStyle receiver differs from CreatePaint result')
            events.append({'op':'PDFPaint::SetStyle(PDFPaintStyle)','receiver':'paint','style_w1':p.x(1)&0xffffffff})
        elif name=='draw_path':
            if p.x(1)!=path or p.x(2)!=paint:raise RuntimeError('DrawPath path/paint arguments differ from created objects')
            events.append({'op':'IPDFWriter::DrawPath','path':'path','paint':'paint'})
        elif name=='destroy_paint':
            if p.x(1)!=paint:raise RuntimeError('DestroyPaint argument differs from CreatePaint result')
            events.append({'op':'IPDFWriter::DestroyPaint','object':'paint'})
        elif name=='destroy_path':
            if p.x(1)!=path:raise RuntimeError('DestroyPath argument differs from CreatePath result')
            events.append({'op':'IPDFWriter::DestroyPath','object':'path'})
        else:raise RuntimeError(f'unexpected PDF sink method {name}')
        u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
    hook_handle=p.u.hook_add(UC_HOOK_CODE,hook)
    ctx=_caller['ctx']
    pen=call('SPen::PDFPenFactory::CreatePDFPen(SPen::String const*, SPen::IContext const*, SPen::IPDFWriter*, float)',[string('com.samsung.android.sdk.pen.pen.preload.FountainPen'),ctx,writer],[1.0])
    if not pen:raise RuntimeError('PDFPenFactory returned null')
    # PDFPen base vtable's virtual WriteObject slot is read from object vtable.
    # Exercise the actual concrete constructor into a fresh PDFPen-sized
    # receiver; retain factory output separately as evidence only.
    factory_pen=pen
    factory_vptr=p.get(factory_pen,'Q')[0]
    pen=p.alloc(0x40);ctor='SPen::PDFFountainPen::PDFFountainPen(SPen::IContext const*, SPen::IPDFWriter*, float)'
    ctor_addr=p.syms[ctor]; ctor_entries=[]
    def trace_ctor(u,a,sz,d):
        if a==ctor_addr:ctor_entries.append({'receiver_matches_expected':p.x(0)==pen,'context_matches_expected':p.x(1)==ctx,'writer_matches_expected':p.x(2)==writer,'scale_s0_f32_bits':f'{u.reg_read(R.UC_ARM64_REG_S0):08x}'})
    ctor_hook=p.u.hook_add(UC_HOOK_CODE,trace_ctor)
    call(ctor,[pen,ctx,writer],[1.0])
    p.u.hook_del(ctor_hook)
    vptr=p.get(pen,'Q')[0]; expected_vptr=rt.bases['Composer']+0x5658e0
    vtable_words=p.get(vptr,'3Q') if 0<vptr<0x100000000 else ()
    expected_vtable_words=(rt.bases['Composer']+0x373d94,rt.bases['Composer']+0x373d98,p.syms['SPen::PDFFountainPen::WriteObject(SPen::ObjectStroke const*)'])
    direct_fn=p.syms['SPen::PDFFountainPen::WriteObject(SPen::ObjectStroke const*)']
    virtual_fn=p.get(vptr+16,'Q')[0] if vptr else 0
    dispatch_route='direct-symbol-fallback'
    fn=direct_fn
    if virtual_fn==direct_fn:
        fn=virtual_fn;dispatch_route='verified-vtable-slot+16'
    plugin_methods={
        'V14':'SPen::FountainPenStrokeDrawableGLV14::GetDrawnPointData(SPen::ObjectStroke const*, SPen::DrawnPointData&)',
        'V16':'SPen::FountainPenStrokeDrawableGLV16::GetDrawnPointData(SPen::ObjectStroke const*, SPen::DrawnPointData&)',
    }
    pdf_method='SPen::PDFPen::getDrawPointData(SPen::ObjectStroke const*, SPen::DrawnPointData&)'
    if pdf_method in p.callbacks or any(method in p.callbacks for method in plugin_methods.values()):
        raise RuntimeError('native geometry route unexpectedly replaced by a host callback')
    route_names={p.syms[pdf_method]:pdf_method}
    for version,method in plugin_methods.items():
        route_names[p.syms[method]]=f'FountainPenStrokeDrawableGL{version}::GetDrawnPointData'
    for version,method in plugin_methods.items():
        thunk=f'non-virtual thunk to {method}'
        if thunk in p.syms:route_names[p.syms[thunk]]=f'FountainPenStrokeDrawableGL{version}::GetDrawnPointData thunk'
    route_entries=[]
    route_hook=p.u.hook_add(UC_HOOK_CODE,lambda u,a,sz,d:route_entries.append(route_names[a]) if a in route_names else None)
    p.x(0,pen);p.x(1,obj);p.u.reg_write(R.UC_ARM64_REG_SP,0x200ef000);p.u.reg_write(R.UC_ARM64_REG_LR,0x200f1000)
    try:p.u.emu_start(fn,0x200f1000,count=30_000_000)
    except Exception as e:raise RuntimeError(f'PDFFountain WriteObject fault PC={p.u.reg_read(R.UC_ARM64_REG_PC):#x} LR={p.u.reg_read(R.UC_ARM64_REG_LR):#x} X0-X3={[hex(p.x(i)) for i in range(4)]}') from e
    if p.u.reg_read(R.UC_ARM64_REG_PC)!=0x200f1000:raise RuntimeError(f'WriteObject PC={p.u.reg_read(R.UC_ARM64_REG_PC):#x}, fn={fn:#x}')
    p.u.hook_del(hook_handle)
    p.u.hook_del(route_hook)
    if p.x(0)!=1:raise RuntimeError(f'PDFFountainPen returned failure in {case}: {p.x(0)}')
    if vptr!=expected_vptr or factory_vptr!=expected_vptr:raise RuntimeError(f'vtable mismatch in {case}: ctor={vptr:#x}, factory={factory_vptr:#x}, expected={expected_vptr:#x}')
    if tuple(vtable_words[:3])!=expected_vtable_words:raise RuntimeError(f'PDFFountainPen vtable entries mismatch in {case}: {vtable_words[:3]} != {expected_vtable_words}')
    if virtual_fn!=direct_fn:raise RuntimeError(f'WriteObject slot does not match direct symbol in {case}: {virtual_fn:#x} != {direct_fn:#x}')
    if dispatch_route!='verified-vtable-slot+16':raise RuntimeError(f'expected verified virtual dispatch in {case}')
    expected_plugin='V14' if advanced=='14;' else 'V16'
    if route_entries.count(pdf_method)!=1:raise RuntimeError(f'expected one native PDFPen::getDrawPointData call in {case}: {route_entries}')
    actual_plugins=[version for version in ('V14','V16') if f'FountainPenStrokeDrawableGL{version}::GetDrawnPointData' in route_entries or f'FountainPenStrokeDrawableGL{version}::GetDrawnPointData thunk' in route_entries]
    if actual_plugins!=[expected_plugin]:raise RuntimeError(f'FountainPen plugin reconstruction mismatch in {case}: {route_entries}')
    circle_events=[e for e in events if e['op']=='PDFPath::Circle(PointF,float,bool)']
    circle_payload=b''.join(b''.join(struct.pack('<I',int(e[k],16)) for k in ('s0_f32_bits','s1_f32_bits','s2_f32_bits'))+struct.pack('<I',e['force_close_w1']) for e in circle_events)
    expected_event_ops=(['IPDFWriter::CreatePaint','IPDFWriter::CreatePath']+['PDFPath::Circle(PointF,float,bool)']*len(circle_events)+['PDFPaint::SetStyle(PDFPaintStyle)','PDFPaint::SetColor(uint32)','IPDFWriter::DrawPath','IPDFWriter::DestroyPaint','IPDFWriter::DestroyPath'])
    if [e['op'] for e in events]!=expected_event_ops:raise RuntimeError(f'PDF writer event sequence mismatch for {case}: {[e["op"] for e in events]}')
    return {'case':case,'advanced_settings':advanced,'input':{'pen_name':'com.samsung.android.sdk.pen.pen.preload.FountainPen','point_count':n,'pen_size':size,'argb':hex(color),'initial_tolerance_f32_bits':tolerance_bits,'timestamps':timestamps if timestamps is not None else list(range(100,100+n)),'tilts':tilts if tilts is not None else [0.0]*n,'orientations':orientations if orientations is not None else [0.0]*n,'pressures':pressures,'points':points,'source_kind':'explicit ObjectStroke setter reconstruction'},'saved_color_getter':hex(saved_color),'model_point_count':model_count,'model_channel_sha256':{k:hashlib.sha256(v).hexdigest() for k,v in native_channels.items()},'native_entry_trace':route_entries,'factory_vptr_offset':hex(factory_vptr-rt.bases['Composer']),'factory_vptr_matches_expected':factory_vptr==expected_vptr,'constructor':{'symbol':'PDFFountainPen::PDFFountainPen(IContext const*,IPDFWriter*,float)','entry_trace':ctor_entries,'receiver_size':0x40,'vptr_offset':hex(vptr-rt.bases['Composer']),'expected_vptr_offset':'0x5658e0','vptr_bytes':bytes(p.u.mem_read(vptr,24)).hex(),'vptr_matches_expected':vptr==expected_vptr,'writeobject_virtual_slot_offset':hex(virtual_fn-rt.bases['Composer']),'direct_symbol_offset':hex(direct_fn-rt.bases['Composer']),'dispatch_route':dispatch_route,'vtable_entry_offsets':[hex(word-rt.bases['Composer']) for word in vtable_words],'expected_vtable_entry_offsets':[hex(word-rt.bases['Composer']) for word in expected_vtable_words]},'geometry_provider':'native Composer PDFPen::getDrawPointData and initialized PenManager/FountainPen plugin path','native_success':p.x(0),'sink_circle_command_count':len(circle_events),'sink_circle_command_payload_sha256':hashlib.sha256(circle_payload).hexdigest(),'sink_event_sequence':[e['op'] for e in events],'events':events}

d=json.loads((ROOT/'conformance/fountain-v14-real-native.json').read_text()); s=d['source_stroke']; ident=d['source_identity']
pts=[[q['x'],q['y']] for q in s['points']]; press=s['pressures']; timestamps=s['timestamps']; tilts=s['tilts']; orientations=s['orientations']
cases=[]
cases.append(run('synthetic-v14',[[0,0],[10,0],[20,2]],[.3,.6,1.],'14;',5.93824,0xff336699))
cases.append(run('synthetic-advanced-18-profile',[[0,0],[10,0],[20,2]],[.3,.6,1.],'18;0;100;',5.93824,0xff336699))
cases.append(run('synthetic-size-color-pressure',[[0,0],[10,0],[20,2]],[.2,.5,.9],'14;',11.87648,0xffaa2244))
cases.append(run('embedded-real-v14',pts,press,'14;',ident['pen_width'],(s['color'] if s['color'] is not None else 0xff000000),timestamps,tilts,orientations,ident['initial_tolerance']))
expected_circle_counts={'synthetic-v14':19,'synthetic-advanced-18-profile':7,'synthetic-size-color-pressure':19,'embedded-real-v14':20}
for _case in cases:
    if _case['sink_circle_command_count']!=expected_circle_counts[_case['case']]:
        raise RuntimeError(f"unexpected PDFPath::Circle count for {_case['case']}: {_case['sink_circle_command_count']}")
out={'schema':'sdocx-fountain-pdf-writer-native-v1','source_sha256':sha(Path(__file__).resolve()),'pins':PINS,'scope':'internal native per-pen PDF object writer with substituted IPDFWriter/path/paint virtual sink; no SDK JNI/UI/export-file claim','boundary_caveats':['ObjectStroke is reconstructed explicitly through native Model setters; the saved frame is not loaded by this oracle.','Composer PDFPenFactory, PDFFountainPen constructor/verified vtable dispatch, PDFPen::getDrawPointData, and the selected initialized FountainPen plugin method execute natively.','IPDFWriter, PDFPath, and PDFPaint are mocked virtual sink objects; no PDFium writer or PDF document is run.','No public Samsung Notes JNI/UI/export-file reachability is established.','Runtime setup borrows fountain_saved_caller_native.py and its declared host stubs/callbacks, allocator, contexts, and dlopen/dlsym bindings; Composer Android-only imports are fail-stop traps.','PDFPath/PDFPaint ABI slots are evidenced from pinned libSPenPdf symbols/vtables; runtime records invocations at the substituted endpoint.'],'source_fixture':{'path':'conformance/fountain-v14-real-native.json','sha256':PINS['input_fixtures']['conformance/fountain-v14-real-native.json'],'source_provenance':d['source_provenance']},'real_case_binding':{'kind':'source-derived channel reconstruction, not full original stroke reconstruction','copied_fields':['points','pressures','timestamps','tilts','orientations','pen_name','advanced_settings','pen_width','color (source null replaced with explicit black when null)','initial_tolerance'],'source_identity':ident,'source_rendering':s['rendering'],'omitted_fields':['fixed_width=false is present in source identity but this harness does not call a native fixed-width setter; remaining ObjectStroke state is model default unless explicitly listed.']},'profile_note':'The 18;0;100; input is an advanced-settings value whose verified Drawing profile is GLV16; this report does not name it native V18.','composer_sha256':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f','model_sha256':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a','pdf_sha256':'cdc62f9e02a3ef60e0dc504dbb13c4352accb811fb1ec629a7c8648954dd8f04','pdf_sink_abi':{'writer_vtable_slots':{'0x58':'CreatePaint','0x60':'DestroyPaint','0x68':'CreatePath','0x70':'DestroyPath','0x78':'DrawPath(IPDFPath const*,IPDFPaint*)'},'path_vtable_slot_0x28':'Circle(PointF,float,bool): this in x0, PointF x/y and radius in s0/s1/s2, bool in w1','paint_vtable_slots':{'0x18':'SetColor(uint32): ARGB in w1','0x20':'SetStyle(PDFPaintStyle): style in w1'},'evidence':'libSPenPdf.so exported PDFWriter/PDFPath/PDFPaint vtables and method relocations'},'pdf_fountain_vtable_elf_symbol':'_ZTVN4SPen14PDFFountainPenE','pdf_fountain_vtable_value_address':hex(rt.bases['Composer']+0x5658d0),'pdf_fountain_vtable_address_point':hex(rt.bases['Composer']+0x5658e0),'fixture':'conformance/fountain-v14-real-native.json','fixture_provenance':d['source_provenance'],'results':cases}
rendered=json.dumps(out,indent=2,sort_keys=True)+'\n'
if args.write_fixture:
    FIXTURE.write_text(rendered)
    print(f'wrote {FIXTURE.relative_to(ROOT)} sha256={sha(FIXTURE)}')
else:
    if not FIXTURE.exists():
        raise SystemExit(f'missing {FIXTURE.relative_to(ROOT)}; use --write-fixture to create it')
    existing=FIXTURE.read_text()
    if existing!=rendered:
        raise SystemExit('fixture mismatch; inspect output with --write-fixture only after review')
    print(f'fixture matches {FIXTURE.relative_to(ROOT)} sha256={sha(FIXTURE)}')
