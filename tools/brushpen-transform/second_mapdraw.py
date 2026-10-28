from pathlib import Path
exec(Path(__file__).with_name('long_probe.py').read_text().split("for n in ['SPen::ObjectDrawing")[0])
import brushpen_paint_positive_native as positive
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
 obj,impl=self.alloc(0x100),self.alloc(0x400)
 self.call('SPen::ObjectStroke::ObjectStroke()', [obj])
 self.call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)',[impl,obj]);self.put(obj+32,'Q',impl)
 self.current_motion=case; self.current_object={'points':case['points'],'tool_type':case['tool_type'],'points_ptr':0,'pressures_ptr':0,'times_ptr':0,'tilts_ptr':0,'orientations_ptr':0}
 self.call('SPen::ObjectStroke::SetPenSize(float)',[obj],[7.0]);self.call('SPen::ObjectStroke::SetSizeLevel(int)',[obj,30]);self.call('SPen::ObjectStroke::SetColor(unsigned int)',[obj,0x806e42d1]);self.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj,case['tool_type']]);self.call('SPen::ObjectStroke::SetStrokeType(SPen::StrokeType)',[obj,0]);self.call('SPen::ObjectStroke::SetCurveEnabled(bool)',[obj,1])
 n=len(case['points']);pts=self.alloc(n*8);prs=self.alloc(n*4);ts=self.alloc(n*4);tilt=self.alloc(n*4);ori=self.alloc(n*4)
 self.put(pts,'f'*(n*2),*(v for xy in case['points'] for v in xy));self.current_object.update(points_ptr=pts,pressures_ptr=prs,times_ptr=ts,tilts_ptr=tilt,orientations_ptr=ori);self.put(prs,'f'*n,*case['pressures']);self.put(ts,'i'*n,*case['times']);self.put(tilt,'f'*n,*case['tilts']);self.put(ori,'f'*n,*case['orientations'])
 self.call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj,pts,prs,ts,tilt,ori,n]);self.call('SPen::ObjectBase::SetVisibility(bool)',[obj,1])
 pname=self.alloc(32);self.call('SPen::String::String()',[pname]);self.call('SPen::String::Construct()',[pname]);c=self.alloc(80);self.u.mem_write(c,b'com.samsung.android.sdk.pen.pen.preload.BrushPen\0');self.call('SPen::String::Set(char const*)',[pname,c]);self.call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj,pname])
 adv=self.alloc(32);self.call('SPen::String::String()',[adv]);self.call('SPen::String::Construct()',[adv]);c=self.alloc(8);self.u.mem_write(c,b'\0');self.call('SPen::String::Set(char const*)',[adv,c]);self.call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[obj,adv])
 pen=self.paint_state['pen']; native_canvas=self.native_canvas['object']; print('CALL_SET_IPEN',hex(drawing),hex(native_canvas),hex(pen))
 self.call('SPen::ObjectDrawing::setIPenBitmap(SPen::ISPPenCanvas*, SPen::IPen*, SPen::StrokeType)',[drawing,native_canvas,pen,0],instruction_limit=5_000_000); print('SET_IPEN_RETURN',hex(self.x(0)))
 rect=self.alloc(32)
 before=len(self.callback_records); print('CALL_DRAWOBJECT')
 self.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[drawing,native_canvas,native_canvas,obj,1],instruction_limit=5_000_000)
 rows=self.callback_records[before:]
 self.drawing_source_result={'label':'same-drawing-probe','callback_arguments':rows,
  'draw_result':self.x(0),'object':hex(obj),'drawing':hex(drawing),'canvas':hex(native_canvas),
  'motion_case':case,'callback_count':len(rows)}
 return self.drawing_source_result

from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_INVALID
watched=[]
for name,address in p.syms.items():
 if any(token in name for token in ('BrushPenStrokeDrawableRTV1::Init','BrushPenStrokeDrawableRTV1::Update','BrushPenStrokeDrawableRTV1::Draw(','BrushPenStrokeDrawableGLV10::RedrawPen','PenGLRenderMsg::','SPPenCanvas::GetBitmap','SPTextureBitmap::Get','GetWritable','GraphicsObject::DrawInstanced','Rect::Height() const','PenDrawableRT::SetPenData(float, int)','Pen::SetColor(int)')):
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
if __import__('os').environ.get('BRUSHPEN_NATIVE_PEN_DATA_MANAGER') == '1':
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
 if p.runtime_phase == 'native PenGLRenderMsg task2' and loc == 4:
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
source=textwrap.dedent(inspect.getsource(q.HandoffProbe.run_handoff)).replace('initial = super().run()', 'initial = positive.PositiveProbe.run(self)').replace('len(raw) != 26 * 44', 'len(raw) != 221 * 44')
old="""    if callback_rt != [hex(rt)]:
        raise RuntimeError(f'AddPoint callbacks used unexpected RTV1 receiver(s): {callback_rt}')"""
