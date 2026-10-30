#!/usr/bin/env python3
"""Compare native AsyncDrawingData clones with and without model attachment."""
import hashlib
import pathlib
import subprocess
import sys
import tempfile

if sys.flags.optimize:
 raise SystemExit('Run without Python optimization; capture requires assertions')

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
for attached in (False, True):
 s=source.replace(anchor,insert.replace('ATTACH_CONTEXT',attach if attached else 'pass')+anchor)
 s=s.replace('[p.drawing,p.bitmap,ol,0,matrix,0,0]', '[p.drawing,p.bitmap,clone,0,matrix,0,0]')
 s=s.replace(' assert [e[0] for e in events]==names, events', ' print("CLONE_EVENTS",json.dumps(events),_always=True)')
 s=s.replace(" assert [line.split()[-1] for line in run.stdout.splitlines() if line.startswith('GET_CONTEXT_INTERVENTION ')]==['0x307f120','0x307f154','0x307f658']", " assert [line.split()[-1] for line in run.stdout.splitlines() if line.startswith('GET_CONTEXT_INTERVENTION ')]==['0x307f120','0x307f154','0x307f658']; assert run.returncode==0, (run.stdout,run.stderr)")
 old=" assert run.returncode==0 and 'LIST_COUNT 1' in run.stdout and 'LIST_RETURN 1' in run.stdout and 'LIST_FAILURE' not in run.stdout and '\"addpoint_calls\": 84' in run.stdout and '\"set_matrix_calls\": 2' in run.stdout, (run.stdout,run.stderr)"
 assert s.count(old)==1
 s=s.replace(old, " assert 'LIST_RETURN 1' in run.stdout and 'LIST_FAILURE' not in run.stdout, (run.stdout,run.stderr)")
 with tempfile.NamedTemporaryFile(mode='w',suffix='.py',dir=queue.parent) as f:
  f.write(s);f.flush();run=subprocess.run([sys.executable,f.name],cwd=root,text=True,capture_output=True)
 assert run.returncode==0, (run.stdout,run.stderr)
 out=run.stdout
 assert out.count('DISTINCT_OBJECT 1')==1
 assert f'CONTEXT_PRESENT {int(attached)}' in out
 assert out.count(f'POINTCOUNT {4 if attached else 0}')==1
 assert ('Pencil2StrokeDrawableGLV14::RedrawPen' in out)==attached
 assert f'"addpoint_calls": {84 if attached else 0}' in out
 print(f'{"attached" if attached else "unattached"}: context={int(attached)}, points={4 if attached else 0}, RedrawPen={attached}, AddPoint={84 if attached else 0}')
print('Native clone/list controls verified; host callbacks and synchronous list dispatch remain explicit.')
