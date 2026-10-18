"""Hash-pinned Samsung Notes AirBrush V1 saved-redraw and pattern oracle.

This executes shipped redraw, SmPath, drawPoint, AddPoint, and texture upload
methods under Unicorn. MotionEvent history, queue scheduling, and bitmap/GPU
vtables are explicit synthetic host boundaries. It captures CPU-side native
records and uploaded pattern bytes; it does not establish GPU pixel parity.
"""
import argparse, json, math, struct, subprocess, hashlib
from pathlib import Path
import fountain_native
from fountain_native import NativeFountain
ROOT=Path(__file__).resolve().parents[1]; LIB=ROOT/'scratch/apk-analysis-native/arm64-v8a'
EXPECTED='92ebe10993ebfcb42fa3f5c55e86f6d469ccac6638421bbd71f92e0558587423'
import hashlib
assert hashlib.sha256((LIB/'libSPenAirBrushPen.so').read_bytes()).hexdigest()==EXPECTED
fountain_native.HASHES['AirBrushPen']=EXPECTED
class Oracle(NativeFountain):
 def __init__(self):
  self.streams=[]
  super().__init__(LIB,ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump',('AirBrushPen',))
  self.callbacks['sincosf']=self.sincos
  self.host_methods=[]
  self.bitmap_get=None; self.bitmap_upload=None; self.bitmap_method=None; self.bitmap_payload=None; self.bitmap_factory_args=None; self.bitmap_obj=None
  self.profile_constructors=[]
  self.relocation_traps={}
  self.cxa_guards=set()
  self.callbacks.update({'SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)':self.bitmap_factory,'malloc':lambda:self.x(0,self.alloc(self.x(0))),'operator new(unsigned long, std::nothrow_t const&)':lambda:self.x(0,self.alloc(self.x(0))),'operator new[](unsigned long, std::nothrow_t const&)':lambda:self.x(0,self.alloc(self.x(0))),'free':lambda:None,'strlen':self.strlen,'atoi':self.atoi,'__android_log_print':lambda:self.x(0,0),'__cxa_atexit':lambda:self.x(0,0),'__cxa_guard_acquire':self.cxa_guard_acquire,'__cxa_guard_release':self.cxa_guard_release,'__cxa_guard_abort':self.cxa_guard_abort})
  for constructor in ('SPen::AirBrushPenStrokeDrawableGLV1::AirBrushPenStrokeDrawableGLV1(SPen::AirBrushPenData*, SPen::PenGLDataManager*)','SPen::AirBrushPenStrokeDrawableGLV2::AirBrushPenStrokeDrawableGLV2(SPen::AirBrushPenData*, SPen::PenGLDataManager*)'):
   self.callbacks[constructor]=lambda name=constructor:self.profile_constructors.append(name)
  self.pending_queue_return=None
  self.relocate_data()
  self.callbacks.update({
   'SPen::MotionEvent::GetHistorySize() const':lambda:self.x(0,len(self.history)),
   'SPen::MotionEvent::GetHistoricalX(int, int) const':lambda:self.d(0,self.history[self.x(1)][0]),
   'SPen::MotionEvent::GetHistoricalY(int, int) const':lambda:self.d(0,self.history[self.x(1)][1]),
   'SPen::MotionEvent::GetOrientation(int) const':lambda:self.f(0,self.end.get('orientation',0.0)),
   'SPen::MotionEvent::GetDownTime() const':lambda:self.x(0,self.end.get('time',1)),
   'SPen::MotionEvent::GetButtonState() const':lambda:self.x(0,0),
   'SPen::MotionEvent::GetFlags() const':lambda:self.x(0,0),
   'SPen::Error::SetError(unsigned long)':lambda:None,
   'SPen::PenReturnCallback::PenReturnCallback(SPen::PenDrawableRT*, SPen::ISPPenCanvas*)':lambda:None,
   'SPen::PenReturnCallback::GetBuffer()':lambda:self.x(0,self.vector),
   'SPen::PenReturnCallback::SetPenRect(SPen::RectF const&)':lambda:None,
   'SPen::PenReturnCallback::~PenReturnCallback()':lambda:None,
   'SPen::PenDrawableRT::SetBuffer(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)':lambda:None,
   'SPen::PenGLDataManager::GetMsgQueue()':lambda:self.x(0,self.queue),
  })
 def relocate_data(self):
  # Resolve data relocations for real C++ vtables/typeinfo; leave PLT slots hooked by NativeFountain.
  import struct
  for index,name in enumerate(('FountainPen','PenCommon','Base','AirBrushPen')):
   blob=(LIB/('libSPen'+name+'.so')).read_bytes(); base=index*0x1000000
   shoff=struct.unpack_from('<Q',blob,40)[0]; es,n,si=struct.unpack_from('<HHH',blob,58)
   sections=[struct.unpack_from('<IIQQQQIIQQ',blob,shoff+i*es) for i in range(n)]
   shstr=sections[si]; names=blob[shstr[4]:shstr[4]+shstr[5]]
   by={}
   for sec in sections:
    end=names.find(b'\0',sec[0]); by[names[sec[0]:end].decode()]=sec
   symsec, strsec=by['.dynsym'],by['.dynstr']; strings=blob[strsec[4]:strsec[4]+strsec[5]]; symlist=[]
   for off in range(symsec[4],symsec[4]+symsec[5],symsec[9]):
    ni,info,other,shndx,val,size=struct.unpack_from('<IBBHQQ',blob,off); end=strings.find(b'\0',ni); symlist.append((strings[ni:end].decode() if ni else '',val))
   nm=subprocess.run(['scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-nm','-D','-C',str(LIB/('libSPen'+name+'.so'))],text=True,capture_output=True,check=True,env={**__import__('os').environ,'LD_LIBRARY_PATH':str(ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')})
   for line in nm.stdout.splitlines():
    parts=line.strip().split(' ',2)
    if len(parts)==3 and parts[1] in ('T','W','V','D','B','R'): self.syms[parts[2]]=base+int(parts[0],16)
   for section_name in ('.rela.dyn',):
    sec=by.get(section_name)
    if not sec: continue
    for off in range(sec[4],sec[4]+sec[5],sec[9]):
     where,info,add=struct.unpack_from('<QQq',blob,off); typ,si=info&0xffffffff,info>>32
     if typ==0x403: self.u.mem_write(base+where,struct.pack('<Q',base+add))
     elif typ==0x101:
      name=symlist[si][0]
      target=self.resolve_symbol(name)
      self.u.mem_write(base+where,struct.pack('<Q',target+add))
     elif typ==0x401 and symlist[si][1]: self.u.mem_write(base+where,struct.pack('<Q',base+symlist[si][1]))
     elif typ==0x401:
      rawname=symlist[si][0]; dem=subprocess.run(['scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-c++filt','-n',rawname],text=True,capture_output=True,check=True,env={**__import__('os').environ,'LD_LIBRARY_PATH':str(ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')}).stdout.strip()
      target=self.resolve_symbol(dem)
      self.u.mem_write(base+where,struct.pack('<Q',target))
 def resolve_symbol(self,name):
  if name in self.syms: return self.syms[name]
  dem=subprocess.run(['scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-c++filt','-n',name],text=True,capture_output=True,check=True,env={**__import__('os').environ,'LD_LIBRARY_PATH':str(ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')}).stdout.strip()
  if dem in self.syms: return self.syms[dem]
  return self.relocation_trap(dem)
 def relocation_trap(self,name):
  if name not in self.relocation_traps:
   self.relocation_traps[name]=0x20002000+len(self.relocation_traps)*4
  return self.relocation_traps[name]
 def bitmap_factory(self):
  sp=self.u.reg_read(__import__('unicorn').arm64_const.UC_ARM64_REG_SP); nameptr=self.x(3); raw=bytearray()
  for i in range(256):
   c=bytes(self.u.mem_read(nameptr+i,1))
   if c==b'\0': break
   raw+=c
  self.bitmap_factory_args={'width':self.x(1),'height':self.x(2),'name':raw.decode(errors='replace'),'data_ptr':hex(self.x(4)),'bool':self.x(5),'int':self.x(6),'bool2':self.x(7),'stack_args':[self.get(sp,'I')[0],self.get(sp+8,'I')[0]]}
  self.x(0,self.bitmap_obj)
 def strlen(self):
  pointer=self.x(0); length=0
  while length<4096 and self.u.mem_read(pointer+length,1)!=b'\0': length+=1
  if length==4096: raise RuntimeError('synthetic strlen exceeded 4096-byte bound')
  self.x(0,length)
 def atoi(self):
  pointer=self.x(0); raw=bytearray()
  while len(raw)<128 and self.u.mem_read(pointer+len(raw),1)!=b'\0': raw.extend(self.u.mem_read(pointer+len(raw),1))
  if len(raw)==128: raise RuntimeError('synthetic atoi exceeded 128-byte bound')
  try: value=int(raw.decode('ascii').strip() or '0')
  except ValueError: value=0
  self.x(0,value)
 def cxa_guard_acquire(self):
  guard=self.x(0)
  if guard in self.cxa_guards:
   self.x(0,0)
  else:
   self.cxa_guards.add(guard); self.x(0,1)
 def cxa_guard_release(self):
  self.cxa_guards.add(self.x(0))
 def cxa_guard_abort(self):
  self.cxa_guards.discard(self.x(0))
 def sincos(self):
  a=self.f(0); self.put(self.x(0),'f',math.sin(a)); self.put(self.x(1),'f',math.cos(a))
 def hook(self,u,address,size,data):
  if address in self.relocation_traps.values():
   name=next(k for k,v in self.relocation_traps.items() if v==address)
   raise RuntimeError('reached unresolved imported relocation: '+name)
  self.trace=getattr(self,'trace',[])+[address]; self.trace=self.trace[-16:]
  if address==getattr(self,'queue_post',None):
   task=self.x(1); vt=self.get(task,'Q')[0]; method=self.get(vt+16,'Q')[0]
   self.pending_queue_return=self.x(30); self.x(0,task); self.x(30,self.queue_task_return); u.reg_write(__import__('unicorn').arm64_const.UC_ARM64_REG_PC,method); return
  if address==getattr(self,'bitmap_method',None):
   u.reg_write(__import__('unicorn').arm64_const.UC_ARM64_REG_PC,u.reg_read(__import__('unicorn').arm64_const.UC_ARM64_REG_LR)); return
  if address==getattr(self,'bitmap_get',None):
   u.reg_write(__import__('unicorn').arm64_const.UC_ARM64_REG_PC,u.reg_read(__import__('unicorn').arm64_const.UC_ARM64_REG_LR)); return
  if address==getattr(self,'bitmap_upload',None):
   ptr=self.x(1); raw=bytes(u.mem_read(ptr,96*96*4)); self.bitmap_payload=raw
   u.reg_write(__import__('unicorn').arm64_const.UC_ARM64_REG_PC,u.reg_read(__import__('unicorn').arm64_const.UC_ARM64_REG_LR)); return
  if address==getattr(self,'host_method',None):
   self.host_methods.append({'x0':hex(self.x(0)),'x1':hex(self.x(1)),'s0':self.f(0)})
   self.x(0,1); u.reg_write(__import__('unicorn').arm64_const.UC_ARM64_REG_PC,u.reg_read(__import__('unicorn').arm64_const.UC_ARM64_REG_LR)); return
  if address==getattr(self,'queue_task_return',None):
   u.reg_write(__import__('unicorn').arm64_const.UC_ARM64_REG_PC,self.pending_queue_return); return
  super().hook(u,address,size,data)
 def saved_redraw(self, size=10.0, history=((0.0,0.0),(20.0,0.0)), endpoint=(40.0,0.0), tool=1, event_values=None):
  self.heap=0x10000000; obj=self.alloc(512); data=self.alloc(64); settings=self.alloc(64); rt=self.alloc(128); vtable=self.alloc(512); vector=self.alloc(24); output=self.alloc(4*8192); mgr=self.alloc(64); queuevt=self.alloc(64); queue=self.alloc(32); event=self.alloc(256); rect=self.alloc(32)
  self.queue_post=self.alloc(16); self.queue_task_return=self.alloc(16); self.host_method=self.alloc(16); self.queue=mgr # overwrite queue set below
  # A queue host object calls task vtable slot 2 (Run) synchronously to model its async render-thread boundary.
  self.queue=queue; self.put(queue,'Q',queuevt); self.put(queuevt+16,'Q',self.queue_post); self.callbacks['SPen::PenGLDataManager::GetMsgQueue()']=lambda:self.x(0,self.queue)
  self.vector=vector; self.end={'x':endpoint[0],'y':endpoint[1],'pressure':0.75,'tilt':0.0,'orientation':0.0,'tool':tool,'source':0,'time':30}; self.end.update(event_values or {}); self.history=list(history)
  self.call('SPen::AirBrushPenStrokeDrawableGLV1::AirBrushPenStrokeDrawableGLV1(SPen::AirBrushPenData*, SPen::PenGLDataManager*)',[obj,data,mgr])
  self.put(obj+16,'Q',rt); self.put(obj+24,'Q',mgr); self.put(obj+32,'Q',self.alloc(64)); self.put(obj+72,'Q',data); self.put(data,'Q',settings); self.put(settings,'fI',size,0xff336699)
  self.put(rt,'Q',vtable)
  for offset in range(8,512,8): self.put(vtable+offset,'Q',self.host_method)
  self.put(vtable+64,'Q',self.syms['SPen::AirBrushPenStrokeDrawableRTV1::AddPoint(SPen::Vector2<float>, float)']); self.put(rt+80,'Q',vector); self.put(vector,'QQQ',output,output,output+8192*4)
  self.put(obj+216,'2f',0.0,0.0); self.put(obj+244,'f',0.5); self.put(mgr,'Q',0)
  self.call('SPen::SmPath::SmPath()', [obj+80])
  try: self.call('SPen::AirBrushPenStrokeDrawableGLV1::RedrawPen(SPen::MotionEvent const*, SPen::RectF*)',[obj,event,rect])
  except Exception as e: print('FAIL',str(e),'PC',hex(self.u.reg_read(__import__('unicorn').arm64_const.UC_ARM64_REG_PC)),'LR',hex(self.u.reg_read(__import__('unicorn').arm64_const.UC_ARM64_REG_LR)),'TRACE',[hex(v) for v in self.trace]); raise
  begin,end,cap=self.get(vector,'QQQ'); assert end>=begin and end<=cap and (end-begin)%16==0
  values=self.get(begin,'f'*((end-begin)//4)); stamps=[values[i:i+4] for i in range(0,len(values),4)]
  return {'stamp_count':len(stamps),'stamps':[[f'{struct.unpack("<I",struct.pack("<f",v))[0]:08x}' for v in stamp] for stamp in stamps], 'host_render_callbacks':self.host_methods}
 def saved_redraw_v2(self, size, history, endpoint, tool, event_values, initial_position=(0.0,0.0), tolerance=0.5, counter_seed=0):
  # Unlike dispatch(), this executes the selected GLV2 constructor body and
  # keeps its actual RTV1 subobject. MotionEvent, queue, canvas and bitmap host
  # interfaces remain explicit boundaries, matching saved_redraw().
  ctor='SPen::AirBrushPenStrokeDrawableGLV2::AirBrushPenStrokeDrawableGLV2(SPen::AirBrushPenData*, SPen::PenGLDataManager*)'
  redraw='SPen::AirBrushPenStrokeDrawableGLV2::RedrawPen(SPen::MotionEvent const*, SPen::RectF*)'
  self.callbacks.pop(ctor,None)
  self.heap=0x10000000; obj=self.alloc(512); data=self.alloc(64); settings=self.alloc(64); mgr=self.alloc(64); canvas=self.alloc(64); vector=self.alloc(24); output=self.alloc(4*8192); queuevt=self.alloc(64); queue=self.alloc(32); event=self.alloc(256); rect=self.alloc(32)
  self.queue_post=self.alloc(16); self.queue_task_return=self.alloc(16); self.host_method=self.alloc(16)
  self.queue=queue; self.put(queue,'Q',queuevt); self.put(queuevt+16,'Q',self.queue_post); self.callbacks['SPen::PenGLDataManager::GetMsgQueue()']=lambda:self.x(0,self.queue)
  self.vector=vector; self.end={'x':endpoint[0],'y':endpoint[1],'pressure':0.75,'tilt':0.0,'orientation':0.0,'tool':tool,'source':0,'time':30}; self.end.update(event_values or {}); self.history=list(history)
  self.call(ctor,[obj,data,mgr])
  self.put(obj+24,'Q',mgr); self.put(obj+32,'Q',canvas); self.put(obj+72,'Q',data); self.put(data,'Q',settings); self.put(settings,'fI',size,0xff336699)
  rt=self.get(obj+16,'Q')[0]; rt_vtable=self.get(rt,'Q')[0]; expected_vtable=self.syms['vtable for SPen::AirBrushPenStrokeDrawableRTV1']
  assert rt_vtable==expected_vtable+16, (hex(rt_vtable),hex(expected_vtable))
  self.put(rt+80,'Q',vector); self.put(vector,'QQQ',output,output,output+8192*4)
  self.put(obj+216,'2f',*initial_position); self.put(obj+232,'I',counter_seed); self.put(obj+244,'f',tolerance)
  self.call(redraw,[obj,event,rect])
  begin,end,cap=self.get(vector,'QQQ'); assert end>=begin and end<=cap and (end-begin)%16==0
  values=self.get(begin,'f'*((end-begin)//4)); stamps=[values[i:i+4] for i in range(0,len(values),4)]
  packed=b''.join(struct.pack('<f',v) for row in stamps for v in row)
  records=[[f'{struct.unpack("<I",struct.pack("<f",v))[0]:08x}' for v in stamp] for stamp in stamps]
  # Build and upload through the exact RT object created by GLV2's constructor.
  bitmap=self.alloc(64); bitmap_vtable=self.alloc(256); self.bitmap_method=self.alloc(16); self.bitmap_get=self.alloc(16); self.bitmap_upload=self.alloc(16); self.bitmap_obj=bitmap
  self.put(bitmap,'Q',bitmap_vtable)
  for offset in range(0,256,8): self.put(bitmap_vtable+offset,'Q',self.bitmap_method)
  self.put(bitmap_vtable+168,'Q',self.bitmap_get); self.put(bitmap_vtable+48,'Q',self.bitmap_upload)
  self.call('SPen::AirBrushPenStrokeDrawableRTV1::buildTextures(int, int)',[rt,96,96])
  assert self.get(rt+224,'Q')[0]==bitmap
  factory=self.bitmap_factory_args
  self.call('SPen::AirBrushPenStrokeDrawableRTV1::setTextures(unsigned int, unsigned int)',[rt,96,96])
  texture=self.bitmap_payload; assert len(texture)==96*96*4
  return {'constructor':'SPen::AirBrushPenStrokeDrawableGLV2','constructor_executed_natively':True,'drawable_profile_version':2,'rt_subobject':'SPen::AirBrushPenStrokeDrawableRTV1','rt_object_offset':'GLV2+0x10','rt_vtable_address_point':hex(rt_vtable),'rt_vtable_symbol':hex(expected_vtable),'stamp_count':len(stamps),'stamps':records,'stamp_stream_sha256':hashlib.sha256(packed).hexdigest(),'texture_build_factory':factory,'texture_upload':{'dimensions':[96,96],'bytes':len(texture),'sha256':hashlib.sha256(texture).hexdigest(),'sample_red_bytes':[texture[(48*96+x)*4] for x in range(44,52)],'other_channels_nonzero':any(texture[i] for i in range(1,len(texture),4)) or any(texture[i] for i in range(2,len(texture),4)) or any(texture[i] for i in range(3,len(texture),4))}}
 def texture_build_witness(self):
  self.heap=0x10000000; rt=self.alloc(512); manager=self.alloc(64); queue=self.alloc(32); qvt=self.alloc(64); bmp=self.alloc(64); bvt=self.alloc(256); self.bitmap_method=self.alloc(16); self.bitmap_obj=bmp; self.bitmap_factory_args=None
  self.put(rt+136,'Q',manager); self.put(manager,'Q',0); self.put(queue,'Q',qvt); self.put(qvt+16,'Q',self.bitmap_method)
  self.put(bmp,'Q',bvt)
  for o in range(0,256,8): self.put(bvt+o,'Q',self.bitmap_method)
  self.callbacks['SPen::PenGLDataManager::GetMsgQueue()']=lambda:self.x(0,queue)
  try: self.call('SPen::AirBrushPenStrokeDrawableRTV1::buildTextures(int, int)',[rt,96,96])
  except Exception as e: print('TEXTURE_FAIL',repr(e),hex(self.u.reg_read(__import__('unicorn').arm64_const.UC_ARM64_REG_PC)),[hex(v) for v in self.trace]); raise
  return self.bitmap_factory_args
 def texture_witness(self):
  self.heap=0x10000000; rt=self.alloc(512); bitmap=self.alloc(64); vt=self.alloc(256); self.bitmap_get=self.alloc(16); self.bitmap_upload=self.alloc(16); self.put(rt+224,'Q',bitmap); self.put(bitmap,'Q',vt); self.put(vt+168,'Q',self.bitmap_get); self.put(vt+48,'Q',self.bitmap_upload)
  self.call('SPen::AirBrushPenStrokeDrawableRTV1::setTextures(unsigned int, unsigned int)',[rt,96,96])
  return {'texture_dimensions':[96,96],'packed_pixel_count':len(self.bitmap_payload)//4,'payload_bytes':len(self.bitmap_payload),'payload_sha256':hashlib.sha256(self.bitmap_payload).hexdigest(),'sample_red_bytes':[self.bitmap_payload[(48*96+x)*4] for x in range(44,52)],'other_byte_channels_nonzero':any(self.bitmap_payload[i] for i in range(1,len(self.bitmap_payload),4)) or any(self.bitmap_payload[i] for i in range(2,len(self.bitmap_payload),4)) or any(self.bitmap_payload[i] for i in range(3,len(self.bitmap_payload),4))}
 def pen_default_version(self):
  self.heap=0x10000000; pen=self.alloc(512); self.call('SPen::Pen::Pen()', [pen]); self.call('SPen::Pen::getVersion()', [pen]); return self.x(0)
 def dispatch(self,version=None):
  self.heap=0x10000000; pen=self.alloc(512); manager=self.alloc(64)
  self.call('SPen::AirBrushPen::AirBrushPen()', [pen])
  self.call('SPen::Pen::getVersion()',[pen]); initial=self.x(0)
  if version is not None: self.put(pen+88,'I',version)
  self.put(pen+8,'Q',manager)
  self.call('SPen::AirBrushPen::GetStrokeDrawableGL()', [pen])
  return {'constructor_version':initial,'input_pen_version':initial if version is None else version,'version_field_override':version is not None,'selected_native_constructor':self.profile_constructors[-1],'returned_ptr':hex(self.x(0))}
 def drawline(self):
  self.heap=0x10000000
  obj=self.alloc(512); data=self.alloc(64); settings=self.alloc(64); rt=self.alloc(128); vtable=self.alloc(128); vector=self.alloc(24); output=self.alloc(4096*4)
  self.put(obj+16,'Q',rt); self.put(obj+72,'Q',data); self.put(data,'Q',settings); self.put(settings,'f',10.0)
  self.put(obj+80,'Q',0) # Path storage is embedded at obj+80; initialize below
  path=obj+80; self.call('SPen::SmPath::SmPath()', [path])
  self.put(obj+216,'2f',0.0,0.0); self.put(obj+244,'f',0.5)
  self.put(rt,'Q',vtable); self.put(vtable+64,'Q',self.syms['SPen::AirBrushPenStrokeDrawableRTV1::AddPoint(SPen::Vector2<float>, float)'])
  self.put(rt+80,'Q',vector); self.put(vector,'QQQ',output,output,output+4096*4)
  self.call('SPen::AirBrushPenStrokeDrawableGLV1::drawLine(float, float)',[obj], [20.0,0.0])
  begin,end,cap=self.get(vector,'QQQ'); assert end>=begin and end<=cap and (end-begin)%16==0
  values=self.get(begin,'f'*((end-begin)//4)); stamps=[values[i:i+4] for i in range(0,len(values),4)]
  return {'stamp_count':len(stamps),'stamps_interleaved_xy_sin_cos':stamps,'raw_call':'V1 drawLine from previous (0,0) to (20,0), AirBrush size 10'}
def capture():
 library=(LIB/'libSPenAirBrushPen.so').read_bytes()
 texture=Oracle()
 shaders={}
 for symbol in ('SPen::AirBrushPenStrokeDrawableShader::szVertexShader','SPen::AirBrushPenStrokeDrawableShader::szFragmentShader'):
  address=texture.syms[symbol]-3*0x1000000
  end=library.index(b'\0',address)
  raw=library[address:end]
  shaders[symbol]={'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw),'source':raw.decode('ascii')}
 cases=[]
 inputs=(
  {'id':'stylus-straight-size10','tool':1,'size':10.0,'history':[(0.,0.),(20.,0.)],'endpoint':(40.,0.)},
  {'id':'stylus-diagonal-size6','tool':1,'size':6.0,'history':[(0.,0.),(12.,9.)],'endpoint':(24.,18.)},
  {'id':'tool2-offset-size14','tool':2,'size':14.0,'history':[(-5.,4.),(7.,4.)],'endpoint':(19.,4.)},
 )
 for case in inputs:
  case['event']={'pressure':0.75,'tilt':0.0,'orientation':0.0,'source':0,'time':30}
  oracle=Oracle(); record=oracle.saved_redraw(case['size'],case['history'],case['endpoint'],case['tool'],case['event'])
  cases.append({'input':case,'runtime_context':{'drawable_profile_version':1,'data_size':case['size'],'data_color_argb':0xff336699,'previous_position':[0.0,0.0],'drawline_tolerance':0.5,'counter_seed':0},'native':record})
 v2_cases=[]
 for case in cases:
  context=case['runtime_context']; inp=case['input']; oracle=Oracle()
  native=oracle.saved_redraw_v2(inp['size'],inp['history'],inp['endpoint'],inp['tool'],inp['event'],context['previous_position'],context['drawline_tolerance'],context['counter_seed'])
  assert native['stamps']==case['native']['stamps'], 'V2 saved redraw differs from V1: '+inp['id']
  native.pop('stamps')
  v2_cases.append({'input_id':inp['id'],'runtime_context':{**context,'drawable_profile_version':2},'native':native,'matches_v1_records_bitwise':True})
 default_v2_selection=texture.dispatch()
 assert default_v2_selection['constructor_version']==2 and not default_v2_selection['version_field_override']
 return {
  'schema':'sdocx.airbrush-saved-native.v1',
  'source':{'apk':'Samsung Notes 4.4.45.37','library':'libSPenAirBrushPen.so','sha256':EXPECTED},
  'supporting_libraries':{'libSPenPenCommon.so':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d','libSPenBase.so':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb'},
  'native_context':{'Pen::Pen_default_version':texture.pen_default_version(),'AirBrushPen::AirBrushPen_default_version':default_v2_selection['constructor_version'],'default_GetStrokeDrawableGL_selection':default_v2_selection,'version_dispatch':[texture.dispatch(1),texture.dispatch(2)],'profile_constructor_boundary':'Pen::Pen and AirBrushPen constructors plus Pen::getVersion execute natively; the default version-2 GetStrokeDrawableGL route selects the V2 constructor; V1 selection was separately observed after an explicit synthetic pen-version field override; saved-redraw cases execute the corresponding GL constructor body directly. No named application setter is established.'},
  'default_v2_saved_redraw':{'profile_version':2,'record_storage':'per-case SHA-256 plus exact equality against the full explicit V1 records; V2 records are omitted to avoid duplicate arrays','rt_material_path':'actual GLV2 constructor owns an actual RTV1 RT subobject; actual RTV1 buildTextures and setTextures execute on that object','cases':v2_cases},
  'texture':{'build_factory':texture.texture_build_witness(),**texture.texture_witness(),'pixel_bytes_order':'little-endian uint32; observed R varying, G/B/A zero','shader_contract':'fragment preserves input RGB and multiplies input alpha by sampled pattern red','native_shader_resources':shaders},
  'host_stubs':{'motion_event':['GetHistorySize','GetHistoricalX','GetHistoricalY','GetX','GetY','GetToolType','GetPressure','GetTilt','GetOrientation','GetSource','GetEventTime','GetDownTime','GetButtonState','GetFlags'],'redraw':['queue task Run synchronous','PenGLDataManager::GetMsgQueue','PenReturnCallback constructor/destructor/GetBuffer/SetPenRect','PenDrawableRT::SetBuffer','Error::SetError','synthetic RT virtual slots'],'texture':['SPGraphicsFactory::CreateBitmap','synthetic bitmap methods','setTextures upload capture'],'runtime':['operator new/new[] and nothrow overloads use bounded Unicorn arena','malloc uses bounded Unicorn arena; free is no-op','memcpy/memmove/memset operate only in emulated memory','strlen bounded to 4096 bytes; atoi bounded to 128 bytes','sincosf uses host math.sin/math.cos','Android logging/cxa_atexit are stubs; cxa guards follow single-thread first-acquire then initialized behavior']},
  'boundaries':['synthetic MotionEvent historical/current coordinate callbacks listed per case','synthetic manager and queue host; queue task Run invoked synchronously','synthetic RT vtable methods except actual RTV1::AddPoint','synthetic bitmap factory and bitmap vtable; actual RTV1::setTextures provides upload bytes','bounded strlen/atoi and allocator host callbacks; unresolved ABS64/GLOB_DAT imports trap if reached','ELF process loader/init_array, Android runtime, GPU context, shader execution, framebuffer and raster output are not run'],
  'cases':cases,
 }

def main():
 parser=argparse.ArgumentParser(); parser.add_argument('--fixture',default='conformance/airbrush-saved-native.json'); group=parser.add_mutually_exclusive_group(required=True); group.add_argument('--write-fixture',action='store_true'); group.add_argument('--check-fixture',action='store_true'); args=parser.parse_args()
 result=capture(); path=Path(args.fixture); encoded=json.dumps(result,indent=2)+'\n'
 if args.check_fixture:
  if not path.exists() or path.read_text()!=encoded: raise SystemExit('AirBrush native fixture mismatch: '+str(path))
  print('AirBrush native fixture matches '+str(path))
 else:
  path.write_text(encoded); print('wrote '+str(path))

if __name__=='__main__': main()
