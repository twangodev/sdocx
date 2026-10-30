#!/usr/bin/env python3
"""Bounded synthetic ObjectList/Pencil2 probe using the pinned geometry adapter."""
import hashlib, importlib.util, os, subprocess, tempfile
from pathlib import Path
import sys
if sys.flags.optimize:
 raise SystemExit('Run without Python optimization; capture requires assertions')
root=Path(__file__).resolve().parents[2]
base=root/'tools/pencil2-page-v219/native/tmp/vector-parity-plan/pen-family-audit/pencil2-v14-geometry-v12/capture_candidate.py'
assert hashlib.sha256(base.read_bytes()).hexdigest()=='996ef6db3a376619e9e5d1d05b7d5e1a940726f3feb2ef6895275a705576ee3e'
spec=importlib.util.spec_from_file_location('capture',base); m=importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
m.check_pins()
source=m.adapt_source({**m._CONTROL_BASELINE,'case_id':'list_probe'})
anchor=" try: p.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[p.drawing,p.draw_canvas,p.pen_canvas,obj,1],limit=5000000)"
assert source.count(anchor)==1
insert='''
 # Synthetic bitmap and ObjectList; native Model and Drawing code execute.
 p.callbacks.pop('SPen::ObjectBase::GetType() const',None)
 p.callbacks.pop('SPen::ObjectBase::IsVisible() const',None)
 uc=p.alloc(80); seq=[0]
 def uuid_create(): p.put(p.x(0),'Q',uc); p.x(0,0)
 def uuid_export():
  seq[0]+=1; p.u.mem_write(p.get(p.x(2),'Q')[0],f'00000000-0000-4000-8000-{seq[0]:012x}\\0'.encode()); p.x(0,0)
 p.callbacks['uuid_create']=uuid_create; p.callbacks['uuid_export']=uuid_export
 p.callbacks['uuid_make']=lambda:p.x(0,0); p.callbacks['uuid_destroy']=lambda:p.x(0,0)
 p.callbacks['clock_gettime']=lambda:(p.put(p.x(1),'qq',0,0),p.x(0,0))
 p.call('SPen::ObjectBase::Construct(SPen::Object::Type, bool)',[obj,1,0]); p.call('SPen::ObjectBase::SetVisibility(bool)',[obj,1]); p.f(0,1.0); p.call('SPen::ObjectBase::SetAlpha(float)',[obj])
 ol=p.alloc(256);p.call('SPen::ObjectList::ObjectList()',[ol]);p.call('SPen::ObjectList::Construct()',[ol]);p.call('SPen::ObjectList::Add(SPen::ObjectBase const*)',[ol,obj]);p.call('SPen::ObjectList::GetCount() const',[ol]); print('LIST_COUNT',p.x(0),_always=True)
 p.u.mem_write(0x200f2500,bytes.fromhex('000080d2c0035fd6'));holder=p.alloc(32);hvt=p.alloc(128);p.put(holder,'Q',hvt);p.put(hvt+32,'Q',0x200f2500);p.put(hvt+40,'Q',0x200f2500);p.callbacks['SPen::RendererFactory::GetContextHolder()']=lambda:p.x(0,holder)
 p.put(p.bitmap_vtable+72,'Q',0x200f20e0);p.put(p.bitmap_vtable+80,'Q',0x200f20f0)
 matrix=p.alloc(36);p.put(matrix,'9f',1,0,0,0,-1,0,6,183,1)
 events=[]
 names=['SPen::ObjectList::GetData(long) const','SPen::ObjectDrawing::drawObject(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectBase const*, SPen::Rect const*)','SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)','SPen::Pencil2StrokeDrawableGLV14::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)']
 addrs={p.syms[n]:n for n in names if n in p.syms}
 def trace_list(u,a,z,d):
  if a in addrs: events.append([addrs[a],*[hex(p.x(i)) for i in range(4)]])
 p.u.hook_add(UC_HOOK_CODE,trace_list)
 try:
  p.call('SPen::ObjectDrawing::DrawObjectList(SPen::ISPBitmap*, SPen::ObjectList const*, SPen::Rect const*, SPen::Matrix3<float> const*, bool, std::__ndk1::function<bool (int, int)>*)',[p.drawing,p.bitmap,ol,0,matrix,0,0],limit=50000000)
  print('LIST_RETURN',p.x(0),_always=True)
 except Exception as e: print('LIST_FAILURE',repr(e),hex(p.u.reg_read(R.UC_ARM64_REG_PC)),_always=True)
 print('LIST_EVENTS',json.dumps(events),_always=True)
 print('LIST_OBSERVATIONS',json.dumps({'set_matrix_calls':p.set_matrix_calls,'addpoint_calls':len(pencil_buffer_events),'matrix':[1,0,0,0,-1,0,6,183,1]}),_always=True)
 raise SystemExit(0)
'''
source=source.replace(anchor,insert+anchor)
env=os.environ.copy();env['PYTHONPATH']=str(m.PINNED_PYTHONPATH)+':'+str(m.PROBE.parent);env['PATH']=str(m.PINNED_TOOLBIN)+':/usr/bin:/bin';env['LD_LIBRARY_PATH']=str(m.PINNED_TOOL_LIBS)
with tempfile.NamedTemporaryFile(mode='w',suffix='.py') as f:
 f.write("PENCIL2_OUTPUT_DIR='/tmp'\nPROFILE_EVIDENCE=None\n__file__="+repr(str(m.PROBE))+"\n"+source);f.flush();run=subprocess.run(['/usr/bin/python3',f.name,'--print-capture','--advanced-setting','14;','--width','192','--height','192'],env=env,cwd=root,text=True,capture_output=True)
 assert run.returncode==0 and 'LIST_COUNT 1' in run.stdout and 'LIST_FAILURE UcError() 0x307c554' in run.stdout and '"addpoint_calls": 84' in run.stdout and '"set_matrix_calls": 2' in run.stdout, (run.stdout,run.stderr)
 print(run.stdout)
