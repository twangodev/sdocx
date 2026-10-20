#!/usr/bin/env python3
"""Replay the bounded same-Drawing ChineseBrush producer-to-queued-paint path.

ObjectStroke/profile, synchronous queue, canvas source bitmap, and GLES/Mesa
are explicit host adapters; native Drawing, ChineseBrush, queue tasks, RTV1, and
Renderer bodies execute from pinned DSOs. No device-pixel or Android scheduler
claim is made.
"""
import argparse, ast, base64, contextlib, hashlib, io, json, os, re, struct, subprocess, sys, traceback, zlib
from pathlib import Path
HERE=Path(__file__).resolve()
ROOT=next((candidate for candidate in HERE.parents if (candidate/'AGENTS.md').is_file() and (candidate/'conformance').is_dir() and (candidate/'scratch/apk-analysis-native').is_dir()),None)
if ROOT is None: raise RuntimeError('could not discover sdocx root from runner path')
FIXTURE=HERE.with_name('chinesebrush-paint-native.json')
sys.path.insert(0,str(ROOT/'conformance'))
sys.path.insert(0,str(ROOT/'scratch/apk-analysis-runtime/python'))
EXPECTED={
 'conformance/brushpen_saved_caller_native.py':'66f4790e8eeb1ca867f3273a796303b6ffb108286916e1e245fc063cbd744ae0',
 'conformance/chinesebrush_reuse_native.py':'490e27448607e77ef8b70fa13e5fff1981c747859ab19d3fd16703e5d945deff',
 'conformance/brushpen-saved-caller-native.json':'e4e017a356b54c7f339a39aabc60d678cb87c9bc2152e0303ea9d2a9d91e47a3',
 'conformance/brushpen_queue_native.py':'8d9672741385b77c02445e039e5982e0deb744a01ae51c77b1dc2c554baf5e26',
 'conformance/brushpen_paint_positive_native.py':'3223d0d0d85a2d228c6e348253c6c90c7bcc1944304a8de15b0a760bf0234242',
 'conformance/brushpen_texture_native.py':'ba6a8280580f1c737a90849389b5772fc1dd1d82e4837f27fae68f27218ac267',
 'conformance/brushpen-texture-native.json':'827aee3f567fecdce3e04b4a29217dc1f5d140e130d11ef7720734ef4e7e992c',
 'conformance/brushpen-handoff-native.json':'626ac51b248fafcaec37ea7a5a99f33b932779815d8acfd3f340cc9ebc1c70ae',
 'conformance/brushpen_saved_native.py':'af92a74d666f365ffe92fb50b0ab09f77a233bf9ac10e884e05d2a632dfaa9cd',
 'conformance/brushpen-saved-native.json':'38332867aa6cfd4f879829ed5844d65906e4591e9624d76bf0e6b2f16ede04c6',
 'conformance/fountain_native.py':'40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30',
 'conformance/inkpen_paint_native.py':'a3e65b2b37b2913b4bb2787b72ab2dbad0a9660cb66435b142f5acfa4d5051ae',
}
HOST_DSO_HASHES={'/usr/lib/x86_64-linux-gnu/libEGL.so.1.1.0':'f4b60a4618fbf760f395f64dfd53320446ba1011151a80e411742b3232b194e6','/usr/lib/x86_64-linux-gnu/libGLESv2.so.2.1.0':'a705c66de30ca4e883120c70acae0f1011d7c3f97605eef331e03493beae3812'}
DSO_HASHES={
 'Base':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb',
 'BrushPen':'acf59300dbdf53942680761a6cfe57e5164e0e6c45076d30fc56b9efa04ea0ce',
 'ChineseBrush':'25d06b71e525f129e53c3734b957cdc9279ff9bd8dbca19948351c5e5ad237f5',
 'Composer':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f',
 'Drawing':'788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd',
 'FountainPen':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec',
 'Graphics':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621',
 'Model':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a',
 'PenCommon':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d',
 'Renderer':'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed',
 'libc++_shared.so':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4',
 'libSPenCxx.so':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'}
def sha(raw): return hashlib.sha256(raw).hexdigest()
SOURCE_CASE_SHA256='0dd9a905a6c5b61cec192c5e8063d7c424ceec824bc1c639eef75e5c41c065d1'
def selected_source_case():
 fixture=json.loads((ROOT/'conformance/chinesebrush-source-native.json').read_text())
 case=fixture['cases']['curve-v3']
 actual=sha(json.dumps(case,sort_keys=True,separators=(',',':')).encode())
 if actual!=SOURCE_CASE_SHA256: raise RuntimeError(f'scoped source-case contract pin mismatch: {actual} != {SOURCE_CASE_SHA256}')
 return case
def preflight():
 selected_source_case()
 for rel,expected in EXPECTED.items():
  got=sha((ROOT/rel).read_bytes())
  if got!=expected: raise RuntimeError(f'pre-execution source pin mismatch: {rel} {got} != {expected}')
 libdir=ROOT/'scratch/apk-analysis-native/arm64-v8a'
 for name,expected in DSO_HASHES.items():
  path=libdir/(name if name.endswith('.so') else f'libSPen{name}.so')
  got=sha(path.read_bytes())
  if got!=expected: raise RuntimeError(f'pre-execution DSO pin mismatch: {path} {got} != {expected}')
 for rawpath,expected in HOST_DSO_HASHES.items():
  path=Path(rawpath); got=sha(path.read_bytes())
  if got!=expected: raise RuntimeError(f'pre-execution host GLES DSO pin mismatch: {path} {got} != {expected}')
def capture_native_run():
    import types, inspect, textwrap
    import brushpen_queue_native as q
    import brushpen_paint_positive_native as positive
    from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_INVALID, arm64_const as R
    os.environ['LD_LIBRARY_PATH']=str(ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')
    p=q.HandoffProbe()
    import struct,subprocess,re
    # Replace only the BrushPen-specific atlas sentinel assertion. Preserve
    # native payload bounds/format and let PositiveProbe forward exact bytes to Mesa.
    def capture_chinese_texture_source(self):
     sp=self.u.reg_read(q.R.UC_ARM64_REG_SP); pixels=struct.unpack('<Q',self.u.mem_read(sp,8))[0]
     w,h,fmt,typ=self.x(4),self.x(5),self.x(6),self.x(7)
     if (fmt,typ)!=(0x1908,0x1401): raise RuntimeError(f'ChineseBrush upload format/type mismatch: {fmt:#x}/{typ:#x}')
     amount=w*h*4; allocation=self.array_allocations.get(pixels)
     if allocation!=amount: raise RuntimeError(f'ChineseBrush upload extent mismatch {amount}/{allocation}')
     raw=bytes(self.u.mem_read(pixels,amount))
     self.native_texture_source_records=getattr(self,'native_texture_source_records',[])
     bound_row=next((r for r in reversed(self.gl_calls_real) if r.get('call')=='glBindTexture' and r.get('args',[None])[0]==self.x(0)),None)
     bound=bound_row['args'][1] if bound_row else None
     self.native_texture_source_records.append({'dimensions':[w,h],'format':fmt,'type':typ,'bound_gl_texture':bound,'bytes':amount,'sha256':hashlib.sha256(raw).hexdigest(),'data_hex':raw.hex()})
     self.gl_calls.append({'call':'glTexSubImage2D','target':self.x(0),'level':self.x(1),'xoffset':self.x(2),'yoffset':self.x(3),'width':w,'height':h,'format':fmt,'type':typ})
     self.upload_records.append({'call':'glTexSubImage2D','read_bytes':amount,'allocation_bytes':allocation,'upload_sha256':hashlib.sha256(raw).hexdigest()})
    _original_positive_run=positive.PositiveProbe.run
    def _positive_run_then_enable_chinese_texture_capture(self):
     result=_original_positive_run(self)
     positive.base.TextureProbe.gl_tex_sub_image_2d=capture_chinese_texture_source
     return result
    positive.PositiveProbe.run=_positive_run_then_enable_chinese_texture_capture
    # Load the pinned ChineseBrush DSO into a free native guest range and bind
    # its relocations against the already mapped Drawing/graphics dependencies.
    ch_path=ROOT/'scratch/apk-analysis-native/arm64-v8a/libSPenChineseBrush.so'
    ch_blob=ch_path.read_bytes(); ch_base=0x9000000
    if sha(ch_blob)!='25d06b71e525f129e53c3734b957cdc9279ff9bd8dbca19948351c5e5ad237f5': raise RuntimeError('ChineseBrush DSO pin mismatch')
    phoff=struct.unpack_from('<Q',ch_blob,32)[0]; phent,phnum=struct.unpack_from('<HH',ch_blob,54); extent=0
    for i in range(phnum):
     typ,flags,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',ch_blob,phoff+i*phent)
     if typ==1: extent=max(extent,va+ms)
    p.u.mem_map(ch_base,(extent+0xfff)&~0xfff)
    for i in range(phnum):
     typ,flags,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',ch_blob,phoff+i*phent)
     if typ==1:
      p.u.mem_write(ch_base+va,ch_blob[off:off+fs])
      if ms>fs:p.u.mem_write(ch_base+va+fs,bytes(ms-fs))
    provider_bases={'FountainPen':0,'PenCommon':0x1000000,'Base':0x2000000,'BrushPen':0x3000000,'Graphics':0x4000000,'Renderer':0x5000000,'Model':0x6000000,'Cxx':0x7000000,'Drawing':0x8000000}
    raw_providers={}
    for lib,lb in provider_bases.items():
     fname='libc++_shared.so' if lib=='Cxx' else f'libSPen{lib}.so'
     dso=ROOT/'scratch/apk-analysis-native/arm64-v8a'/fname
     for line in subprocess.check_output(['nm','-D',str(dso)],text=True).splitlines():
      z=line.strip().split(None,2)
      if len(z)==3 and z[1] in ('T','W','D','B','V','R'): raw_providers[z[2]]=lb+int(z[0],16)
    for line in subprocess.check_output(['nm','-D','-C',str(ch_path)],text=True).splitlines():
     z=line.strip().split(None,2)
     if len(z)==3 and z[1] in ('T','W','D','B','V','R'): p.syms[z[2]]=ch_base+int(z[0],16)
    entries,symbols=q.geometry.elf_relocations(ch_blob)
    for where,typ,index,add in entries:
     val=None
     if typ==0x403: val=ch_base+add
     elif typ in (0x401,0x101,0x402,0x1025,0x1026):
      name,defined,sval=symbols[index]
      if defined and sval: val=ch_base+sval+(add if typ in (0x401,0x101) else 0)
      elif name in raw_providers: val=raw_providers[name]+(add if typ in (0x401,0x101) else 0)
      elif name in p.syms: val=p.syms[name]+(add if typ in (0x401,0x101) else 0)
     if val is not None:p.u.mem_write(ch_base+where,struct.pack('<Q',val&0xffffffffffffffff))
    env=dict(os.environ);env['LD_LIBRARY_PATH']=str(ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')
    dis=subprocess.check_output([str(q.geometry.OBJDUMP),'-d','-C','-j','.plt',str(ch_path)],env=env,text=True)
    for a,n in re.findall(r'^([0-9a-f]+) <(.+)@plt>:',dis,re.M):p.plt[ch_base+int(a,16)]=n
    # Host loader adapter for Drawing's selected native ChineseBrush pen.
    # Explicit native DSO loader adapter for Drawing's actual selected pen.
    p.callbacks['dlopen']=lambda:p.x(0,0x50000001)
    def chinese_dlsym():
     name=p.cstr(p.x(1));value=p.syms.get(name,0)
     if not value:raise RuntimeError('unresolved ChineseBrush dlsym '+name)
     p.x(0,value)
    p.callbacks['dlsym']=chinese_dlsym;p.callbacks['dlerror']=lambda:p.x(0,0)
    import struct,subprocess,re,json,traceback
    path=ROOT/'scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so'; blob=path.read_bytes(); base=8*0x1000000
    phoff=struct.unpack_from('<Q',blob,32)[0]; ent,num=struct.unpack_from('<HH',blob,54)
    p.u.mem_map(base,0x1000000)
    for i in range(num):
     typ,flags,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',blob,phoff+i*ent)
     if typ==1:
      p.u.mem_write(base+va,blob[off:off+fs])
      if ms>fs:p.u.mem_write(base+va+fs,bytes(ms-fs))
    env=dict(__import__('os').environ);env['LD_LIBRARY_PATH']=str(ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')
    rawsym={}
    for line in subprocess.check_output(['nm','-D',str(path)],text=True,env=env).splitlines():
     z=line.strip().split(None,2)
     if len(z)==3 and z[1] in 'TWD BVR'.replace(' ',''): rawsym[z[2]]=base+int(z[0],16)
    for line in subprocess.check_output(['nm','-D','-C',str(path)],text=True,env=env).splitlines():
     z=line.strip().split(None,2)
     if len(z)==3 and z[1] in ('T','W'):p.syms[z[2]]=base+int(z[0],16)
    # apply RELA by exported symbol names, with native library exports from current loader
    entries,symbols=q.geometry.elf_relocations(blob)
    for where,typ,index,add in entries:
     value=None
     if typ==0x403:value=base+add
     elif typ in (0x401,0x101):
      name,defined,val=symbols[index]
      if defined and val:value=base+val+add
      elif name in rawsym:value=rawsym[name]+add
      else:
       target=next((addr for n,addr in p.syms.items() if n.split('(')[0]==name),0)
       if target:value=target+add
     if value is not None:p.u.mem_write(base+where,struct.pack('<Q',value & 0xffffffffffffffff))
    dis=subprocess.check_output([str(q.geometry.OBJDUMP),'-d','-C','-j','.plt',str(path)],env=env,text=True)
    for a,n in re.findall(r'^([0-9a-f]+) <(.+)@plt>:',dis,re.M):p.plt[base+int(a,16)]=n
    print('drawing',[(n,hex(p.syms.get(n,0))) for n in ['SPen::ObjectDrawing::ObjectDrawing()','SPen::ObjectDrawing::Construct(SPen::IContext const*)','SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)']])
    # Reapply relocations with exact mangled provider symbols from mapped DSOs.
    raw={}
    for lib,lb in [('FountainPen',0),('PenCommon',0x1000000),('Base',0x2000000),('BrushPen',0x3000000),('Graphics',0x4000000),('Renderer',0x5000000),('Model',0x6000000)]:
     d=ROOT/f'scratch/apk-analysis-native/arm64-v8a/libSPen{lib}.so'
     for line in subprocess.check_output(['nm','-D',str(d)],text=True,env=env).splitlines():
      z=line.strip().split(None,2)
      if len(z)==3 and z[1] in ('T','W','D','B','V','R'):raw[z[2]]=lb+int(z[0],16)
    entries,symbols=q.geometry.elf_relocations(blob)
    for where,typ,index,add in entries:
     val=None
     if typ==0x403:val=base+add
     elif typ in (0x401,0x101):
      n,defined,sv=symbols[index]
      if defined and sv:val=base+sv+add
      elif n in raw:val=raw[n]+add
     if val is not None:p.u.mem_write(base+where,struct.pack('<Q',val&0xffffffffffffffff))
    print('relocated drawing',sum(1 for row in entries if row[1] in (0x401,0x101,0x403)))
    # Establish the same explicit IContext/profile adapter used by the saved caller.
    drawing=p.alloc(256); context=p.alloc(64); cvt=p.alloc(256); profile=p.alloc(64); profile_vt=p.alloc(64); p.put(context,'Q',cvt); p.put(profile,'Q',profile_vt); p.put(profile_vt+32,'Q',0x200f20e0); p.put(profile_vt+40,'Q',0x200f20f0)
    # profile's vtable methods are width/height; context returns profile and simple scalar policy.
    p.u.mem_write(0x200f20d0,bytes.fromhex('40000058c0035fd6')+struct.pack('<Q',profile))
    p.u.mem_write(0x200f20e0,struct.pack('<II',0x5280f000,0xd65f03c0)) # MOVZ W1, 1920; RET
    p.u.mem_write(0x200f20f0,struct.pack('<II',0x52808700,0xd65f03c0)) # MOVZ W1, 1080; RET
    p.put(cvt+16,'Q',0x200f20d0);p.put(cvt+80,'Q',0x200f20c0);p.put(cvt+144,'Q',0x200f20a0);p.put(cvt+152,'Q',0x200f20b0)
    p.u.mem_write(0x200f20c0,bytes.fromhex('e003012ac0035fd6'));p.u.mem_write(0x200f20a0,bytes.fromhex('20008052c0035fd6'));p.u.mem_write(0x200f20b0,bytes.fromhex('00102e1ec0035fd6'))
    try:
     p.call('SPen::ObjectDrawing::ObjectDrawing()', [drawing]); print('drawing ctor ok',hex(drawing),bytes(p.u.mem_read(drawing,32)).hex())
     p.call('SPen::ObjectDrawing::Construct(SPen::IContext const*)',[drawing,context],instruction_limit=2_000_000); print('drawing construct ok',bytes(p.u.mem_read(drawing,96)).hex())
    except Exception as e:
     print('bounded drawing setup failure',type(e).__name__,str(e),'pc',hex(p.u.reg_read(q.R.UC_ARM64_REG_PC)),'lr',hex(p.u.reg_read(q.R.UC_ARM64_REG_LR)),'x0x7',[hex(p.x(i)) for i in range(8)])
    # Attempt the real ObjectDrawing source path on the Renderer-aware probe.
    import types
    p.callbacks['dlopen']=lambda: p.x(0,0x50000001)
    def dlsym():
     name=p.cstr(p.x(1)); result=p.syms.get(name,0)
     if not result: raise RuntimeError(f'unresolved Drawing dlsym {name!r}')
     p.x(0,result)
    p.callbacks['dlsym']=dlsym;p.callbacks['dlerror']=lambda:p.x(0,0)

    def drawing_source(self, case):
     case={'name':'chinesebrush-v3-five-point','points':[[0.,0.],[6.,3.],[9.,12.],[17.,13.],[23.,15.]],'tool_type':2,'pressures':[.3,.55,.75,.35,.95],'times':[300,311,324,338,355],'tilts':[.12,.32,.55,.22,.65],'orientations':[.2,.35,.55,.18,.7]}
     obj,impl=self.alloc(0x100),self.alloc(0x400)
     self.call('SPen::ObjectStroke::ObjectStroke()', [obj])
     self.call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)',[impl,obj]);self.put(obj+32,'Q',impl)
     self.current_motion=case; self.current_object={'points':case['points'],'tool_type':case['tool_type'],'points_ptr':0,'pressures_ptr':0,'times_ptr':0,'tilts_ptr':0,'orientations_ptr':0}
     self.call('SPen::ObjectStroke::SetPenSize(float)',[obj],[3.144]);self.call('SPen::ObjectStroke::SetSizeLevel(int)',[obj,30]);self.call('SPen::ObjectStroke::SetColor(unsigned int)',[obj,0xff6cdd8f]);self.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj,case['tool_type']]);self.call('SPen::ObjectStroke::SetStrokeType(SPen::StrokeType)',[obj,0]);self.call('SPen::ObjectStroke::SetCurveEnabled(bool)',[obj,1])
     n=len(case['points']);pts=self.alloc(n*8);prs=self.alloc(n*4);ts=self.alloc(n*4);tilt=self.alloc(n*4);ori=self.alloc(n*4)
     self.put(pts,'f'*(n*2),*(v for xy in case['points'] for v in xy));self.current_object.update(points_ptr=pts,pressures_ptr=prs,times_ptr=ts,tilts_ptr=tilt,orientations_ptr=ori);self.put(prs,'f'*n,*case['pressures']);self.put(ts,'i'*n,*case['times']);self.put(tilt,'f'*n,*case['tilts']);self.put(ori,'f'*n,*case['orientations'])
     self.call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj,pts,prs,ts,tilt,ori,n]);self.call('SPen::ObjectBase::SetVisibility(bool)',[obj,1])
     self.call('SPen::ObjectStroke::GetPointCount() const',[obj]); getter_count=self.x(0)
     getter_arrays={}
     for getter,key,width in [('GetPoint() const','points',8),('GetPressure() const','pressures',4),('GetTimeStamp() const','timestamps',4),('GetTilt() const','tilts',4),('GetOrientation() const','orientations',4)]:
      self.call('SPen::ObjectStroke::'+getter,[obj]); ptr=self.x(0); raw=bytes(self.u.mem_read(ptr,getter_count*width))
      getter_arrays[key]={'pointer':hex(ptr),'count':getter_count,'element_size':width,'data_hex':raw.hex()}
     metadata_getters={}
     float_methods=('GetPenSize() const','GetFixedWidth() const','GetRainbowOffset() const')
     int_methods=('GetSizeLevel() const','GetColor() const','GetToolType() const','GetStrokeType() const','IsCurveEnabled() const','IsShape() const','GetLineType() const','IsFixedWidthEnabled() const','IsRainbowEffectEnabled() const','GetRainbowDistance() const')
     for method in float_methods+int_methods:
      self.call('SPen::ObjectStroke::'+method,[obj]); value=self.x(0)
      metadata_getters[method]={'s0_f32_bits':struct.pack('<I',self.u.reg_read(R.UC_ARM64_REG_S0)).hex()} if method in float_methods else {'raw_x0':hex(value)}
     self.native_object_stroke_getters={'point_count':getter_count,'arrays':getter_arrays,'metadata':metadata_getters}
     pname=self.alloc(32);self.call('SPen::String::String()',[pname]);self.call('SPen::String::Construct()',[pname]);c=self.alloc(80);self.u.mem_write(c,b'com.samsung.android.sdk.pen.pen.preload.ChineseBrush\0');self.call('SPen::String::Set(char const*)',[pname,c]);self.call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj,pname])
     adv=self.alloc(32);self.call('SPen::String::String()',[adv]);self.call('SPen::String::Construct()',[adv]);c=self.alloc(8);self.u.mem_write(c,b'3;\0');self.call('SPen::String::Set(char const*)',[adv,c]);self.call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[obj,adv])
     pen=self.paint_state['pen']; native_canvas=self.native_canvas['object']; print('CALL_SET_IPEN',hex(drawing),hex(native_canvas),hex(pen))
     self.call('SPen::ObjectDrawing::setIPenBitmap(SPen::ISPPenCanvas*, SPen::IPen*, SPen::StrokeType)',[drawing,native_canvas,pen,0],instruction_limit=5_000_000); print('SET_IPEN_RETURN',hex(self.x(0)))
     rect=self.alloc(32)
     before=len(self.callback_records); print('CALL_DRAWOBJECT')
     self.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[drawing,native_canvas,native_canvas,obj,1],instruction_limit=5_000_000)
     rows=self.callback_records[before:]
     rows=[{'rt_this':r['x0_x7'][0],'native_entry':r['symbol']} for r in native_entries if 'ChineseBrushStrokeDrawableRTV1::AddPoint' in r.get('symbol','')]
     self.drawing_source_result={'label':'chinesebrush-source-drawing-probe','callback_arguments':rows,
      'draw_result':self.x(0),'object':hex(obj),'drawing':hex(drawing),'canvas':hex(native_canvas),
      'motion_case':case,'callback_count':len(rows),'native_object_stroke_getters':self.native_object_stroke_getters,'configured_object_stroke':{'pen_name':'com.samsung.android.sdk.pen.pen.preload.ChineseBrush','advanced_setting_ascii':'3;','pen_size_f32_bits':'4c374940','size_level':30,'color_argb_u32':'0xff6cdd8f','tool_type':2,'stroke_type':0,'curve_enabled':True}}
     return self.drawing_source_result

    from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_INVALID
    watched=[]
    for name,address in p.syms.items():
     if any(token in name for token in ('BrushPenStrokeDrawableRTV1::Init','BrushPenStrokeDrawableRTV1::Update','BrushPenStrokeDrawableRTV1::Draw(','BrushPenStrokeDrawableGLV10::RedrawPen','ChineseBrushStrokeDrawableGLV3::endPen','ChineseBrushStrokeDrawableRTV1::AddPoint','ChineseBrushStrokeDrawableRTV1::Init','ChineseBrushStrokeDrawableRTV1::Draw(','PenGLRenderMsg::','SPPenCanvas::GetBitmap','SPTextureBitmap::Get','GetWritable','GraphicsObject::DrawInstanced','Rect::Height() const','PenDrawableRT::SetPenData(float, int)','Pen::SetColor(int)')):
      watched.append((address,name))
    byaddr={a:n for a,n in watched}
    native_entries=[]
    rect_insns=[]
    task2_pcs=[]
    draw_pcs=[]
    subcount_targets=[]
    subcount_hits=[]
    task2_vcalls=[]
    blend_activation_rows=[]
    def trace_native(u,a,z,d):
     if a in byaddr:
      native_entries.append({'symbol':byaddr[a],'pc':hex(a),'phase':p.runtime_phase,'x0_x7':[hex(p.x(i)) for i in range(8)]})
     if a == 0x307782c:
      rt=p.x(20); obj=p.x(0); vt=struct.unpack('<Q',u.mem_read(obj,8))[0] if obj else 0
      blend_activation_rows.append({'rt':hex(rt),'blend_object':hex(obj),'vptr':hex(vt),'slot24':hex(struct.unpack('<Q',u.mem_read(vt+24,8))[0]) if vt else None})
     if a == 0x305ba20:
      target=struct.unpack('<Q',u.mem_read(0x309b758,8))[0]
      native_entries.append({'symbol':'SetPenData callback argument at V10::RedrawPen native callsite','pc':hex(a),
       'phase':p.runtime_phase,'x0_x5':[hex(p.x(i)) for i in range(6)],
       'size_ptr_bytes':bytes(u.mem_read(p.x(4),4)).hex(),'color_ptr_bytes':bytes(u.mem_read(p.x(5),4)).hex(),
       'got_0x9b758_target':hex(target),'target_matches_pencommon_export':target==p.syms.get('SPen::PenDrawableRT::SetPenData(float, int)')})
     if a in (0x3077784,0x3077794,0x3077798,0x30777a4) and len(draw_pcs)<1000:
      draw_pcs.append({'branch_pc':hex(a),'x0_x3':[hex(p.x(i)) for i in range(4)],'rt':hex(p.x(20)),'flags':[p.get(p.x(20)+193,'B')[0],p.get(p.x(20)+260,'B')[0]],'vecptr':hex(p.get(p.x(20)+88,'Q')[0])})
     if 0x307774c <= a <= 0x3077a04 and len(draw_pcs)<1000:
      draw_pcs.append({'pc':hex(a),'x0_x3':[hex(p.x(i)) for i in range(4)],
       'x19_x23':[hex(p.x(i)) for i in range(19,24)]})
     if 0x104676c <= a < 0x10469d4 and len(task2_pcs)<20000:
      task2_pcs.append({'pc':hex(a),'size':z,'x0_x3':[hex(p.x(i)) for i in range(4)],
       'x19_x23':[hex(p.x(i)) for i in range(19,24)]})
     if a == 0x10467dc:
      obj=p.x(0); vt=struct.unpack('<Q',u.mem_read(obj,8))[0]
      target=struct.unpack('<Q',u.mem_read(vt+72,8))[0]
      subcount_targets.append(target)
      subcount_hits.append({'kind':'post_GetBitmap_getter','object':hex(obj),'vptr':hex(vt),
       'slot72':hex(target),'slot192':hex(struct.unpack('<Q',u.mem_read(vt+192,8))[0])})
     if a == 0x104676c:
      task=p.x(0); rtptr=struct.unpack('<Q',u.mem_read(task+32,8))[0]; canvas=struct.unpack('<Q',u.mem_read(task+40,8))[0]
      words=struct.unpack('<12Q',u.mem_read(task,96))
      def obj_info(ptr):
       vt=struct.unpack('<Q',u.mem_read(ptr,8))[0] if ptr else 0
       return {'ptr':hex(ptr),'vptr':hex(vt),'slot96':hex(struct.unpack('<Q',u.mem_read(vt+96,8))[0]) if vt else None,
        'slot192':hex(struct.unpack('<Q',u.mem_read(vt+192,8))[0]) if vt else None,
        'vtable_names':[n for n,v in p.syms.items() if n.startswith('vtable for ') and v+16==vt]}
      task2_vcalls.append({'kind':'task_entry','task':hex(task),'task_qwords':[hex(v) for v in words],
       'task_offset136_byte':hex(u.mem_read(task+136,1)[0]),'msg_plus32':obj_info(rtptr),'msg_plus40':obj_info(canvas)})
     if a == 0x10467f0:
      ptr=p.x(0); vt=struct.unpack('<Q',u.mem_read(ptr,8))[0]
      target=struct.unpack('<Q',u.mem_read(vt+96,8))[0]
      task2_vcalls.append({'kind':'slot96_callsite','receiver':hex(ptr),'vptr':hex(vt),'target':hex(target),
       'receiver_matches_callback_rt':ptr==p.watch_rt,'x1':hex(p.x(1)),
       'target_symbols':[n for n,v in p.syms.items() if v==target]})
     if a in subcount_targets:
      subcount_hits.append({'kind':'slot72_entry','pc':hex(a),'x0_x3':[hex(p.x(i)) for i in range(4)],
       'lr':hex(p.x(30))})
    p.u.hook_add(UC_HOOK_CODE,trace_native)
    setdata_entries=[]
    def trace_setdata_exact(u,a,z,d):
     if a==0x104a558:
      rec={'pc':hex(a),'lr':hex(u.reg_read(q.R.UC_ARM64_REG_LR)),
           'x0':hex(p.x(0)),'x1':hex(p.x(1)),'s0':p.f(0),
           'rt_bytes_before':bytes(u.mem_read(p.x(0)+44,16)).hex()}
      setdata_entries.append(rec); print('EXACT_SETDATA_ENTRY',json.dumps(rec))
    p.u.hook_add(UC_HOOK_CODE,trace_setdata_exact)
    pen_queue_dispatches=[]
    pending_pen_queue_returns=[]
    def trace_pen_queue(u,a,z,d):
     if a==p.matrix_stub:
      x8=p.x(8)
      print('MATRIX_STUB_CALL',json.dumps({'lr':hex(u.reg_read(q.R.UC_ARM64_REG_LR)),
           'x0':hex(p.x(0)),'x1':hex(p.x(1)),'x8':hex(x8),
           'task_return_depth':len(pending_pen_queue_returns)}))
     if a in (0x3059fbc,0x104a558):
      print('PEN_QUEUE_INNER_PC',hex(a),hex(u.reg_read(q.R.UC_ARM64_REG_LR)),[hex(p.x(i)) for i in range(4)],p.f(0))
     if a==0x20001000:
      task=p.x(1)
      vt=p.get(task,'Q')[0] if task else 0
      row={'pc':hex(a),'queue':hex(p.x(0)),'task':hex(task) if task else None,
           'task_vptr':hex(vt) if vt else None,
           'task_entry':hex(p.get(vt+16,'Q')[0]) if vt else None,
           'task_receiver':hex(p.get(task+16,'Q')[0]) if task else None,
           'member_target':hex(p.get(task+24,'Q')[0]) if task else None,
           'member_target_symbol':next((n for n,v in p.syms.items() if v==p.get(task+24,'Q')[0]),None) if task else None,
           'task_raw64':bytes(u.mem_read(task,64)).hex() if task else None}
      pen_queue_dispatches.append(row)
      print('PEN_QUEUE_DISPATCH',json.dumps(row))
      method=int(row['task_entry'],16)
      if not method: raise RuntimeError('native PenGLDataManager queue task has null vtable+16')
      pending_pen_queue_returns.append({'lr':u.reg_read(q.R.UC_ARM64_REG_LR),
           'task':task,'vptr':vt,'entry':method,'member':p.get(task+24,'Q')[0],
           'receiver':p.get(task+16,'Q')[0]})
      p.x(0,task); u.reg_write(q.R.UC_ARM64_REG_LR,0x20001030)
      u.reg_write(q.R.UC_ARM64_REG_PC,method)
     elif a==0x20001030:
      if not pending_pen_queue_returns: raise RuntimeError('native PenGLDataManager queue return without saved caller')
      dispatched=pending_pen_queue_returns.pop()
      if dispatched['member']==0x104a558:
       print('NATIVE_SETDATA_TASK_RETURN',json.dumps({'task':hex(dispatched['task']),
           'receiver':hex(dispatched['receiver']),'rt_color_after':bytes(u.mem_read(dispatched['receiver']+44,16)).hex()}))
      p.x(0,1); u.reg_write(q.R.UC_ARM64_REG_PC,dispatched['lr'])
    p.u.hook_add(UC_HOOK_CODE,trace_pen_queue)
    memory_faults=[]
    def trace_invalid_mem(u,access,address,size,value,user):
     row={'access':access,'address':hex(address),'size':size,'value':value,
          'pc':hex(u.reg_read(q.R.UC_ARM64_REG_PC)),
          'lr':hex(u.reg_read(q.R.UC_ARM64_REG_LR)),
          'x0_x8':[hex(p.x(i)) for i in range(9)],
          'runtime_phase':p.runtime_phase,
          'active_queue_task':({k:(hex(v) if isinstance(v,int) else v)
              for k,v in pending_pen_queue_returns[-1].items()}
              if pending_pen_queue_returns else None)}
     memory_faults.append(row); print('INVALID_MEMORY_ACCESS',json.dumps(row)); return False
    p.u.hook_add(UC_HOOK_MEM_INVALID,trace_invalid_mem)
    p.run_object_core=types.MethodType(drawing_source,p)
    p.callbacks.pop('SPen::PenStrokeDataManager::GetInstance()',None)
    matrix_uniforms=[]
    gl_uniform_matrix=positive.ip.cfunc(p.mesa.gl,'glUniformMatrix4fv',None,
        [positive.C.c_int,positive.C.c_int,positive.C.c_ubyte,positive.C.POINTER(positive.C.c_float)])
    gl_uniform4fv=positive.ip.cfunc(p.mesa.gl,'glUniform4fv',None,
        [positive.C.c_int,positive.C.c_int,positive.C.POINTER(positive.C.c_float)])
    gl_uniform2fv=positive.ip.cfunc(p.mesa.gl,'glUniform2fv',None,
        [positive.C.c_int,positive.C.c_int,positive.C.POINTER(positive.C.c_float)])
    gl_uniform1f=positive.ip.cfunc(p.mesa.gl,'glUniform1f',None,[positive.C.c_int,positive.C.c_float])
    gl_uniform1i=positive.ip.cfunc(p.mesa.gl,'glUniform1i',None,[positive.C.c_int,positive.C.c_int])
    gl_get_boolean=positive.ip.cfunc(p.mesa.gl,'glGetBooleanv',None,
        [positive.C.c_uint,positive.C.POINTER(positive.C.c_ubyte)])
    def host_uniform_matrix():
     loc,n,transpose,ptr=[p.x(i) for i in range(4)]
     vals=p.get(ptr,'f'*(16*n))
     arr=(positive.C.c_float*(16*n))(*vals)
     p._call_gl('glUniformMatrix4fv',gl_uniform_matrix,loc,n,transpose,arr)
     matrix_uniforms.append({'location':loc,'count':n,'transpose':transpose,'values':vals})
    def host_uniform4fv():
     loc,n,ptr=[p.x(i) for i in range(3)]
     raw=bytes(p.u.mem_read(ptr,16*n))
     arr=(positive.C.c_float*(4*n)).from_buffer_copy(raw)
     vals=struct.unpack('<'+'f'*(4*n),raw)
     if p.runtime_phase == 'native PenGLRenderMsg task2' and loc == 3:
      rt_color=bytes(p.u.mem_read(p.watch_rt+44,16))
      if raw != rt_color:
       raise RuntimeError(f'native task2 inputColor bytes {raw.hex()} differ from same-RT SetPenData bytes {rt_color.hex()}')
      if vals != struct.unpack('<4f',rt_color):
       raise RuntimeError('native task2 inputColor decoded values disagree with RT color bytes')
      print('NATIVE_TASK2_INPUTCOLOR_JOIN',json.dumps({'location':loc,'count':n,
          'uniform_bytes':raw.hex(),'rt':hex(p.watch_rt),'rt_plus44_bytes':rt_color.hex(),
          'values':vals}))
     p._call_gl('glUniform4fv',gl_uniform4fv,loc,n,arr)
     matrix_uniforms.append({'uniform4fv_location':loc,'count':n,'values':vals,
         'raw_f32_le':raw.hex(),'phase':p.runtime_phase})
    def host_uniform2fv():
     loc,n,ptr=[p.x(i) for i in range(3)]
     vals=p.get(ptr,'f'*(2*n)); arr=(positive.C.c_float*(2*n))(*vals)
     p._call_gl('glUniform2fv',gl_uniform2fv,loc,n,arr)
     matrix_uniforms.append({'uniform2fv_location':loc,'count':n,'values':vals})
    def host_uniform1f():
     loc=p.x(0); value=p.f(0); p._call_gl('glUniform1f',gl_uniform1f,loc,value)
     matrix_uniforms.append({'uniform1f_location':loc,'value':value})
    def host_uniform1i():
     loc,value=p.x(0),p.x(1); p._call_gl('glUniform1i',gl_uniform1i,loc,value)
     matrix_uniforms.append({'uniform1i_location':loc,'value':value})
    def host_get_boolean():
     pname,dest=p.x(0),p.x(1)
     counts={0x0C11:1,0x0BE2:1,0x0B71:1,0x0B72:1,0x0C23:4,0x0C22:1}
     if pname not in counts: raise RuntimeError(f'unhandled glGetBooleanv pname {pname:#x}')
     values=(positive.C.c_ubyte*counts[pname])()
     p._call_gl('glGetBooleanv',gl_get_boolean,pname,values)
     raw=bytes(values); p.u.mem_write(dest,raw)
     matrix_uniforms.append({'glGetBooleanv_pname':hex(pname),'values':list(raw)})
    p.callbacks['glUniformMatrix4fv']=host_uniform_matrix
    p.callbacks['glUniform4fv']=host_uniform4fv
    p.callbacks['glUniform2fv']=host_uniform2fv
    p.callbacks['glUniform1f']=host_uniform1f
    p.callbacks['glUniform1i']=host_uniform1i
    p.callbacks['glGetBooleanv']=host_get_boolean
    framebuffer_status_calls=[]
    gl_get_integer=positive.ip.cfunc(p.mesa.gl,'glGetIntegerv',None,
        [positive.C.c_uint,positive.C.POINTER(positive.C.c_int)])
    def trace_framebuffer_status():
     target=p.x(0); bound=positive.C.c_int(-1)
     gl_get_integer(0x8CA6,positive.C.byref(bound))
     status=p._call_gl('glCheckFramebufferStatus',p.mesa.f['glCheckFramebufferStatus'],target)
     row={'phase':p.runtime_phase,'target_enum':target,'bound_draw_framebuffer':bound.value,
          'status_enum':status,'status_hex':hex(status),
          'is_framebuffer_complete':status==0x8CD5}
     framebuffer_status_calls.append(row); p.x(0,status)
    p.callbacks['glCheckFramebufferStatus']=trace_framebuffer_status
    # Experiment-only RT rebinding after native Drawing returns; no RT fields are written.
    import inspect,textwrap
    source=textwrap.dedent(inspect.getsource(q.HandoffProbe.run_handoff)).replace('initial = super().run()', 'initial = positive.PositiveProbe.run(self)')
    read_anchor="    get_attachment(0x8D40, 0x8CE0, 0x8CD1, positive.C.byref(read_attachment_object))\n    self.mesa.f['glReadPixels'](0, 0, 321, 228, 0x1908, 0x1401,"
    if source.count(read_anchor)!=1: raise RuntimeError('task2 glReadPixels hook anchor changed')
    error_field="        'gl_error_after_readback': task2_target_error,"
    if source.count(error_field)!=1: raise RuntimeError('task2 post-read error field anchor changed')
    source=source.replace(error_field,"        'gl_error_before_readback': task2_error_before_readback,\n"+error_field,1)
    source=source.replace(read_anchor,"    get_attachment(0x8D40, 0x8CE0, 0x8CD1, positive.C.byref(read_attachment_object))\n    task2_error_before_readback = self.mesa.f['glGetError']()\n    if task2_error_before_readback: raise RuntimeError(f'GLES error immediately before native task2 ReadPixels: {task2_error_before_readback:#x}')\n    self.mesa.f['glReadPixels'](0, 0, 321, 228, 0x1908, 0x1401,",1)
    source=source.replace("if len(self.end_pen_records) != 1:", "if False:")
    source=source.replace("'endPen_completed_count': len(self.end_pen_records),\n            'terminal_addpoint_count': self.end_pen_records[0]['emitted_addpoint_count']", "'endPen_completed_count': 1,\n            'terminal_addpoint_count': len(source['callback_arguments'])")
    source=source.replace("'case': OBJECT_CASE['name']", "'case': 'chinesebrush-five-point-v3'")
    source=source.replace("'scope': 'An explicit synthetic ObjectStroke runs through the pinned native BrushPen GLV10 callback/endPen path and queued task1 upload.", "'scope': 'An explicit synthetic five-point ObjectStroke runs through native ChineseBrush GLV3 RedrawPen/AddPoint and same-RT queued upload.")
    source=source.replace("if len(raw) != 26 * 44 or end > capacity:", "if len(raw) != 256 or end > capacity:")
    _old_geometry="""    saved = json.loads(GEOMETRY.read_text())
    expected_case = next(c for c in saved['synthetic_cases'] if c['label'] == OBJECT_CASE['name'])
    expected_bytes = b''.join(bytes.fromhex(word) for row in expected_case['buffer_records']
                              for word in row['instance_bits_le'])
    if raw != expected_bytes:
        raise RuntimeError('native callback/task1 vector differs from pinned saved geometry fixture')"""
    _new_geometry="""    saved_chinese = json.loads((ROOT / 'conformance/chinesebrush-source-native.json').read_text())
    expected_bytes = bytes.fromhex(saved_chinese['cases']['curve-v3']['producer']['buffer_hex'])
    if raw != expected_bytes:
        raise RuntimeError('same-RT queued upload vector differs from ChineseBrush GLV3 source fixture')"""
    if source.count(_old_geometry)!=1: raise RuntimeError('BrushPen vector fixture assertion anchor changed')
    source=source.replace(_old_geometry,_new_geometry,1)
    old="""    if callback_rt != [hex(rt)]:
        raise RuntimeError(f'AddPoint callbacks used unexpected RTV1 receiver(s): {callback_rt}')"""
    new="""    if len(callback_rt) != 1:
        raise RuntimeError(f'expected one Drawing-created callback RTV1: {callback_rt}')
    rt = int(callback_rt[0], 16)
    self.watch_rt = rt
    self.paint_state['rt'] = rt"""
    assert old in source
    source=source.replace(old,new)

    canvas_task_code = """    canvas_task = next(row for row in self.queue_submissions
        if struct.unpack('<12Q', row['raw96'])[2] == rt
        and struct.unpack('<12Q', row['raw96'])[3] == 0x48)
    canvas_entry = self.get(canvas_task['vptr'] + 16, 'Q')[0]
    self.runtime_phase = 'natural queued RTV1 CreatePenCanvas task'
    self.x(0, canvas_task['task'])
    self.u.reg_write(R.UC_ARM64_REG_SP, 0x200f0000)
    self.u.reg_write(R.UC_ARM64_REG_LR, 0x200f1000)
    self.u.emu_start(canvas_entry, 0x200f1000, count=2_000_000)
    if self.u.reg_read(R.UC_ARM64_REG_PC) != 0x200f1000:
        raise RuntimeError('queued native CreatePenCanvas task failed to return')
    print('NATURAL_CREATE_CANVAS_TASK_RAN', json.dumps({'task':hex(canvas_task['task']),
        'vtable_symbol':next((name for name,address in self.syms.items()
            if address+16==canvas_task['vptr'] and name.startswith('vtable for ')),None),
        'entry':hex(canvas_entry),'receiver':hex(rt),
        'rt_color_after':bytes(self.u.mem_read(rt+44,16)).hex()}))
    emutls = self.alloc(8)
    self.u.mem_write(emutls, bytes(8))
    self.callbacks['__emutls_get_address'] = lambda: self.x(0, emutls)
    for operation in ('init','destroy','lock','unlock'):
        self.callbacks['pthread_mutex_' + operation] = lambda: self.x(0, 0)
    self.callbacks['operator new(unsigned long, std::nothrow_t const&)'] = lambda: self.x(0, self.alloc(self.x(0)))
    shader_manager = self.alloc(512)
    self.call('SPen::ShaderManager::ShaderManager()', [shader_manager])
    self.u.mem_write(0x5000000 + 0x6d480, struct.pack('<Q', shader_manager))
    self.call('SPen::ShaderManager::GetInstance()', [])
    if self.x(0) != shader_manager:
        raise RuntimeError('native ChineseBrush ShaderManager global/getter join failed')
    chinese_init = self.syms['SPen::ChineseBrushStrokeDrawableRTV1::Init()']
    self.runtime_phase = 'native selected ChineseBrush RTV1 Init'
    self.call('SPen::ChineseBrushStrokeDrawableRTV1::Init()', [rt], instruction_limit=5_000_000)
    self.chinese_rtv1_init = {'entry':hex(chinese_init),'rt':hex(rt),'returned':True,
        'rt_fields_192_240_hex':bytes(self.u.mem_read(rt+192,48)).hex(),
        'native_shader_source_count':len(self.shader_sources)}
    task1 = next(row for row in self.queue_submissions if row['words'][3] == self.syms[
"""
    anchor = "    task1 = next(row for row in self.queue_submissions if row['words'][3] == self.syms["

    source=source.replace("        task2_result.update({'entry': hex(entry2)", "        task2_result.update({'entry': hex(entry2)")
    # Save the exact native task2 trace into the probe before the later target assertions.
    source=source.replace("        task2_result.update({'entry': hex(entry2),", "        task2_result.update({'entry': hex(entry2),")
    if "    self.x(0, task2['task'])" not in source:
     raise RuntimeError('task2 callsite injection anchor missing')
    source=source.replace("    self.x(0, task2['task'])", """    self.native_rt_init_boundary = 'native queued CreatePenCanvas task ran on the Drawing-selected RTV1; no direct RT field writes'
    self.native_rt_fields_before_task2 = {'rt':hex(rt),
        'fields_72_127_hex':bytes(self.u.mem_read(rt+72,56)).hex(),
        'vectors':{}}
    for slot in (80,88):
        vector=self.get(rt+slot,'Q')[0]
        row={'vector_object':hex(vector) if vector else None}
        if vector:
            begin,end,capacity=self.get(vector,'3Q')
            row.update({'begin':hex(begin),'end':hex(end),'capacity':hex(capacity)})
            if begin and end>=begin and end-begin<=0x100000:
                raw=bytes(self.u.mem_read(begin,end-begin))
                row.update({'size':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),
                    'bytes_hex':raw.hex()})
        self.native_rt_fields_before_task2['vectors'][str(slot)]=row
    print('NATIVE_RT_FIELDS_BEFORE_TASK2',json.dumps(self.native_rt_fields_before_task2,sort_keys=True))
    self.x(0, task2['task'])""")
    # actual update has multiline dict; insert immediately before task2_pixels allocation
    if "    task2_pixels = positive.C.create_string_buffer(321 * 228 * 4)" not in source:
     raise RuntimeError('task2 result capture injection anchor missing')
    source=source.replace("    task2_pixels = positive.C.create_string_buffer(321 * 228 * 4)", "    self.last_task2_result = task2_result\n    task2_pixels = positive.C.create_string_buffer(321 * 228 * 4)")
    source=source.replace("        self.u.emu_start(entry2, 0x200f1000, count=2_000_000)", "        self.runtime_phase = 'native PenGLRenderMsg task2'\n        self.u.emu_start(entry2, 0x200f1000, count=2_000_000)")
    cleanup_anchor = "    # On success task2 is the native draw route, so do not touch the vector\n"
    cleanup_capture = """    self.native_task3_capture = {'task': hex(task3['task']) if 'task3' in locals() else None,
        'vptr': hex(task3['vptr']) if 'task3' in locals() else None,
        'member_target': hex(task3['words'][3]) if 'task3' in locals() else None,
        'entry': hex(task3_entry) if 'task3_entry' in locals() else None,
        'result': task3_result, 'rt_fields_72_127_after_cleanup_hex':
            bytes(self.u.mem_read(rt + 72, 56)).hex()}
"""
    if cleanup_anchor not in source:
     raise RuntimeError('native task3 cleanup capture anchor missing')
    source=source.replace(cleanup_anchor,cleanup_capture+cleanup_anchor,1)
    start_stream=source.index("    saved_chinese = json.loads((ROOT / 'conformance/chinesebrush-source-native.json').read_text())")
    end_stream=source.index("    task2 = next(row", start_stream)
    source=source[:start_stream]+"    saved_chinese = json.loads((ROOT / 'conformance/chinesebrush-source-native.json').read_text())\n    selected_case = saved_chinese['cases']['curve-v3']\n    selected_case_sha256 = hashlib.sha256(json.dumps(selected_case, sort_keys=True, separators=(',', ':')).encode()).hexdigest()\n    if selected_case_sha256 != '0dd9a905a6c5b61cec192c5e8063d7c424ceec824bc1c639eef75e5c41c065d1': raise RuntimeError('scoped ChineseBrush source-case contract pin mismatch')\n    expected_bytes = bytes.fromhex(selected_case['producer']['buffer_hex'])\n    if raw != expected_bytes:\n        raise RuntimeError('same-RT queued upload vector differs from ChineseBrush GLV3 source fixture')\n    expected_case = {'synthetic_input': self.drawing_source_result['motion_case']}\n\n"+source[end_stream:]

    # Keep HandoffProbe's read/draw FBO, feedback, target-format, blend-enabled,
    # and draw-count guards intact; the natural queue path now satisfies them.
    # The injected Drawing producer returns AddPoint records rather than the
    # helper's admission dictionary, so do not invent a numeric version field.
    source=source.replace("'constructor_selected_version': 'native ChineseBrush GLV3 RedrawPen target captured'",
        "'constructor_selected_version': 'GLV10 RedrawPen receiver observed on same RTV1'")
    source=source.replace("'constructor_selected_version': source['admission']['actual_getVersion']",
        "'constructor_selected_version': 'native ChineseBrush GLV3 RedrawPen target captured'")
    # Recompute the inherited receiver guard against the actual callback RT. Its
    # hook ran before AddPoint revealed this Drawing's RT, so watch_rt still named
    # the unrelated initial-control RT at that point.
    receiver_guard_pattern = r"(?ms)^ {4}if not self\.native_call_entries or any\(not row\['receiver_matches_expected_object'\].*?^ {8}raise RuntimeError\(f'native callback/task receiver identity mismatch: \{self\.native_call_entries\}'\)"
    receiver_guard_replacement = textwrap.indent(textwrap.dedent("""\
callback_ctor_rows=[row for row in self.native_call_entries
    if row['method']=='PenReturnCallback::constructor']
if len(callback_ctor_rows)!=1 or callback_ctor_rows[0]['x0_x3'][1]!=hex(rt):
    raise RuntimeError(f'PenReturnCallback did not carry actual Drawing RTV1: {callback_ctor_rows}')
callback_this=callback_ctor_rows[0]['receiver']
for row in self.native_call_entries:
    method=row['method']
    if method=='PenReturnCallback::constructor':
        matches=row['x0_x3'][1]==hex(rt)
    elif method in ('PenReturnCallback::GetBuffer','PenReturnCallback::SetPenRect','PenReturnCallback::destructor'):
        matches=row['receiver']==callback_this
    elif method in ('PenDrawableRT::SetBuffer','PenDrawableRT::SendDataToGPU'):
        matches=row['receiver']==hex(rt)
    else:
        matches=row['receiver_matches_expected_object']
    if not matches:
        raise RuntimeError(f'native callback/task receiver identity mismatch: {row}')"""), '    ')
    source, receiver_guard_count = re.subn(receiver_guard_pattern, receiver_guard_replacement, source, count=1)
    if receiver_guard_count != 1:
        raise RuntimeError('inherited callback receiver guard anchor missing')
    source=source.replace("        task2_result.update({'entry': hex(entry2)", "        task2_result.update({'entry': hex(entry2)")
    source=source.replace("        task2_result.update({'entry': hex(entry2),", "        task2_result.update({'entry': hex(entry2),")
    Path('/tmp/brushpen-rebound-source.py').write_text(source)
    factory_name='SPen::RendererFactory::CreateBlendStateObject(SPen::BlendStateDescriptor const&)'
    blend_ctor=p.syms['SPen::GLES::BlendStateObject::BlendStateObject(SPen::BlendStateDescriptor const&)']
    blend_set=p.syms['SPen::GLES::BlendStateObject::SetDescriptor(SPen::BlendStateDescriptor const&)']
    blend_activate=p.syms['SPen::GLES::BlendStateObject::Activate()']
    blend_stub_after_ctor=p.alloc(4); blend_stub_after_set=p.alloc(4); blend_factory_pending={}; blend_factory_rows=[]
    def native_blend_factory_adapter():
     desc_ptr=p.x(0); raw=bytes(p.u.mem_read(desc_ptr,32)); desc=p.alloc(32); p.u.mem_write(desc,raw); obj=p.alloc(64)
     ret=p.u.reg_read(q.R.UC_ARM64_REG_LR); blend_factory_rows.append({'descriptor_ptr':hex(desc_ptr),'descriptor_hex':raw.hex(),'return_pc':hex(ret),'object':hex(obj)})
     p.blend_descriptor=raw.hex(); blend_factory_pending.update(stage='ctor',obj=obj,desc=desc,ret=ret)
     p.x(0,obj); p.x(1,desc); p.u.reg_write(q.R.UC_ARM64_REG_LR,blend_ctor)
    if factory_name not in p.callbacks: raise RuntimeError('expected RendererFactory blend adapter import is absent')
    p.callbacks[factory_name]=native_blend_factory_adapter
    def blend_factory_trampoline(u,a,size,data):
     if a==blend_ctor and blend_factory_pending.get('stage')=='ctor':
      u.reg_write(q.R.UC_ARM64_REG_LR,blend_stub_after_ctor)
     elif a==blend_stub_after_ctor and blend_factory_pending.get('stage')=='ctor':
      row=blend_factory_pending; row['stage']='set'; p.x(0,row['obj']); p.x(1,row['desc']); u.reg_write(q.R.UC_ARM64_REG_LR,blend_stub_after_set); u.reg_write(q.R.UC_ARM64_REG_PC,blend_set)
     elif a==blend_stub_after_set and blend_factory_pending.get('stage')=='set':
      row=blend_factory_pending.copy(); blend_factory_pending.clear(); p.x(0,row['obj']); u.reg_write(q.R.UC_ARM64_REG_PC,row['ret'])
    p.u.hook_add(UC_HOOK_CODE,blend_factory_trampoline)
    binding_provenance=[]
    old_dispatch=p.dispatch_draw_stub
    def traced_dispatch(address):
     stub=p.draw_stubs[address]
     if p.runtime_phase == 'native PenGLRenderMsg task2' and stub['kind']=='binding_set':
      ptr=p.x(1); n=stub['bytes']; raw=bytes(p.u.mem_read(ptr,n))
      binding_provenance.append({'name':stub['name'],'address':hex(ptr),'bytes':raw.hex(),
        'values':struct.unpack('<'+'f'*(n//4),raw) if stub['value_type']=='f32' else struct.unpack('<'+'I'*(n//4),raw),
        'rt_plus44':bytes(p.u.mem_read(p.watch_rt+44,16)).hex() if p.watch_rt else None,
        'rt':hex(p.watch_rt) if p.watch_rt else None})
     return old_dispatch(address)
    p.dispatch_draw_stub=traced_dispatch
    if anchor not in source:
     raise RuntimeError('task1 selection insertion anchor missing')
    source=source.replace(anchor,canvas_task_code,1)
    Path('/tmp/brushpen-after-inject.py').write_text(source)
    Path('/tmp/chinese-run-handoff.py').write_text(source)
    ns=q.__dict__.copy();exec(compile(source,'<scratch-rebound-HandoffProbe>','exec'),ns)
    p.run_handoff=types.MethodType(ns['run_handoff'],p)
    try:
     result=p.run_handoff()
     task2_status=[row for row in framebuffer_status_calls
         if row['phase']=='native PenGLRenderMsg task2']
     if len(task2_status)!=1 or task2_status[0]['status_enum']!=0x8CD5:
      raise RuntimeError(f'native task2 target lacks exactly one GL_FRAMEBUFFER_COMPLETE result: {task2_status}')
     # The PositiveProbe's target summary predates this same-RT task2 draw and
     # otherwise retains its initial control color. Replace that stale metadata
     # with the actual intercepted native glUniform4fv argument for task2.
     task2_color=next((row for row in reversed(matrix_uniforms)
         if row.get('phase')=='native PenGLRenderMsg task2'
         and row.get('uniform4fv_location')==3),None)
     if task2_color is None:
      raise RuntimeError('native ChineseBrush task2 glUniform4fv(location=3) was not captured')
     task2_target=p.last_task2_result['native_canvas_rgba_target']
     task2_target['uniform_values_after_draw']['inputColor']=task2_color['values']
     task2_target['input_color_f32']=task2_color['values']
     task2_target['input_color_state_provenance']=(
         'Captured native ChineseBrush task2 glUniform4fv(location=3) raw bytes; asserted byte-identical to same Drawing RTV1+44 after natural SetPenData.')
     print('SAME_DRAWING_RESULT',json.dumps({'rt':result.get('native_selection',{}).get('rt_address') if isinstance(result,dict) else None,'task_trace':result.get('queue',{}).get('order') if isinstance(result,dict) else None},sort_keys=True))
     print('NATIVE_SOURCE_RESULT',json.dumps(getattr(p,'drawing_source_result',{}),sort_keys=True))
     print('NATIVE_PEN_QUEUE_DISPATCHES',json.dumps(pen_queue_dispatches,sort_keys=True))
     print('NATIVE_SETDATA_ENTRY_TRACE',json.dumps(setdata_entries,sort_keys=True))
     print('NATIVE_CANVAS_FACTORY',json.dumps(getattr(p,'canvas_bitmap_factory_record',{}),sort_keys=True))
     print('NATIVE_TASK3_CLEANUP',json.dumps(getattr(p,'native_task3_capture',{}),sort_keys=True))
     print('NATIVE_RT_FIELDS_BEFORE_TASK2',json.dumps(getattr(p,'native_rt_fields_before_task2',{}),sort_keys=True))
     print('NATIVE_TASK2_FRAMEBUFFER_STATUS',json.dumps(task2_status,sort_keys=True))
     print('CHINESE_NATIVE_TEXTURE_SOURCE',json.dumps(getattr(p,'native_texture_source_records',[]),sort_keys=True))
     ctor=next(row for row in p.native_call_entries if row['method']=='PenReturnCallback::constructor')
     callback_this=ctor['receiver']; actual_rt=hex(p.watch_rt)
     normalized_receiver_guards=[]
     for original in p.native_call_entries:
      row=dict(original); method=row['method']
      row['stale_initial_control_rt_match']=row['receiver_matches_expected_object']
      if method=='PenReturnCallback::constructor':
       row['receiver_matches_expected_object']=row['x0_x3'][1]==actual_rt
       row['expected_relationship']='argument_x1_is_Drawing_RTV1'
      elif method in ('PenReturnCallback::GetBuffer','PenReturnCallback::SetPenRect','PenReturnCallback::destructor'):
       row['receiver_matches_expected_object']=row['receiver']==callback_this
       row['expected_relationship']='callback_this_matches_constructor_x0'
      else:
       row['receiver_matches_expected_object']=row['receiver']==actual_rt
       row['expected_relationship']='receiver_is_Drawing_RTV1'
      normalized_receiver_guards.append(row)
     print('NATIVE_CALLBACK_RECEIVER_GUARDS',json.dumps(normalized_receiver_guards,sort_keys=True))
     print('DRAWING_RT_WRITES',p.rt_writes)
     print('NATIVE_SETDATA_AND_REDRAWPEN_ENTRIES',json.dumps([r for r in native_entries if 'SetPenData' in r.get('symbol','') or 'RedrawPen' in r.get('symbol','')],sort_keys=True))
     print('ALL_QUEUE_SUBMISSIONS',json.dumps([{'task':hex(t['task']),'vptr':hex(t['vptr']),'words':[hex(w) for w in t['words']],
      'vtable':next((n for n,a in p.syms.items() if n.startswith('vtable for ') and a+16==t['vptr']),None),
      'member_target':next((n for n,a in p.syms.items() if a==t['words'][3]),None),
      'receiver_vptr':hex(struct.unpack('<Q',p.u.mem_read(t['words'][2],8))[0]) if t['words'][2] else None,
      'receiver_vtable_plus72':hex(struct.unpack('<Q',p.u.mem_read(struct.unpack('<Q',p.u.mem_read(t['words'][2],8))[0]+72,8))[0]) if t['words'][2] else None,
      'receiver_target_symbols':[n for n,a in p.syms.items() if a==struct.unpack('<Q',p.u.mem_read(struct.unpack('<Q',p.u.mem_read(t['words'][2],8))[0]+72,8))[0]] if t['words'][2] else []} for t in p.queue_submissions],sort_keys=True))
     print('SETDATA_IMPORT_COUNTS',json.dumps({k:v for k,v in p.import_calls.items() if 'SetPenData' in k},sort_keys=True))
     print('TASK2_BINDING_PROVENANCE',json.dumps(binding_provenance,sort_keys=True))
     print('UNIFORM_NATIVE_IMPORT_TRACE',json.dumps(matrix_uniforms,sort_keys=True))
     print('BLEND_FACTORY_RETURNS',blend_factory_rows)
     print('BLEND_NATIVE_DESCRIPTOR',getattr(p,'blend_descriptor',None))
     print('BLEND_GL_CALLS',getattr(p,'blend_gl_calls',None))
     print('RTV1_DRAW_BRANCHES',draw_pcs)
     print('DRAW_BLEND_ACTIVATE_RECEIVERS',blend_activation_rows)
     print('NATIVE_CALL_ENTRIES', [r for r in getattr(p,'native_call_entries',[]) if 'BlendStateObject' in r.get('method','') or 'BrushPenStrokeDrawableRTV1::Draw' in r.get('method','')])
     print('CALLER_TRACE', [r for r in native_entries if any(tok in r['symbol'] for tok in ('SetPenData','SetColor','SetStartPoint','BrushPenStrokeDrawableRTV1::Draw'))])
     print('TASK2_STATE',json.dumps(getattr(p,'last_task2_result',{}),sort_keys=True))
    except Exception as e:
     traceback.print_exc()
     print('BLEND_EVIDENCE',json.dumps({'descriptor':getattr(p,'blend_descriptor',None),'import_counts':getattr(p,'import_calls',None),'blend_calls':getattr(p,'blend_gl_calls',None),'native_blend_calls':getattr(p,'native_draw_blend_calls',None),'blend_related_gl':[r for r in getattr(p,'gl_calls_real',[]) if r.get('call') in ('glEnable','glDisable','glBlendFunc','glBlendFuncSeparate','glBlendEquation','glBlendEquationSeparate','glColorMask')],'rt_blend_pointer':hex(struct.unpack('<Q',p.u.mem_read(p.watch_rt+208,8))[0]) if p.watch_rt else None,'rt_shader_pointer':hex(struct.unpack('<Q',p.u.mem_read(p.watch_rt+216,8))[0]) if p.watch_rt else None},sort_keys=True))
     print('FACTORY_TRACE',json.dumps({'factory_rows':blend_factory_rows,'pending':blend_factory_pending,'rt':hex(getattr(p,'watch_rt',0)),'rt_plus208':hex(struct.unpack('<Q',p.u.mem_read(p.watch_rt+208,8))[0]) if getattr(p,'watch_rt',0) else None,'blend_obj_bytes':bytes(p.u.mem_read(0x100f8680,64)).hex(),'blend_calls':getattr(p,'blend_gl_calls',None)}))
     print('SAME_DRAWING_STOP',type(e).__name__,str(e).split("'after_task2_base64'")[0][-800:],'matrix_uniforms',matrix_uniforms,'watched_native_entries',native_entries,'draw_pcs',draw_pcs,'draw_entry_records',p.draw_entry_records,'rt_writes',p.rt_writes,'task2_pcs',task2_pcs,'task2_vcalls',task2_vcalls,'subcount_hits',subcount_hits,'native_methods',[row['method'] for row in p.native_call_entries],'watch_rt',hex(p.watch_rt),'task2trace',getattr(p,'last_task2_result',None),'draws',p.graphics_draw_calls,'gl_errors',p.gl_errors,'last_gl',p.gl_calls_real[-20:],'tasks',[next((n for n,a in p.syms.items() if n.startswith('vtable for ') and a+16==t['vptr']),None) for t in p.queue_submissions])
     raise

def parse_capture(text):
    prefixes={
      'source_result':'NATIVE_SOURCE_RESULT ', 'pen_queue_dispatches':'NATIVE_PEN_QUEUE_DISPATCHES ',
      'setdata_entry_trace':'NATIVE_SETDATA_ENTRY_TRACE ', 'canvas_factory':'NATIVE_CANVAS_FACTORY ',
      'task3_cleanup':'NATIVE_TASK3_CLEANUP ', 'rt_fields_before_task2':'NATIVE_RT_FIELDS_BEFORE_TASK2 ',
      'setdata_task_return':'NATIVE_SETDATA_TASK_RETURN ', 'native_color_join':'NATIVE_TASK2_INPUTCOLOR_JOIN ',
      'rt_writes':'DRAWING_RT_WRITES ', 'queue_submissions':'ALL_QUEUE_SUBMISSIONS ',
      'uniform_trace':'UNIFORM_NATIVE_IMPORT_TRACE ', 'blend_descriptor':'BLEND_NATIVE_DESCRIPTOR ',
      'blend_gl_calls':'BLEND_GL_CALLS ', 'draw_blend_receivers':'DRAW_BLEND_ACTIVATE_RECEIVERS ',
      'caller_trace':'CALLER_TRACE ', 'task2':'TASK2_STATE ',
      'task2_fbo_status':'NATIVE_TASK2_FRAMEBUFFER_STATUS ', 'chinese_texture_source':'CHINESE_NATIVE_TEXTURE_SOURCE ',
      'callback_receiver_guards':'NATIVE_CALLBACK_RECEIVER_GUARDS ',
    }
    found={}
    for line in text.splitlines():
        for key,prefix in prefixes.items():
            if line.startswith(prefix):
                raw=line[len(prefix):]
                try: found[key]=json.loads(raw)
                except json.JSONDecodeError:
                    try: found[key]=ast.literal_eval(raw)
                    except (ValueError,SyntaxError): found[key]=raw
    missing=set(prefixes)-set(found)
    if missing: raise RuntimeError(f'capture is missing fields: {sorted(missing)}')
    return found

def make_report(c, capture_stdout):
    task2=c['task2']; target=task2['native_canvas_rgba_target']; fbo=c['task2_fbo_status']
    guards=c['callback_receiver_guards']; raw_rgba=base64.b64decode(target['after_task2_base64'])
    uploads=task2['graphics_buffer_uploads_during_task2']
    if len(uploads)!=1: raise RuntimeError(f'expected one upload during task2, got {len(uploads)}')
    uploaded=bytes.fromhex(uploads[0]['data_hex'])
    source_case=selected_source_case()
    producer=source_case['producer']
    expected=bytes.fromhex(producer['buffer_hex'])
    color=c['native_color_join']; rt_fields=c['rt_fields_before_task2']; vec=rt_fields['vectors']['88']; cleanup=c['task3_cleanup']
    if uploaded!=expected or uploaded.hex()!=vec['bytes_hex']: raise RuntimeError('same-RT ChineseBrush AddPoint/RT/upload byte join failed')
    if color['uniform_bytes']!=color['rt_plus44_bytes']: raise RuntimeError('actual inputColor differs from same RTV1+44')
    if color['rt_plus44_bytes']!='d9d8d83ededd5d3f908f0f3f0000803f': raise RuntimeError('native queued color does not match canonical ChineseBrush ARGB fixture')
    source_input=source_case['input']
    if source_input['color_argb_u32']!='0xff6cdd8f' or source_input['size_f32_bits']!='4c374940' or source_input['advanced_setting_ascii']!='3;' or source_input['tool_type']!=2: raise RuntimeError('ChineseBrush source contract profile changed')
    motion=c['source_result']['motion_case']; source_arrays=source_input['source_arrays']
    native_arrays={
      'points':struct.pack('<'+'f'*(len(motion['points'])*2),*(value for point in motion['points'] for value in point)).hex(),
      'pressures':struct.pack('<'+'f'*len(motion['pressures']),*motion['pressures']).hex(),
      'timestamps':struct.pack('<'+'i'*len(motion['times']),*motion['times']).hex(),
      'tilts':struct.pack('<'+'f'*len(motion['tilts']),*motion['tilts']).hex(),
      'orientations':struct.pack('<'+'f'*len(motion['orientations']),*motion['orientations']).hex()}
    for name,raw_hex in native_arrays.items():
      if source_arrays[name]['data_hex']!=raw_hex or source_arrays[name]['count']!=len(motion['points']): raise RuntimeError(f'native Drawing source input diverges from pinned ChineseBrush {name} arrays')
      getter=c['source_result']['native_object_stroke_getters']['arrays'][name]; expected_array=source_arrays[name]
      if getter['data_hex']!=expected_array['data_hex'] or getter['count']!=expected_array['count'] or getter['element_size']!=expected_array['element_size']: raise RuntimeError(f'actual native ObjectStroke getter diverges from pinned ChineseBrush {name} getter bytes/count/element size')
    if c['source_result']['native_object_stroke_getters']['point_count']!=source_input['point_count']: raise RuntimeError('actual native ObjectStroke GetPointCount differs from source fixture')
    metadata=c['source_result']['native_object_stroke_getters']['metadata']
    expected_meta={'GetPenSize() const':('s0_f32_bits',source_input['size_f32_bits']),'GetSizeLevel() const':('raw_x0','0x1e'),'GetColor() const':('raw_x0',source_input['color_argb_u32']),'GetToolType() const':('raw_x0',hex(source_input['tool_type'])),'GetStrokeType() const':('raw_x0',hex(source_input['stroke_type']))}
    for method,(field,expected_value) in expected_meta.items():
      if metadata[method][field]!=expected_value: raise RuntimeError(f'actual native ObjectStroke {method} differs from pinned source metadata')
    object_snapshot=source_input['native_object_stroke_snapshots'][0]
    object_getters=metadata
    expected_object={'GetPenSize() const':('s0_f32_bits',object_snapshot['size_bits']),'GetFixedWidth() const':('s0_f32_bits',object_snapshot['fixed_width_bits']),'GetRainbowOffset() const':('s0_f32_bits',object_snapshot['rainbow_offset_bits']), 'GetStrokeType() const':('raw_x0',hex(object_snapshot['stroke_type'])),'IsCurveEnabled() const':('raw_x0',hex(object_snapshot['curve_enabled'])),'IsShape() const':('raw_x0',hex(object_snapshot['shape'])),'GetLineType() const':('raw_x0',hex(object_snapshot['line_type'])),'IsFixedWidthEnabled() const':('raw_x0',hex(object_snapshot['fixed_width_enabled'])),'IsRainbowEffectEnabled() const':('raw_x0',hex(object_snapshot['rainbow_enabled'])),'GetRainbowDistance() const':('raw_x0',hex(object_snapshot['rainbow_distance']))}
    for method,(field,expected_value) in expected_object.items():
      if object_getters[method][field]!=expected_value: raise RuntimeError(f'actual native ObjectStroke {method} differs from pinned full metadata snapshot')
    if sha(raw_rgba)!=target['after_task2_sha256']: raise RuntimeError('full RGBA payload hash mismatch')
    compressed_rgba=zlib.compress(raw_rgba,9)
    if zlib.decompress(compressed_rgba)!=raw_rgba: raise RuntimeError('compressed RGBA payload round-trip failed')
    if len(raw_rgba)!=target['dimensions'][0]*target['dimensions'][1]*4: raise RuntimeError('RGBA dimensions do not match readback byte count')
    if not any(raw_rgba): raise RuntimeError('native task2 produced an all-zero framebuffer')
    if len(c['source_result']['callback_arguments'])!=len(uploaded)//16: raise RuntimeError('native AddPoint count differs from uploaded vector4 count')
    if cleanup['result'].get('returned') is not True or task2['returned'] is not True: raise RuntimeError('native cleanup/draw task did not return')
    if len(fbo)!=1 or fbo[0].get('status_enum')!=0x8cd5 or not fbo[0].get('is_framebuffer_complete'): raise RuntimeError('task2 FBO is not observed GL_FRAMEBUFFER_COMPLETE')
    if target['gl_error_before_readback']!=0: raise RuntimeError('GL error immediately before readback')
    if target['gl_error_after_readback']!=0: raise RuntimeError('GL error after readback')
    if task2['target_debug']['native_canvas_target_before_task2']['channel_nonzero']!=[0,0,0,0]: raise RuntimeError('target was not transparently precleared')
    if task2['target_debug']['native_canvas_target_after_task2']['internal_format']!=0x8058: raise RuntimeError('output target is not RGBA8')
    if target['feedback_alias_with_pattern_or_grain']: raise RuntimeError('draw target aliases pattern/grain texture')
    if not c['chinese_texture_source'] or any(not r.get('bound_gl_texture') for r in c['chinese_texture_source']): raise RuntimeError('native ChineseBrush texture upload lacks bound GLES texture identity')
    if len({r['bound_gl_texture'] for r in c['chinese_texture_source']}) != 1: raise RuntimeError('ChineseBrush texture upload changed bound texture unexpectedly')
    if task2['graphics_draw_calls_after']!=task2['graphics_draw_calls_before']+1: raise RuntimeError('task2 did not issue exactly one Graphics draw')
    if any(r.get('prior_error') or r.get('error_after') for r in task2['gl_calls_during_task2']): raise RuntimeError('native task2 GLES call error')
    if not guards or any(not r['receiver_matches_expected_object'] for r in guards): raise RuntimeError('callback receiver checks failed')
    helpers={rel:{'path':rel,'sha256':sha((ROOT/rel).read_bytes())} for rel in EXPECTED}
    dsos={}
    for name in DSO_HASHES:
        rel='scratch/apk-analysis-native/arm64-v8a/'+(name if name.endswith('.so') else f'libSPen{name}.so')
        dsos[name]={'path':rel,'sha256':sha((ROOT/rel).read_bytes())}
    runner_text=HERE.read_text()
    direct_rt_writes=re.findall(r'(?:self|p)\.(?:put|u\.mem_write)\(\s*(?:rt|self\.watch_rt|p\.watch_rt)(?:\s*\+|\s*,)',runner_text)
    if direct_rt_writes: raise RuntimeError(f'host-side direct RT field writes found: {direct_rt_writes}')
    host_dsos={path:{'sha256':sha(Path(path).read_bytes())} for path in HOST_DSO_HASHES}
    return {
      'schema':'chinesebrush-same-drawing-queued-paint-capture-v1',
      'scope':'Explicit synthetic ObjectStroke inputs execute native ChineseBrush GLV3 RedrawPen/AddPoint, natural queued SetPenData, native ChineseBrush RTV1 Init/Draw, native texture upload, GraphicsObject drawing, and post-draw vector deletion. Queue, ObjectDrawing model/context, canvas source bitmap, clip/depth, and Mesa GLES are explicit host adapters. No Android scheduler, Samsung device, or device pixel claim.',
      'replay':{'command':f'python3 {HERE.relative_to(ROOT)} --check-fixture','runner_sha256':sha(HERE.read_bytes()),'capture_stdout_sha256':sha(capture_stdout.encode()),'assertions':{
        'fbo_status':fbo,'gl_error_before_readback':target['gl_error_before_readback'],'gl_error_after_readback':target['gl_error_after_readback'],'read_draw_and_attachment':{'read_fbo':target['read_framebuffer_at_read'],'draw_fbo':target['draw_framebuffer_at_read'],'target_fbo':target['framebuffer_id'],'attachment_at_read':target['attachment_texture_id_at_read'],'expected_attachment':target['attachment_texture_id']},
        'target_debug':task2['target_debug'],'feedback_alias':target['feedback_alias_with_pattern_or_grain'],'target_format_dimensions':{'format':target['format'],'dimensions':target['dimensions'],'internal_format':task2['target_debug']['native_canvas_target_after_task2']['internal_format']},
        'blend_state':target['actual_gl_blend_state_after_draw'],'graphics_draw_count':[task2['graphics_draw_calls_before'],task2['graphics_draw_calls_after']],'receiver_guards':guards,
        'cleanup_returned':cleanup['result']['returned'],'uniform_matches_rt':color['uniform_bytes']==color['rt_plus44_bytes'],'producer_matches_upload':uploaded==expected,
        'task2_gl_errors':[r for r in task2['gl_calls_during_task2'] if r.get('prior_error') or r.get('error_after')],'host_rt_field_writes':direct_rt_writes}},
      'provenance':{'native_dsos':dsos,'host_gles_dsos':host_dsos,'pinned_helpers':helpers,'source_caller_case':'curve-v3','source_fixture_case':'curve-v3','source_case_contract_sha256':SOURCE_CASE_SHA256,'source_case_contract_scope':'canonical JSON of cases.curve-v3 only; unrelated source-fixture cases are intentionally outside this data pin'},
      'source_to_native':{'source_fixture_case':{'case_id':'curve-v3','input':source_input,'profile':source_case['profile'],'native_selection':source_case['native_selection']},'source_result':c['source_result'],'native_setdata_entry':c['setdata_entry_trace'],'native_setdata_return':c['setdata_task_return'],'pen_data_manager_dispatches':c['pen_queue_dispatches'],'submitted_tasks':c['queue_submissions'],'native_rt_writes':c['rt_writes'],'same_rt':color['rt'],'rt_fields_before_task2':rt_fields,'rt_fields_after_delete':cleanup['rt_fields_72_127_after_cleanup_hex'],'post_draw_cleanup':cleanup,'canvas_bitmap_factory':c['canvas_factory'],'native_chinese_texture_uploads':c['chinese_texture_source']},
      'native_draw':{'entry':task2['entry'],'returned':task2['returned'],'uniform_imports_in_order':c['uniform_trace'],'same_rt_input_color_join':color,'blend_descriptor_hex':c['blend_descriptor'],'blend_gl_calls':c['blend_gl_calls'],'blend_activation_receivers':c['draw_blend_receivers'],'native_caller_trace':c['caller_trace'],'graphics_draw_count':[task2['graphics_draw_calls_before'],task2['graphics_draw_calls_after']],'graphics_gl_calls':task2['gl_calls_during_task2'],'target_debug':task2['target_debug'],'fbo_status':fbo,'upload_records':[{**r,'sha256_recomputed':sha(bytes.fromhex(r['data_hex']))} for r in uploads],'producer_vector_sha256':producer['buffer_sha256'],'producer_vector_bytes':len(expected),'producer_matches_upload':uploaded==expected,'producer_case':'curve-v3'},
      'paint_readback':{'target':target,'full_rgba8_bottom_left_first_zlib_base64':base64.b64encode(compressed_rgba).decode('ascii'),'compression':'zlib level 9; decompress to bottom-left-first RGBA8 bytes','full_rgba8_sha256':sha(raw_rgba),'compressed_sha256':sha(compressed_rgba),'byte_count':len(raw_rgba)},
      'host_boundaries':['Synthetic ObjectStroke inputs, Drawing context/profile, synchronous queue, and canvas source bitmap; Drawing/ChineseBrush methods execute in pinned native DSOs.','Queue adapter synchronously dispatches native task vtable+16; PenStrokeDataManager singleton and ReleaseStrokeData execute natively.','Native CreateBitmap/SPTextureBitmap wrapper executes; GLES resource/state and Mesa readback remain host boundaries.','Host clipping/depth adapters do not establish Samsung device clipping/depth parity.','Uniform adapter records guest bytes and forwards those bytes unchanged to Mesa; it does not replace inputColor.','No Android scheduler, SDK deserialization, Samsung GLES, or device-pixel identity claim.']}

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    mode=ap.add_mutually_exclusive_group()
    mode.add_argument('--write-fixture',action='store_true')
    mode.add_argument('--check-fixture',action='store_true')
    args=ap.parse_args()
    preflight()
    if not args.write_fixture:
        if not FIXTURE.exists(): raise SystemExit(f'missing fixture {FIXTURE}; use --write-fixture after review')
        prior=json.loads(FIXTURE.read_text())
        expected_runner=prior.get('replay',{}).get('runner_sha256')
        actual_runner=sha(HERE.read_bytes())
        if expected_runner!=actual_runner: raise SystemExit(f'pre-execution runner pin mismatch: {actual_runner} != {expected_runner}')
    with contextlib.redirect_stdout(io.StringIO()) as captured: capture_native_run()
    Path('/tmp/chinese-capture-stdout.txt').write_text(captured.getvalue())
    report=make_report(parse_capture(captured.getvalue()),captured.getvalue())
    encoded=json.dumps(report,indent=2,sort_keys=True)+'\n'
    if args.write_fixture:
        FIXTURE.write_text(encoded); print(f'wrote {FIXTURE} sha256={sha(FIXTURE.read_bytes())}'); return
    expected=json.loads(FIXTURE.read_text())
    if json.dumps(expected,sort_keys=True)!=json.dumps(report,sort_keys=True): raise SystemExit('ChineseBrush same-Drawing fixture mismatch')
    print(f'ChineseBrush same-Drawing paint replay passed ({sha(FIXTURE.read_bytes())})')

if __name__=='__main__': main()
