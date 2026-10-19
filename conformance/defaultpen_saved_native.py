"""Synthetic saved ObjectStroke through shipped DefaultPen wrapper, MotionEvent, queued redraw/endPen; RT AddPoint is captured."""
import hashlib, json, math, struct, sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT/'conformance'))
import fountain_native
from fountain_native import NativeFountain, HASHES
from unicorn import arm64_const as R
LIB=ROOT/'scratch/apk-analysis-native/arm64-v8a'
HASHES['DefaultPen']='1b37056c81646c407e9e0697c78e31b50ffab0bdb896966d1e89feb02385544f'
class Oracle(NativeFountain):
 def __init__(self):
  super().__init__(LIB,ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump',('DefaultPen',))
  self.add_sentinel=self.alloc(16); self.post_sentinel=self.alloc(16); self.task_return=self.alloc(16); self.canvas_queue_sentinel=self.alloc(16); self.canvas_matrix_sentinel=self.alloc(16)
  self.add_points=[]; self.add_debug=[]; self.add_after=[]; self.redraw_trace=[]; self.repeat_trace=[]; self.end_trace=[]; self.rt=0; self.terminal_event=0; self.post_return=None; self.queue=0; self.trace=[]; self.exception_events=[]; self.error_codes=[]
  # Bind RELATIVE slots (readelf omits a symbol column) and imported GLOB_DAT
  # slots against the known pinned DSOs so WorkerTask vtables execute correctly.
  import subprocess
  demangled={}
  for idx,name in enumerate(('FountainPen','PenCommon','Base','DefaultPen')):
   base=idx*0x1000000; path=LIB/f'libSPen{name}.so'
   for line in subprocess.check_output(['readelf','-rW',str(path)],text=True).splitlines():
    cols=line.split()
    if len(cols)<4 or not cols[0].isalnum() or not cols[2].startswith('R_AARCH64_'): continue
    offset=int(cols[0],16); typ=cols[2]
    if typ=='R_AARCH64_RELATIVE':
     self.put(base+offset,'Q',base+int(cols[3],16)); continue
    if typ not in ('R_AARCH64_GLOB_DAT','R_AARCH64_ABS64') or len(cols)<5: continue
    symval=int(cols[3],16); raw=cols[4]; add=int(cols[6],16) if len(cols)>6 and cols[5] in ('+','-') else 0
    if cols[5:6]==['-']: add=-add
    if symval: target=base+symval
    else:
     if raw not in demangled: demangled[raw]=subprocess.check_output(['c++filt','-n',raw],text=True).strip()
     target=self.syms.get(demangled[raw],0)
    if target: self.put(base+offset,'Q',target+add)
  self.callbacks={k:v for k,v in self.callbacks.items() if not k.startswith('SPen::MotionEvent::')}
  self.callbacks.update({'__cxa_begin_catch':self.begin_catch,'__cxa_throw':self.throwing,'sincosf':self.sincos,'sqrtf':lambda:self.f(0,math.sqrt(self.f(0))), 'SPen::PenGLDataManager::GetMsgQueue()':lambda:self.x(0,self.queue), 'SPen::MotionEvent::~MotionEvent()':lambda:None, 'SPen::Error::SetError(unsigned long)':lambda:self.error_codes.append(self.x(0)), '__android_log_print':lambda:self.x(0,0)})
 def begin_catch(self):
  raise RuntimeError(f'unexpected native begin_catch at {self.u.reg_read(R.UC_ARM64_REG_PC):#x}, object={self.x(0):#x}')
 def throwing(self):
  raise RuntimeError(f'unexpected native throw at {self.u.reg_read(R.UC_ARM64_REG_PC):#x}, object={self.x(0):#x}, typeinfo={self.x(1):#x}')
 def sincos(self):
  a=self.f(0); self.put(self.x(0),'f',math.sin(a)); self.put(self.x(1),'f',math.cos(a))
 def hook(self,u,a,size,data):
  self.trace.append(a); self.trace=self.trace[-100:]
  if a==self.syms.get('SPen::DefaultPenStrokeDrawableGL::redrawLine(float, float)'):
   d=self.x(0); self.redraw_trace.append({'xy':[self.f(0),self.f(1)],'tolerance':self.get(d+264,'f')[0],'state':[self.get(d+i,'f')[0] for i in (88,92,96,100,104,108,112,116)],'flags':[self.get(d+i,'B')[0] for i in (80,260)]})
  if a==self.syms.get('SPen::DefaultPenStrokeDrawableGL::getRepeat(float, int)'):
   self.repeat_trace.append([self.f(0),self.x(1)])
  if a==self.syms.get('SPen::DefaultPenStrokeDrawableGL::endPen(SPen::MotionEvent const*, SPen::RectF*)'):
   d=self.x(0); self.terminal_event=self.x(1); self.end_trace.append({'event_ptr':hex(self.x(1)),'state':[self.get(d+i,'f')[0] for i in (88,92,96,100,104,108,112,116)],'flags':[self.get(d+i,'B')[0] for i in (80,260)]})
  if a==self.syms.get('SPen::DefaultPenStrokeDrawableRT::AddPoint(SPen::Vector2<float>, float)')+0x254:
   vp=self.get(self.rt+80,'Q')[0]; h=self.get(vp,'QQQ') if vp else (0,0,0); count=(h[1]-h[0])//4 if h[0] else 0; self.add_after.append([hex(v) for v in h]+[count,list(self.get(h[0],str(count)+'f')) if count else []])
  if a==self.syms.get('SPen::DefaultPenStrokeDrawableRT::AddPoint(SPen::Vector2<float>, float)'):
   self.add_points.append([self.f(0),self.f(1),self.f(2)]); self.add_points[-1].append(hex(u.reg_read(R.UC_ARM64_REG_LR))); vp=self.get(self.x(0)+80,'Q')[0]; self.add_debug.append([hex(self.x(0)),hex(vp),[hex(v) for v in self.get(vp,'QQQ')] if vp else None])
  if a==self.syms.get('SPen::DefaultPenStrokeDrawableRT::SendDataToGPU(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, float)'):
   self.x(0,1); u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR)); return
  if a==self.canvas_matrix_sentinel:
   sp=u.reg_read(R.UC_ARM64_REG_SP); u.mem_write(sp,bytes(40)); u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR)); return
  if a==self.canvas_queue_sentinel:
   self.x(0,self.queue); u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR)); return
  if a==self.post_sentinel:
   task=self.x(1); tv=self.get(task,'Q')[0]; run=self.get(tv+16,'Q')[0]
   self.post_return=u.reg_read(R.UC_ARM64_REG_LR)
   if tv==0x301f978: # PenGLRenderMsg: GPU submission boundary, geometry already captured at RT AddPoint.
    self.x(0,1); u.reg_write(R.UC_ARM64_REG_PC,self.post_return); return
   self.x(0,task); self.x(30,self.task_return); u.reg_write(R.UC_ARM64_REG_PC,run); return
  if a==self.task_return:
   self.x(0,1)
   u.reg_write(R.UC_ARM64_REG_PC,self.post_return); return
  super().hook(u,a,size,data)
 def run(self,points,pressures,times,tilts,orientations,tool=1,curve=True,width=10.0):
  n=len(points); assert n>=1 and len(pressures)==len(times)==n and len(tilts)==len(orientations)==n
  # Stack-like objects the native wrapper reads. Every ObjectStroke getter is a host boundary,
  # but the MotionEvent constructor/getters and DefaultPen redraw path execute from pinned DSOs.
  obj=self.alloc(64); rect=self.alloc(32); draw=self.alloc(512); manager=self.alloc(128); data=self.alloc(64); settings=self.alloc(64)
  self.points_ptr=self.alloc(n*8); self.pressure_ptr=self.alloc(n*4); self.time_ptr=self.alloc(n*4); self.tilt_ptr=self.alloc(n*4); self.orientation_ptr=self.alloc(n*4)
  self.put(self.points_ptr,'f'*(2*n),*(v for p in points for v in p)); self.put(self.pressure_ptr,'f'*n,*pressures); self.put(self.time_ptr,'i'*n,*times); self.put(self.tilt_ptr,'f'*n,*tilts); self.put(self.orientation_ptr,'f'*n,*orientations)
  self.callbacks.update({'SPen::ObjectStroke::GetToolType() const':lambda:self.x(0,tool),'SPen::ObjectStroke::GetPointCount() const':lambda:self.x(0,n),'SPen::ObjectStroke::GetPoint() const':lambda:self.x(0,self.points_ptr),'SPen::ObjectStroke::GetPressure() const':lambda:self.x(0,self.pressure_ptr),'SPen::ObjectStroke::GetTimeStamp() const':lambda:self.x(0,self.time_ptr),'SPen::ObjectStroke::GetTilt() const':lambda:self.x(0,self.tilt_ptr),'SPen::ObjectStroke::GetOrientation() const':lambda:self.x(0,self.orientation_ptr)})
  # Host boundaries: ObjectStroke getters and queue/context APIs are synthetic. The shipped GL/RT constructors and Base MotionEvent path execute natively.
  qvt=self.alloc(64); self.queue=self.alloc(32); self.put(self.queue,'Q',qvt); self.put(qvt+16,'Q',self.post_sentinel); self.put(manager+56,'Q',self.queue)
  self.put(data,'Q',settings); self.put(settings,'fI',width,0xff336699)
  self.call('SPen::DefaultPenStrokeDrawableGL::DefaultPenStrokeDrawableGL(SPen::DefaultPenData*, SPen::PenGLDataManager*)',[draw,data,manager])
  rt=self.get(draw+16,'Q')[0]; self.rt=rt; vector=self.alloc(24); storage=self.alloc(200000); self.put(vector,'QQQ',storage,storage,storage+200000); self.put(rt+80,'Q',vector)
  canvas=self.alloc(64); cvt=self.alloc(256); self.put(canvas,'Q',cvt); self.put(cvt+184,'Q',self.canvas_queue_sentinel); self.put(cvt+216,'Q',self.canvas_matrix_sentinel); self.put(draw+32,'Q',canvas); self.put(draw+264,'f',5.0); self.put(draw+256,'f',0.0); self.put(data+8,'B',1 if curve else 0)
  try: self.call('SPen::DefaultPenStrokeDrawableGL::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)',[draw,obj,rect],instruction_limit=8000000)
  except Exception:
   print('EXCEPTIONS',self.exception_events); print('TRACE', [hex(a) for a in self.trace], 'PC', hex(self.u.reg_read(R.UC_ARM64_REG_PC)), 'LR', hex(self.u.reg_read(R.UC_ARM64_REG_LR)), 'X0',hex(self.x(0)),'X8',hex(self.x(8)),'X19',hex(self.x(19)),'X20',hex(self.x(20))); raise
  terminal={}
  if self.terminal_event:
   for name in ('GetX','GetY'):
    self.call(f'SPen::MotionEvent::{name}(int) const',x=(self.terminal_event,0)); terminal[name]=struct.unpack('<d',struct.pack('<Q',self.u.reg_read(R.UC_ARM64_REG_D0)))[0]
   for name in ('GetToolType','GetSource','GetEventTime'):
    args=(self.terminal_event,0) if name=='GetToolType' else (self.terminal_event,)
    self.call(f'SPen::MotionEvent::{name}'+('(int)' if name=='GetToolType' else '()')+' const',x=args); terminal[name]=self.x(0)
  return {'terminal_motion':terminal,'error_codes':self.error_codes,'points':points,'pressures':pressures,'times':times,'tool':tool,'curve_enabled_host_field':curve,'width_setting':width,'add_points':self.add_points,'redraw_trace':self.redraw_trace,'repeat_trace':self.repeat_trace,'end_trace':self.end_trace,'add_after':self.add_after,'add_debug':self.add_debug,'rect':[self.get(rect+i*4,'f')[0] for i in range(4)],'output_scalar_floats':(self.get(vector+8,'Q')[0]-self.get(vector,'Q')[0])//4,'native_interleaved_first_last':([list(self.get(self.get(vector,'Q')[0],'3f')),list(self.get(self.get(vector+8,'Q')[0]-12,'3f'))] if self.get(vector+8,'Q')[0]>self.get(vector,'Q')[0] else [])}
def main():
 import argparse
 ap=argparse.ArgumentParser()
 group=ap.add_mutually_exclusive_group(required=True)
 group.add_argument('--write-fixture',action='store_true')
 group.add_argument('--check-fixture',action='store_true')
 args=ap.parse_args()
 cases=[
  ('base',[[0,0],[10,0],[20,0],[30,10]],[.25,.5,.75,1.],[10,20,30,40],[0]*4,[0]*4,1,True,10.),
  ('pressure_high',[[0,0],[10,0],[20,0],[30,10]],[1.,1.,1.,1.],[10,20,30,40],[0]*4,[0]*4,1,True,10.),
  ('pressure_low',[[0,0],[10,0],[20,0],[30,10]],[0.,0.,0.,0.],[10,20,30,40],[0]*4,[0]*4,1,True,10.),
  ('tool2',[[0,0],[10,0],[20,0],[30,10]],[.25,.5,.75,1.],[10,20,30,40],[0]*4,[0]*4,2,True,10.),
  ('tool3',[[0,0],[10,0],[20,0],[30,10]],[.25,.5,.75,1.],[10,20,30,40],[0]*4,[0]*4,3,True,10.),
  ('width20',[[0,0],[10,0],[20,0],[30,10]],[.25,.5,.75,1.],[10,20,30,40],[0]*4,[0]*4,1,True,20.),
  ('times_slow',[[0,0],[10,0],[20,0],[30,10]],[.25,.5,.75,1.],[10,1000,2000,3000],[0]*4,[0]*4,1,True,10.),
  ('curve_false',[[0,0],[10,0],[20,0],[30,10]],[.25,.5,.75,1.],[10,20,30,40],[0]*4,[0]*4,1,False,10.),
 ]
 cases += [
  ('unequal_tool1',[[0,0],[10,0],[20,0],[80,0],[90,10],[100,20]],[.1,.2,.3,.4,.5,.6],[0,10,20,30,40,50],[0]*6,[0]*6,1,True,10.),
  ('short_tool2',[[0,0],[2,0],[4,1],[6,0]],[.2,.4,.6,.8],[0,1,2,3],[0]*4,[0]*4,2,True,10.),
  ('long_tool2',[[0,0],[10,0],[25,0],[40,10],[70,5],[100,20]],[.1,.2,.3,.4,.5,.6],[0,10,20,30,40,50],[0]*6,[0]*6,2,True,10.),
  ('size11_5_tool3',[[0,0],[5,0],[16,4],[40,12]],[.1,.4,.6,.9],[0,10,20,30],[0]*4,[0]*4,3,True,11.5),
  ('all_coincident_tool2',[[0,0],[0,0],[0,0],[0,0]],[.2,.4,.6,.8],[0,1,2,3],[0]*4,[0]*4,2,True,10.),
  ('duplicate_then_move_tool2',[[0,0],[0,0],[0,0],[10,0]],[.2,.4,.6,.8],[0,1,2,3],[0]*4,[0]*4,2,True,10.),
  ('single_tap_tool2',[[0,0]],[.5],[0],[0],[0],2,True,10.),
  ('two_point_tool2',[[0,0],[10,0]],[.2,.8],[0,10],[0]*2,[0]*2,2,True,10.),
 ]
 out=[]
 for label,*case_args in cases:
  p=Oracle(); result=p.run(*case_args); result['case']=label
  assert not p.exception_events, f'unexpected exception path in {label}: {p.exception_events}'
  floats=result['add_after'][-1][4] if result['add_after'] else []
  result={'case':label,'input':{'points':case_args[0],'pressures':case_args[1],'timestamps_ms':case_args[2],'tilts':case_args[3],'orientations':case_args[4],'tool_type_raw':case_args[5],'saved_curve_field':case_args[6],'size':case_args[7]},'native':{'redraw_line_calls':result['redraw_trace'],'get_repeat_calls':result['repeat_trace'],'end_pen_state':result['end_trace'],'terminal_motion':result['terminal_motion'],'error_codes':result['error_codes'],'add_point_count':len(result['add_points']),'final_addpoint_vector_f32':floats,'triples':[floats[i:i+3] for i in range(0,len(floats),3)]}}
  out.append(result)
 probe=Oracle(); repeat_matrix=[]
 for width in (1,5,6,10,11,20,55,56):
  row=[]
  for length in (0.0,0.1,1.0,4.0,20.0):
   probe.call('SPen::DefaultPenStrokeDrawableGL::getRepeat(float, int)',x=(0,width),f=(length,)); row.append(probe.x(0))
  repeat_matrix.append({'width':width,'lengths':[0.0,0.1,1.0,4.0,20.0],'counts':row})
 result={'schema':'sdocx-defaultpen-saved-native-v1','repeat_matrix':repeat_matrix,'source':{'defaultpen_sha256':'1b37056c81646c407e9e0697c78e31b50ffab0bdb896966d1e89feb02385544f','pencommon_sha256':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d','base_sha256':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb','fountainpen_helper_sha256':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec'},'evidence_scope':'Synthetic ObjectStroke getter/channel values and synthetic queue/context APIs; native DefaultPen GL/RT constructors, saved wrapper, Base MotionEvent constructor/getters, redrawLine/endPen, SmPath sampling, and RT AddPoint execute. MotionEvent destruction is host-stubbed. Final native RT vectors are captured at AddPoint epilogues. Renderer upload/rasterization and real app admission are outside this probe.','cases':out}
 encoded=json.dumps(result,indent=2,sort_keys=True)+'\n'
 outpath=ROOT/'conformance/defaultpen-saved-native.json'
 if args.write_fixture:
  outpath.write_text(encoded); print(f'wrote {outpath}'); return
 expected=json.loads(outpath.read_text())
 if json.dumps(result,sort_keys=True)!=json.dumps(expected,sort_keys=True): raise AssertionError('native DefaultPen output differs from conformance fixture; inspect before --write-fixture')
 print('DefaultPen native oracle passed:',len(out),'synthetic saved redraw cases')

if __name__=='__main__': main()
