"""Replay a synthetic serialized frame with native Model initialization and attachment."""
import pathlib,hashlib,sys,json
root=pathlib.Path(__file__).resolve().parents[3]
source=root/'tmp/vector-parity-plan/montblanc-rtv1-mesh-capture-root-v2/montblanc_state_controls.py'
archive=root/'tmp/vector-parity-plan/inkpen-v1-serialized-admission/fountain-frame-name-id2-canonical-null-setting.sdocx'
for path, expected in ((source, 'a8647b96fa001dbe387de8c231ed4895d2f7b5ef619240835c17867366f8c0c8'), (archive, '1a011b8fd8225d1cb5ec3b06e2fb146c5540d3bbbe46110e1727a1603ce4f910')):
 if hashlib.sha256(path.read_bytes()).hexdigest()!=expected: raise SystemExit(f'input hash mismatch: {path}')
s=source.read_text()
a=s.index('obj=p.alloc(256);impl=p.alloc(512);')
b=s.index('model_arrays={}',a)
block='''import zipfile
note_path=ROOT / 'tmp/vector-parity-plan/inkpen-v1-serialized-admission/fountain-frame-name-id2-canonical-null-setting.sdocx'
with zipfile.ZipFile(note_path) as archive:
 note=archive.read('note.note'); page=archive.read('a2ab6718-b54d-11f1-8a68-83a46e5f73c2.page')
size=struct.unpack_from('<I',note,765)[0]; table=note[769:769+size]
old='com.samsung.android.sdk.pen.pen.preload.InkPen'; new='com.samsung.android.sdk.pen.pen.preload.MontblancFountainPen'
a=(2).to_bytes(4,'little')+len(old).to_bytes(2,'little')+old.encode('utf-16le'); b=(2).to_bytes(4,'little')+len(new).to_bytes(2,'little')+new.encode('utf-16le'); assert table.count(a)==1; table=table.replace(a,b)
frame_size=struct.unpack_from('<I',page,2445)[0]; frame=page[2445:2445+frame_size]
rt.run_init(rt.MODEL,rt.BASE)
manager=p.alloc(8);p.u.mem_write(manager,bytes(8));call('SPen::StringIDManager::StringIDManager()',[manager]);call('SPen::StringIDManager::Construct()',[manager]);tp=p.alloc(len(table));p.u.mem_write(tp,table);assert call('SPen::StringIDManager::ApplyBinary(unsigned char*, int)',[manager,tp,len(table)])==1
mc=p.alloc(0x800);p.u.mem_write(mc,bytes(0x800));call('SPen::ModelContext::ModelContext()',[mc]);call('SPen::ModelContext::SetStringIDManager(SPen::StringIDManager*)',[mc,manager])
for name in ('uuid_create','uuid_make','uuid_export','uuid_destroy'):p.callbacks[name]=lambda:p.x(0,0)
obj=p.alloc(256);call('SPen::ObjectStroke::ObjectStroke()',[obj]);call('SPen::ObjectStroke::Construct()',[obj]);impl=p.get(obj+32,'Q')[0]
src=p.alloc(len(frame));p.u.mem_write(src,frame);handler=p.alloc(256);p.u.mem_write(handler,bytes(256));call('SPen::ObjectStrokeBinaryHandler::ObjectStrokeBinaryHandler(SPen::ObjectStrokeImpl*)',[handler,impl]);consumed=call('SPen::ObjectStrokeBinaryHandler::NewApplyBinary(unsigned char*, int, unsigned int, float)',[handler,src,len(frame),0],[1.0]);assert consumed==len(frame)
attach=call('SPen::ObjectStroke::OnAttach(SPen::ModelContext*, int)',[obj,mc,0])
call('SPen::ObjectStroke::GetAdvancedPenSetting() const',[obj]); adv=p.x(0)
getters={}
for label,getter in [('color','GetColor() const'),('tool','GetToolType() const'),('count','GetPointCount() const')]:
 call('SPen::ObjectStroke::'+getter,[obj]);getters[label]=p.x(0)
arrays={}
for label,getter,width in [('points','GetPoint() const',8),('pressure','GetPressure() const',4),('time','GetTimeStamp() const',4),('tilt','GetTilt() const',4),('orientation','GetOrientation() const',4)]:
 call('SPen::ObjectStroke::'+getter,[obj]);ptr=p.x(0);arrays[label]=hashlib.sha256(bytes(p.u.mem_read(ptr,getters['count']*width))).hexdigest() if ptr else None

if MODE=='explicit':call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[obj,string('1;')])
'''
s=s[:a]+block+s[b:]
marker='p.u.hook_del(alpha_source_hook)\nmanager=p.get(drawing+16,\'Q\')[0]'
s=s[:s.index(marker)]+'''\nstream=[[r['entry'],r['float_words_s0_s5']] for r in add_calls]\nprint(json.dumps({'getters':getters,'array_sha256':arrays,'consumed':consumed,'attach':attach,'serialized_advanced_null':not bool(adv),'count':model_count,'size':model_size_bits,'return':rc,'selected':any(r.get('handle')==hex(MB_HANDLE) and r.get('name')=='createPenInst' for r in loader),'redraw':[r['entry'] for r in redraws],'add_count':len(stream),'stream_sha256':hashlib.sha256(json.dumps(stream,separators=(',',':')).encode()).hexdigest()}))\n'''
sys.argv=[str(source),'--case','baseline'];exec(compile(s,str(source),'exec'),{'__file__':str(source),'__name__':'__main__','MODE':__import__('os').environ.get('MODE','null')})