new="""    if len(callback_rt) != 1:
        raise RuntimeError(f'expected one Drawing-created callback RTV1: {callback_rt}')
    rt = int(callback_rt[0], 16)
    self.watch_rt = rt
    self.paint_state['rt'] = rt"""
assert old in source
source=source.replace(old,new)
anchor_matrix = "    canvas, canvas_vtable = actual_canvas, native_canvas_vptr"
insert_matrix = anchor_matrix + """
    matrix = self.alloc(36)
    self.put(matrix, '9f', *[float(x) for x in __import__('os').environ.get('BRUSHPEN_CANVAS_MATRIX','1,0,0,0,1,0,0,0,1').split(',')])
    self.call('SPen::SPPenCanvas::SetMatrix(SPen::Matrix3<float> const&)', [actual_canvas, matrix])
"""
assert anchor_matrix in source
source=source.replace(anchor_matrix, insert_matrix)


canvas_task_code = """    if __import__('os').environ.get('BRUSHPEN_RUN_CREATE_CANVAS_TASK') == '1':
        canvas_task = next(row for row in self.queue_submissions
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
    task1 = next(row for row in self.queue_submissions if row['words'][3] == self.syms["""
anchor = "    task1 = next(row for row in self.queue_submissions if row['words'][3] == self.syms["

source=source.replace("        task2_result.update({'entry': hex(entry2)", "        task2_result.update({'entry': hex(entry2)")
# Save the exact native task2 trace into the probe before the later target assertions.
source=source.replace("        task2_result.update({'entry': hex(entry2),", "        task2_result.update({'entry': hex(entry2),")
if "    self.x(0, task2['task'])" not in source:
 raise RuntimeError('task2 callsite injection anchor missing')
