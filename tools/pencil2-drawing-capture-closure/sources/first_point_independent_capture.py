import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[2]
here = Path(__file__).resolve().parent
base = root / 'tools/pencil2-public-replay/drawing_native_target_translated_probe.py'
patch = root / 'tools/pencil2-mixed-drawing/capture.patch'
with tempfile.TemporaryDirectory(dir=root / 'tmp', prefix='pencil2-independent-') as directory:
    tmp = Path(directory)
    probe = tmp / 'probe.py'
    probe.write_bytes(base.read_bytes())
    subprocess.run(['patch', str(probe), str(patch)], check=True)
    code = probe.read_text().replace('root = Path(__file__).resolve().parents[2]', f'root = Path({str(root)!r})')
    code = code.replace("dir=root / 'tmp'", f'dir=Path({str(tmp)!r})')
    for name in ('first', 'second'):
        code = code.replace(f"root / 'tmp/pencil2-mixed-v73/{name}.zlib'", f'Path({str(tmp / (name + ".zlib"))!r})')
    anchor = "  p.put(matrix,'16f',1,0,0,0,0,1,0,0,0,0,1,0,12,12,0,1)"
    assert code.count(anchor) == 1
    injected = '''  second={'stage':'constructor','first_rt':hex(selected_rt),'first_object':hex(obj),'queue_start':len(queue_events),'handoff_start':len(handoff_events)}
  try:
   obj2=p.alloc(0x100); impl2=p.alloc(0x400)
   p.call('SPen::ObjectStroke::ObjectStroke()',[obj2]); p.call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)',[impl2,obj2]); p.put(obj2+32,'Q',impl2)
   second.update({'object':hex(obj2),'impl':hex(impl2),'stage':'setters'})
   p.call('SPen::ObjectStroke::SetColor(unsigned int)',[obj2,0xc0cc3344]); p.f(0,10.0); p.call('SPen::ObjectStroke::SetPenSize(float)',[obj2]); p.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj2,2]); p.call('SPen::ObjectStroke::SetStrokeType(SPen::StrokeType)',[obj2,0]); p.call('SPen::ObjectStroke::SetCurveEnabled(bool)',[obj2,1])
   pts2=p.alloc(32); prs2=p.alloc(16); ts2=p.alloc(16); tilt2=p.alloc(16); orient2=p.alloc(16)
   p.u.mem_write(pts2,struct.pack('<8f',0,2,10,2,20,4,30,12)); p.u.mem_write(prs2,struct.pack('<4f',0.9,0.7,0.5,0.3)); p.u.mem_write(ts2,struct.pack('<4i',10,20,30,40)); p.u.mem_write(tilt2,struct.pack('<4f',0.4,0.4,0.4,0.4)); p.u.mem_write(orient2,struct.pack('<4f',0,0,0,0))
   p.call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj2,pts2,prs2,ts2,tilt2,orient2,4]); p.call('SPen::ObjectBase::SetVisibility(bool)',[obj2,1]); p.call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj2,pname]); p.call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[obj2,advanced])
   second['model_getters']=snapshot_object_stroke('second_before_draw',obj2); second['source_arrays']=snapshot_source_arrays(obj2)
   p.call('SPen::ObjectStroke::GetAdvancedPenSetting() const',[obj2]); setting2=p.x(0); p.call('SPen::String::GetUTF8Size() const',[setting2]); size2=p.x(0); buf2=p.alloc(size2+1); p.call('SPen::String::GetUTF8(char*, int) const',[setting2,buf2,size2+1]); second['advanced_setting']=bytes(p.u.mem_read(buf2,size2+1)).split(bytes([0]),1)[0].decode('ascii')
   second['stage']='drawing'; phase='second_draw_object_stroke'
   p.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[p.drawing,p.draw_canvas,p.pen_canvas,obj2,1],limit=5000000)
   second['drawing_result']=p.x(0); second['stage']='renderer_identity'
   pen2=factory_created_pens[-1]; p.call('SPen::Pencil2::GetStrokeDrawableGL()',[pen2]); drawable2=p.x(0); rt2=p.get(drawable2+16,'Q')[0]; second.update({'pen':hex(pen2),'drawable':hex(drawable2),'rt':hex(rt2),'same_rt':rt2==selected_rt,'outcome':'returned'})
  except Exception as error:
   second.update({'outcome':'exception','error':repr(error),'pc':hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})
  second['queue_events']=queue_events[second['queue_start']:]; second['handoff_events']=handoff_events[second['handoff_start']:]; target['independent_second']=second
'''
    code = code.replace(anchor, injected + anchor)
    second_call = "  p.call('SPen::Pencil2StrokeDrawableRTV4::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[selected_rt,bitmap,matrix],limit=8000000)"
    assert code.count(second_call) == 2
    first, second = code.split(second_call, 1)
    second = second.replace(second_call, """  if target['independent_second'].get('outcome')=='returned':
   descriptor=p.get(rt2+88,'Q')[0]; begin,end,capacity=p.get(descriptor,'3Q'); geometry=bytes(p.u.mem_read(begin,end-begin)); Path({str(tmp / 'second.vbo')!r}).write_bytes(geometry); target['independent_second']['geometry_sha256']=hashlib.sha256(geometry).hexdigest(); target['independent_second']['geometry_bytes']=len(geometry)
   target['independent_second']['stage']='update'
   p.call('SPen::Pencil2StrokeDrawableRTV4::Update(bool)',[rt2,0],limit=5000000)
   target['independent_second']['update_gl_error']=f['glGetError']()
   target['independent_second']['stage']='draw'
   p.call('SPen::Pencil2StrokeDrawableRTV4::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[rt2,bitmap,matrix],limit=8000000)
   target['independent_second']['stage']='draw_returned'
""", 1)
    code = first + second_call + second
    code = code.replace("Path({str(tmp / 'second.vbo')!r})", f'Path({str(tmp / "second.vbo")!r})')
    probe.write_text(code)
    env = {**os.environ, 'PYTHONPATH': str(root / 'scratch/apk-analysis-runtime/python')}
    result = subprocess.run(['python3', str(probe)], cwd=root, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    captures = list(tmp.glob('pencil2-queue-intervention-*/handoff/capture.json'))
    if not captures:
        print(result.stdout[-8000:])
        raise SystemExit(f'capture missing; exit {result.returncode}')
    capture = json.loads(captures[0].read_text())
    target = capture['native_target_probe']
    receipt = {'scope':'ARM64 native ObjectStroke and Drawing on synthetic queue, host Mesa framebuffer; explicit native Update and Draw; no device parity', 'capture_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'base_sha256':hashlib.sha256(base.read_bytes()).hexdigest(), 'patch_sha256':hashlib.sha256(patch.read_bytes()).hexdigest(), 'first_sha256':target.get('first_sha256'), 'first_gl_error':target.get('first_error'), 'second':target.get('independent_second'), 'second_sha256':target.get('readback_sha256'), 'second_gl_error':target.get('draw_gl_error'), 'uniforms':[x for x in target.get('gl_calls',[]) if x[0]=='glUniform2fv'], 'outer_outcome':target.get('outcome'), 'outer_error':target.get('error'), 'exit_code':result.returncode}
    (here / 'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
    if (tmp / 'second.vbo').exists():
        (here / 'second.vbo').write_bytes((tmp / 'second.vbo').read_bytes())
    for name in ('first', 'second'):
        if (tmp / (name + '.zlib')).exists():
            (here / (name + '.zlib')).write_bytes((tmp / (name + '.zlib')).read_bytes())
    print(json.dumps(receipt,indent=2))
