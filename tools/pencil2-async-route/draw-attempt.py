#!/usr/bin/env python3
"""Invoke native async draw with an attached synthetic Pencil2 clone."""
import hashlib
import json
import pathlib
import subprocess
import sys
import tempfile

if sys.flags.optimize:
 raise SystemExit('Run without Python optimization; capture requires assertions')

if sys.argv[1:] not in ([], ['--write-receipt']):
 raise SystemExit('Use no arguments to verify or --write-receipt to update')

root = pathlib.Path(__file__).resolve().parents[2]
queue = root / 'tools/pencil2-page-v219/queue-probe.py'
assert hashlib.sha256(queue.read_bytes()).hexdigest() == '1e9e1f7445548f7d23f3b194f0cf6502094519a4a0c05ef2a38950f66f47670c'
source = queue.read_text()
anchor = ' events=[]; matrices=[]'
assert source.count(anchor) == 1
insert = '''
 ATTACH_CONTEXT
 refs=[]
 p.callbacks['SPen::SPGraphicsFactory::RefBitmap(SPen::ISPBitmap*)']=lambda:(refs.append(p.x(0)),p.x(0,0))
 optrect=p.alloc(32);optmatrix=p.alloc(48);p.u.mem_write(optmatrix,bytes(p.u.mem_read(matrix,36)));p.put(optmatrix+36,'B',1)
 data=p.alloc(160)
 name='SPen::AsyncDrawingData::AsyncDrawingData(SPen::ISPBitmap*, SPen::ISPBitmap*, SPen::ISPBitmap*, SPen::ISPBitmap*, SPen::ObjectList const*, std::__ndk1::optional<SPen::Rect>, std::__ndk1::optional<SPen::Matrix3<float> >, bool)'
 def copyhook(u,a,z,d):
  base=p.syms['SPen::ObjectStrokeImpl::CopyDrawingData(SPen::ObjectStrokeImpl*)']
  if a==base+0x24: print('CONTEXT_PRESENT',int(p.x(8)!=0),_always=True)
  if a==p.syms['SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)']:
   assert p.x(3) and p.x(3)!=obj; print('DISTINCT_OBJECT',1,_always=True)
  if a==0x3081a68: print('POINTCOUNT',p.x(0),_always=True)
 p.u.hook_add(UC_HOOK_CODE,copyhook)
 p.call('SPen::ObjectStroke::GetPointCount() const',[obj]); assert p.x(0)==4
 p.call(name,[data,p.bitmap,0,0,0,ol,optrect,optmatrix,0],limit=50000000)
 clone=p.get(data+32,'Q')[0];p.call('SPen::ObjectList::GetCount() const',[clone]); assert p.x(0)==1
 assert refs==[p.bitmap,0,0,0]
'''
attach = '''namebytes='com.samsung.android.sdk.pen.pen.preload.Pencil2'.encode('utf-16le'); setting='14;'.encode('utf-16le')
 manager_bytes=struct.pack('<HIH',2,0,len(namebytes)//2)+namebytes+struct.pack('<IH',1,len(setting)//2)+setting
 mgr=p.alloc(256);p.call('SPen::StringIDManager::StringIDManager()',[mgr]);p.call('SPen::StringIDManager::Construct()',[mgr])
 mb=p.alloc(len(manager_bytes));p.u.mem_write(mb,manager_bytes);p.call('SPen::StringIDManager::ApplyBinary(unsigned char*, int)',[mgr,mb,len(manager_bytes)])
 ctx=p.alloc(0x800);p.call('SPen::ModelContext::ModelContext()',[ctx]);p.call('SPen::ModelContext::SetStringIDManager(SPen::StringIDManager*)',[ctx,mgr]);p.call('SPen::ObjectStroke::OnAttach(SPen::ModelContext*, int)',[obj,ctx,0])'''