source=source.replace("    self.x(0, task2['task'])", """    before_init = bytes(self.u.mem_read(rt, 0x130)).hex()
    print('DRAWING_RT_RAWCOLOR_BEFORE_INIT', bytes(self.u.mem_read(rt+44,16)).hex())
    try:
        self.runtime_phase = 'explicit native Init on Drawing-created RT'
        if __import__('os').environ.get('BRUSHPEN_RUN_CREATE_CANVAS_TASK') != '1':
            self.call('SPen::BrushPenStrokeDrawableRTV1::Init()', [rt], instruction_limit=5_000_000)
        print('DRAWING_RT_INIT_RETURNED', hex(rt), bytes(self.u.mem_read(rt, 0x130)).hex())
        print('DRAWING_RT_RAWCOLOR_AFTER_INIT', bytes(self.u.mem_read(rt+44,16)).hex())
        if __import__('os').environ.get('BRUSHPEN_EXPLICIT_COLOR') == '1':
            source_object = int(source['object'], 16)
            self.call('SPen::ObjectStroke::GetColor() const', [source_object])
            source_argb = self.x(0)
            self.call('SPen::ObjectStroke::GetPenSize() const', [source_object])
            source_size = self.f(0)
            color_before_native_setter = bytes(self.u.mem_read(rt+44,16)).hex()
            self.call('SPen::PenDrawableRT::SetPenData(float, int)', [rt, source_argb], [source_size])
            color_after_native_setter = bytes(self.u.mem_read(rt+44,16)).hex()
            print('EXPLICIT_NATIVE_COLOR_HANDOFF', json.dumps({'source_object':hex(source_object),
                'getter_argb':hex(source_argb),'getter_size_f32':source_size,
                'receiver_rt':hex(rt),'bytes_before':color_before_native_setter,
                'bytes_after':color_after_native_setter,
                'setter_address':hex(self.syms['SPen::PenDrawableRT::SetPenData(float, int)'])}))
    except Exception as init_error:
        print('DRAWING_RT_INIT_FAILED', type(init_error).__name__, str(init_error), 'pc', hex(self.u.reg_read(R.UC_ARM64_REG_PC)), 'lr', hex(self.u.reg_read(R.UC_ARM64_REG_LR)), 'before', before_init, 'after', bytes(self.u.mem_read(rt, 0x130)).hex())
        raise
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
start_stream=source.index("    saved = json.loads(GEOMETRY.read_text())")
end_stream=source.index("    task2 = next(row", start_stream)
source=source[:start_stream]+"    saved_caller = json.loads((Path('/tmp/brushpen-second-geometry.json')).read_text())\n    expected_case = {'synthetic_input': json.loads((ROOT / 'tmp/conformance-cleanup/native-capture-a241793/conformance/brushpen-handoff-native.json').read_text())['input']['motion']}\n    expected_bytes = bytes.fromhex(saved_caller['producer']['buffer_hex'])\n    if raw != expected_bytes:\n        raise RuntimeError(f'Drawing-produced stream differs from pinned saved-caller fixture: {sha256(raw)}')\n\n"+source[end_stream:]

# Keep HandoffProbe's read/draw FBO, feedback, target-format, blend-enabled,
# and draw-count guards intact; the natural queue path now satisfies them.
# The injected Drawing producer returns AddPoint records rather than the
# helper's admission dictionary, so do not invent a numeric version field.
source=source.replace("'constructor_selected_version': source['admission']['actual_getVersion']",
    "'constructor_selected_version': 'GLV10 RedrawPen receiver observed on same RTV1'")
# Recompute the inherited receiver guard against the actual callback RT. Its
# hook ran before AddPoint revealed this Drawing's RT, so watch_rt still named
# the unrelated initial-control RT at that point.
old2="""    if not self.native_call_entries or any(not row['receiver_matches_expected_object']
                                           for row in self.native_call_entries):
        raise RuntimeError(f'native callback/task receiver identity mismatch: {self.native_call_entries}')"""
new2="""    callback_ctor_rows=[row for row in self.native_call_entries
        if row['method']=='PenReturnCallback::constructor']
    if len(callback_ctor_rows)!=1 or callback_ctor_rows[0]['x0_x3'][1]!=hex(rt):
        raise RuntimeError(f'PenReturnCallback did not carry actual Drawing RTV1: {callback_ctor_rows}')
    callback_this=callback_ctor_rows[0]['receiver']
    for row in self.native_call_entries:
        method=row['method']
        if method=='PenReturnCallback::constructor':
            matches=row['x0_x3'][1]==hex(rt)
        elif method in ('PenReturnCallback::GetBuffer','PenReturnCallback::SetPenRect',
                      'PenReturnCallback::destructor'):
            matches=row['receiver']==callback_this
        elif method in ('PenDrawableRT::SetBuffer','PenDrawableRT::SendDataToGPU'):
            matches=row['receiver']==hex(rt)
        else:
            matches=row['receiver_matches_expected_object']
        if not matches:
            raise RuntimeError(f'native callback/task receiver identity mismatch: {row}')"""
if old2 not in source:
 raise RuntimeError('inherited callback receiver guard anchor missing')
source=source.replace(old2,new2,1)
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
if __import__('os').environ.get('BRUSHPEN_RUN_CREATE_CANVAS_TASK') == '1':
 if anchor not in source:
  Path('/tmp/brushpen-source-dump.py').write_text(source)
  raise RuntimeError('task1 selection insertion anchor missing')
 source=source.replace(anchor,canvas_task_code,1)
 Path('/tmp/brushpen-source-after-injection.py').write_text(source)
ns=q.__dict__.copy();ns['ROOT']=ROOT;ns['OBJECT_CASE']={'name':'long-synthetic','points':[[125.,30.],[118.,44.],[104.,59.],[87.,66.],[72.,59.],[63.,43.],[70.,29.],[86.,23.]],'tool_type':2,'pressures':[.25,.4,.55,.7,.85,.75,.5,.3],'times':[300,315,330,345,360,375,390,405],'tilts':[.12,.15,.2,.25,.3,.28,.22,.15],'orientations':[.2,.25,.3,.35,.4,.45,.5,.55]};exec(compile(source,'<scratch-rebound-HandoffProbe>','exec'),ns)
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
     and row.get('uniform4fv_location')==4),None)
 if task2_color is None:
  raise RuntimeError('native task2 glUniform4fv(location=4) was not captured')
 task2_target=p.last_task2_result['native_canvas_rgba_target']
 task2_target['uniform_values_after_draw']['inputColor']=task2_color['values']
 task2_target['input_color_f32']=task2_color['values']
 task2_target['input_color_state_provenance']=(
     'Captured native task2 glUniform4fv(location=4) raw bytes; asserted byte-identical to same Drawing RTV1+44 after natural SetPenData.')
 print('SAME_DRAWING_RESULT',json.dumps({'rt':result.get('native_selection',{}).get('rt_address') if isinstance(result,dict) else None,'task_trace':result.get('queue',{}).get('order') if isinstance(result,dict) else None},sort_keys=True))
 print('NATIVE_SOURCE_RESULT',json.dumps(getattr(p,'drawing_source_result',{}),sort_keys=True))
 print('NATIVE_PEN_QUEUE_DISPATCHES',json.dumps(pen_queue_dispatches,sort_keys=True))
 print('NATIVE_SETDATA_ENTRY_TRACE',json.dumps(setdata_entries,sort_keys=True))
 print('NATIVE_CANVAS_FACTORY',json.dumps(getattr(p,'canvas_bitmap_factory_record',{}),sort_keys=True))
 print('NATIVE_TASK3_CLEANUP',json.dumps(getattr(p,'native_task3_capture',{}),sort_keys=True))
 print('NATIVE_RT_FIELDS_BEFORE_TASK2',json.dumps(getattr(p,'native_rt_fields_before_task2',{}),sort_keys=True))
 print('NATIVE_TASK2_FRAMEBUFFER_STATUS',json.dumps(task2_status,sort_keys=True))
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
