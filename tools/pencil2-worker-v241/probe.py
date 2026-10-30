import hashlib, json, pathlib, subprocess, tempfile, sys
if sys.flags.optimize: raise SystemExit("Run without Python optimization")
assert sys.argv[1:] in ([], ["--write-receipt"])
root=pathlib.Path(__file__).resolve().parents[2]
q=root/'tools/pencil2-page-v219/queue-probe.py'; s=q.read_text()
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(q)=='1e9e1f7445548f7d23f3b194f0cf6502094519a4a0c05ef2a38950f66f47670c'
draw=(root/'tools/pencil2-async-route/draw-attempt.py').read_text()
insert=draw.split("insert = '''",1)[1].split("'''\nattach =",1)[0]
attach=draw.split("attach = '''",1)[1].split("'''\nfor attached",1)[0]
insert=insert.replace('ATTACH_CONTEXT',attach)
s=s.replace(' events=[]; matrices=[]',insert+' events=[]; matrices=[]')
old="p.call('SPen::ObjectDrawing::DrawObjectList(SPen::ISPBitmap*, SPen::ObjectList const*, SPen::Rect const*, SPen::Matrix3<float> const*, bool, std::__ndk1::function<bool (int, int)>*)',[p.drawing,p.bitmap,ol,0,matrix,0,0],limit=50000000)"
new='''
  name='SPen::ObjectAsyncDrawing::DrawingData::DrawingData(SPen::ISPBitmap*, SPen::ObjectList const*, std::__ndk1::optional<SPen::Rect>, std::__ndk1::optional<SPen::Matrix3<float> >, bool, SPen::ISPBitmap*, SPen::ISPBitmap*, SPen::ISPBitmap*)'
  dd=p.alloc(112);p.call(name,[dd,p.bitmap,ol,optrect,optmatrix,0,0,0,0],limit=50000000)
  vector=p.alloc(24);p.put(vector,'QQQ',dd,dd+112,dd+112)
  async_obj=p.alloc(256);p.put(async_obj+120,'B',1);p.put(async_obj+8,'Q',async_obj+16)
  requests=[]
  p.put(async_obj+32,'QQQ',async_obj+32,async_obj+32,0)
  signals=[]
  p.callbacks['SPen::ObjectAsyncDrawingImpl::GetInstance()']=lambda:p.x(0,async_obj)
  p.callbacks['SPen::AutoCriticalSection::AutoCriticalSection(SPen::CriticalSection*, char const*, int)']=lambda:p.x(0,0)
  p.callbacks['SPen::AutoCriticalSection::~AutoCriticalSection()']=lambda:p.x(0,0)
  p.callbacks['SPen::ConditionalVariable::Signal()']=lambda:(signals.append(1),p.x(0,0))
  # Minimal bitmap validation object; synthetic true-return and self-return methods.
  yes=0x200f2520;p.u.mem_write(yes,bytes.fromhex('200080d2c0035fd6'))
  ret=0x200f2530;p.u.mem_write(ret,bytes.fromhex('c0035fd6'))
  fake=p.alloc(32);vt=p.alloc(256);p.put(fake,'Q',vt);p.put(vt+16,'Q',ret);p.put(vt+56,'Q',ret)
  p.put(async_obj+64,'QQ',fake,p.drawing)
  p.callbacks['SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, SPen::ISPBitmap*, char const*, int)']=lambda:p.x(0,p.bitmap)
  p.callbacks['SPen::SPGraphicsFactory::ReleaseBitmap(SPen::ISPBitmap*)']=lambda:p.x(0,0)
  p.put(p.bitmap_vtable+16,'Q',ret);p.put(p.bitmap_vtable+24,'Q',ret)
  p.put(p.bitmap_vtable+168,'Q',ret);p.put(p.bitmap_vtable+72,'Q',yes);p.put(p.bitmap_vtable+64,'Q',ret);p.put(p.bitmap_vtable+120,'Q',ret)
  sy='SPen::ObjectAsyncDrawing::DrawObjectList(long, int, std::__ndk1::vector<SPen::ObjectAsyncDrawing::DrawingData, std::__ndk1::allocator<SPen::ObjectAsyncDrawing::DrawingData> >&)'
  queue_entries=[]
  def trace_submit(u,a,z,d):
   if a in (0x307bde8,0x307beb4,0x307bf68): queue_entries.append(a)
   if 0x3079968<=a<=0x3079db8 and (a in (0x30799d4,0x3079a30,0x3079aec,0x3079bbc,0x3079c8c,0x3079d08)): print('SUBMIT_PC',hex(a),_always=True)
  p.u.hook_add(UC_HOOK_CODE,trace_submit)
  p.call(sy,[async_obj,1,1,vector],limit=50000000)
  assert queue_entries==[0x307bde8,0x307beb4,0x307bf68], queue_entries
  print('QUEUE_ENTRIES',json.dumps(queue_entries),_always=True)
  node=p.get(async_obj+40,'Q')[0]; assert node!=async_obj+32
  requests.append(p.get(node+16,'Q')[0]); assert p.get(async_obj+48,'Q')[0]==1 and len(signals)==1
  print('SUBMIT_RETURN',p.x(0),'REQUESTS',len(requests),_always=True)
  # EventLoop accesses the library singleton globals directly; transplant the
  # synthetic queue links into that singleton's native list head.
  head=0x30ce9b0; node=p.get(async_obj+40,'Q')[0]
  p.put(head,'QQQ',node,node,1); p.put(node,'QQ',head,head)
  p.put(0x30ce9a8,'Q',async_obj+16); p.put(0x30ce9d0,'Q',async_obj+16)
  worker=[]
  def worker_trace(u,a,z,d):
   if a in (0x307b81c,0x307b830,0x307b848,0x307b868,0x307b880,0x307b88c): worker.append(a)
   if a==0x307b88c: u.emu_stop()
  p.u.hook_add(UC_HOOK_CODE,worker_trace)
  p.callbacks['SPen::AsyncDrawingDataList::~AsyncDrawingDataList()']=lambda:p.x(0,0)
  try: p.call('SPen::ObjectAsyncDrawingImpl::EventLoop(void*)',[0],limit=50000000)
  except RuntimeError as e: assert 'PC=0x307b88c' in str(e), e
  print('WORKER_TRACE',worker,_always=True)
  print('WORKER_REMAINING',p.get(head+16,'Q')[0],_always=True)
  print('WORKER_EMPTY_LINKS',int(p.get(head,'QQ')==(head,head)),_always=True)
  print('DRAIN_RETURN',_always=True)
'''
assert old in s;s=s.replace(old,new)
s=s.replace(' assert [e[0] for e in events]==names, events',' print("EVENTS",events,_always=True)').replace(' assert matrices==[[1,0,0,0,-1,0,6,183,1]]*2, matrices',' pass')
s=s.replace(" assert [line.split()[-1] for line in run.stdout.splitlines() if line.startswith('GET_CONTEXT_INTERVENTION ')]==['0x307f120','0x307f154','0x307f658']",' print(run.stderr)')
s=s.replace(" assert run.returncode==0 and 'LIST_COUNT 1' in run.stdout and 'LIST_RETURN 1' in run.stdout and 'LIST_FAILURE' not in run.stdout and '\"addpoint_calls\": 84' in run.stdout and '\"set_matrix_calls\": 2' in run.stdout, (run.stdout,run.stderr)",' pass')
with tempfile.NamedTemporaryFile(mode='w',suffix='.py',dir=q.parent) as f:
 f.write(s);f.flush();p=subprocess.run([sys.executable,f.name],cwd=root,text=True,capture_output=True)
 assert p.returncode==0, (p.stdout,p.stderr)
 out=p.stdout
 assert 'LIST_FAILURE' not in out and 'SUBMIT_RETURN 1 REQUESTS 1' in out and 'DRAIN_RETURN' in out, out
 assert 'QUEUE_ENTRIES [50839016, 50839220, 50839400]' in out, out
 assert 'WORKER_REMAINING 0' in out and 'WORKER_EMPTY_LINKS 1' in out, out
 assert 'WORKER_TRACE [50837532, 50837552, 50837576, 50837608, 50837632, 50837644]' in out, out
 assert out.count('DISTINCT_OBJECT 1')==1 and out.count('POINTCOUNT 4')==1, out
 assert out.count('CONTEXT_PRESENT 1')==2, out
 lines=[x.removeprefix('LIST_OBSERVATIONS ') for x in out.splitlines() if x.startswith('LIST_OBSERVATIONS ')]
 assert len(lines)==1, out
 obs=json.loads(lines[0]); assert obs['addpoint_calls']==84 and obs['set_matrix_calls']==2 and obs['observed_matrices']==[obs['input_matrix']]*2, obs
 events=[x.removeprefix('LIST_EVENTS ') for x in out.splitlines() if x.startswith('LIST_EVENTS ')]
 assert len(events)==1
 names=[x[0] for x in json.loads(events[0])]
 assert names==['SPen::ObjectList::GetData(long) const']*2+['SPen::ObjectDrawing::drawObject(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectBase const*, SPen::Rect const*)','SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)','SPen::Pencil2StrokeDrawableGLV14::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)'], names
 receipt={'scope':'Native EventLoop consumes a synthetic enqueued request in Unicorn, with host intervention; no concurrent scheduling, Matrix4, GPU paint, saved page or device behavior.', 'script_sha256':sha(pathlib.Path(__file__)), 'queue_probe_sha256':sha(q), 'draw_probe_sha256':sha(root/'tools/pencil2-async-route/draw-attempt.py'), 'drawing_dso_sha256':sha(root/'scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so'), 'observed':{'submission_return':1,'native_entry_order':['RequestDraw','findRequest','addRequest'],'requests_enqueued':1,'eventloop_entry_and_cleanup_rvas':['0x7b81c','0x7b830','0x7b848','0x7b868','0x7b880','0x7b88c'],'remaining_requests':0,'empty_sentinel_links':True,'distinct_clone':True,'clone_point_count':4,'redraw_pen_calls':1,'addpoint_calls':obs['addpoint_calls'],'matrix_setter_calls':obs['set_matrix_calls'],'matrix':obs['input_matrix']},'limitations':'Queue links are transplanted from the synthetic submission object into native singleton globals. Lock and condition operations and request destructor are host callbacks; execution stops at the loop back-edge. Bitmap, graphics, context, allocation and reference adapters are synthetic. Host runtime and transitive harness dependencies are not fully pinned.', 'reproduction':'python3 tools/pencil2-worker-v241/probe.py'}
 path=pathlib.Path(__file__).with_name('enqueue-receipt.json')
 if sys.argv[1:]: path.write_text(json.dumps(receipt,indent=2)+'\n')
 else: assert json.loads(path.read_text())==receipt, 'Receipt differs from observed run'
 print('Verified one native EventLoop iteration on a synthetic queue; concurrent scheduling and paint remain unobserved.')