for attached in (True,):
 s=source.replace(anchor,insert.replace('ATTACH_CONTEXT',attach if attached else 'pass')+anchor)
 s=s.replace("p.call('SPen::ObjectDrawing::DrawObjectList(SPen::ISPBitmap*, SPen::ObjectList const*, SPen::Rect const*, SPen::Matrix3<float> const*, bool, std::__ndk1::function<bool (int, int)>*)',[p.drawing,p.bitmap,ol,0,matrix,0,0],limit=50000000)", """
  request=p.alloc(128); vec=p.alloc(8);p.put(vec,'Q',data);p.put(request+40,'QQ',vec,vec+8);p.put(request+32,'Q',p.drawing)
  fake=p.alloc(32);vt=p.alloc(256);p.put(fake,'Q',vt);p.put(request+24,'Q',fake)
  stub=0x200f2520;p.u.mem_write(stub,bytes.fromhex('c0035fd6'));p.put(vt+16,'Q',stub);p.put(vt+56,'Q',stub)
  p.put(p.bitmap_vtable+16,'Q',stub);p.put(p.bitmap_vtable+120,'Q',stub);p.put(p.bitmap_vtable+168,'Q',stub);p.put(p.bitmap_vtable+64,'Q',stub);p.put(p.bitmap_vtable+24,'Q',stub)
  p.callbacks['SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, SPen::ISPBitmap*, char const*, int)']=lambda:p.x(0,p.bitmap)
  p.callbacks['SPen::SPGraphicsFactory::ReleaseBitmap(SPen::ISPBitmap*)']=lambda:p.x(0,0)
  p.call('SPen::ObjectAsyncDrawingImpl::draw(SPen::AsyncDrawingDataList*)',[request],limit=50000000)""")
 s=s.replace(' assert [e[0] for e in events]==names, events', ' print("CLONE_EVENTS",json.dumps(events),_always=True)')
 s=s.replace(" assert [line.split()[-1] for line in run.stdout.splitlines() if line.startswith('GET_CONTEXT_INTERVENTION ')]==['0x307f120','0x307f154','0x307f658']", " assert run.returncode==0, (run.stdout,run.stderr)")
 old=" assert run.returncode==0 and 'LIST_COUNT 1' in run.stdout and 'LIST_RETURN 1' in run.stdout and 'LIST_FAILURE' not in run.stdout and '\"addpoint_calls\": 84' in run.stdout and '\"set_matrix_calls\": 2' in run.stdout, (run.stdout,run.stderr)"
 assert s.count(old)==1
 s=s.replace(old, " assert 'LIST_RETURN ' in run.stdout and 'LIST_FAILURE' not in run.stdout, (run.stdout,run.stderr)")
 with tempfile.NamedTemporaryFile(mode='w',suffix='.py',dir=queue.parent) as f:
  f.write(s);f.flush();run=subprocess.run([sys.executable,f.name],cwd=root,text=True,capture_output=True)
 assert run.returncode==0, (run.stdout,run.stderr)
 print(run.stdout); out=run.stdout
 assert out.count('DISTINCT_OBJECT 1')==1
 assert f'CONTEXT_PRESENT {int(attached)}' in out
 assert out.count(f'POINTCOUNT {4 if attached else 0}')==1
 event_lines=[line.removeprefix('CLONE_EVENTS ') for line in out.splitlines() if line.startswith('CLONE_EVENTS ')]
 assert len(event_lines)==1
 events=json.loads(event_lines[0])
 redraw_calls=sum('Pencil2StrokeDrawableGLV14::RedrawPen' in e[0] for e in events)
 assert redraw_calls==1, events
 observation_lines=[line.removeprefix('LIST_OBSERVATIONS ') for line in out.splitlines() if line.startswith('LIST_OBSERVATIONS ')]
 assert len(observation_lines)==1
 observation=json.loads(observation_lines[0])
 assert observation['addpoint_calls']==84 and observation['set_matrix_calls']==2
 assert observation['observed_matrices']==[observation['input_matrix']]*2
 print(f'{"attached" if attached else "unattached"}: context={int(attached)}, points={4 if attached else 0}, RedrawPen={attached}, AddPoint={84 if attached else 0}')
sha=lambda path:hashlib.sha256(path.read_bytes()).hexdigest()
receipt={'scope':'Explicit native ObjectAsyncDrawingImpl::draw with fabricated request, attached synthetic four-point Pencil2 clone, host bitmap and graphics adapters, and deferred queue; no ordinary scheduling, ownership, Matrix4, paint, saved page, or device output established.', 'probe_sha256':sha(pathlib.Path(__file__)), 'queue_probe_sha256':sha(queue), 'drawing_dso_sha256':sha(root/'scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so'), 'observed':{'distinct_clone':True, 'point_count':4, 'redraw_pen_calls':redraw_calls, 'addpoint_calls':observation['addpoint_calls'], 'set_matrix_calls':observation['set_matrix_calls'], 'matrix':observation['input_matrix']}, 'reproduction':'python3 tools/pencil2-async-route/draw-attempt.py', 'limitations':'RefBitmap, CreateBitmap and ReleaseBitmap are host callbacks; virtual graphics and bitmap methods are synthetic stubs; transitive harness and runtime libraries are not fully pinned.'}
receipt_path=pathlib.Path(__file__).with_name('draw-attempt-receipt.json')
if sys.argv[1:]: receipt_path.write_text(json.dumps(receipt,indent=2)+'\n')
else: assert json.loads(receipt_path.read_text())==receipt, 'Receipt differs from observed run'
print('Native draw handoff verified; fabricated request and graphics adapters remain explicit.')
