import hashlib
import gzip
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import zlib

if not __debug__:
    raise RuntimeError("Optimized Python is unsupported")

root = Path(__file__).resolve().parents[2]
here = Path(__file__).resolve().parent
archive_dir = root / 'tools/pencil2-drawing-capture-closure'
base = archive_dir / 'sources/first_point_translated_probe.py'
patch = here / 'capture.patch'
for path, expected in ((base, '675279014dfeb4778d8118a02d8d9d77779f4cec1c6f18444a5f7b28751e6861'), (patch, 'b9342438df1b8099caf4185a0dc6ea692d7726e48014e921866ef6c3b4389a83')):
    if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
        raise RuntimeError(f'Pinned input changed: {path}')
out = Path(os.environ['PENCIL_SCENE_OUTPUT'])
backdrop = os.environ.get('PENCIL_SCENE_BACKDROP', 'blue')
scale_text = os.environ.get('PENCIL_SCENE_SCALE', '1')
if backdrop not in ('blue', 'white', 'transparent') or scale_text not in ('1', '2'):
    raise ValueError('Unsupported backdrop or scale')
scale = int(scale_text)
if out.exists() or not out.parent.is_dir():
    raise ValueError('Output must be a new directory with an existing parent')
def replace_once(text, old, new):
    if text.count(old) != 1:
        raise RuntimeError(f'Expected exactly one capture anchor: {old}')
    return text.replace(old, new, 1)
native_dir = Path(os.environ.get('PENCIL_SCENE_NATIVE_DIR', root / 'scratch/apk-analysis-native/arm64-v8a')).resolve()
runtime_dir = Path(os.environ.get('PENCIL_SCENE_RUNTIME_DIR', root / 'scratch/apk-analysis-runtime')).resolve()
with tempfile.TemporaryDirectory(dir='/tmp', prefix='pencil2-independent-') as directory:
    tmp = Path(directory)
    workspace = tmp / 'workspace'
    spec = importlib.util.spec_from_file_location('archive', archive_dir / 'restore.py')
    archive = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(archive)
    for source, destination, expected in archive.FILES:
        src = archive_dir / 'sources' / source if not source.startswith('../') else root / 'tools' / source[3:]
        data = src.read_bytes()
        if source.endswith('.gz'):
            data = gzip.decompress(data)
        if hashlib.sha256(data).hexdigest() != expected:
            raise RuntimeError(f'Archive hash mismatch: {src}')
        dest = workspace / destination
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(data)
    helper = (archive_dir / 'sources/inkpen_paint_native.py').read_bytes()
    if hashlib.sha256(helper).hexdigest() != 'a3e65b2b37b2913b4bb2787b72ab2dbad0a9660cb66435b142f5acfa4d5051ae':
        raise RuntimeError('GLES helper hash mismatch')
    conf = workspace / archive.CONF
    (conf / 'inkpen_paint_native.py').write_bytes(helper)
    scratch = workspace / 'scratch'
    (scratch / 'apk-analysis-native').mkdir(parents=True)
    (scratch / 'apk-analysis-native/arm64-v8a').symlink_to(native_dir, target_is_directory=True)
    (scratch / 'apk-analysis-runtime').symlink_to(runtime_dir, target_is_directory=True)
    (conf.parent / 'scratch').symlink_to(scratch, target_is_directory=True)
    (workspace / 'tools/pencil2-public-replay').mkdir(parents=True)
    (workspace / 'tmp').mkdir(exist_ok=True)
    probe = tmp / 'probe.py'
    probe.write_bytes(base.read_bytes())
    subprocess.run(['patch', str(probe), str(patch)], check=True)
    code = probe.read_text()
    old_root = '/home/jding/RustroverProjects/sdocx'
    if code.count(old_root) != 4:
        raise RuntimeError('Unexpected adapter path count')
    code = code.replace(old_root, str(workspace))
    code = replace_once(code, 'root = Path(__file__).resolve().parents[2]', f'root = Path({str(workspace)!r})')
    code = replace_once(code, "dir=root / 'tmp'", f'dir=Path({str(workspace / "tmp")!r})')
    clear = {'blue': '0.25,0.5,0.75,1', 'white': '1,1,1,1', 'transparent': '0,0,0,0'}[backdrop]
    code = replace_once(code, "f['glClearColor'](0.25,0.5,0.75,1)", f"f['glClearColor']({clear})")
    if scale == 2:
        code = replace_once(code, "p.put(matrix,'16f',1,0,0,0,0,1,0,0,0,0,1,0,6,9,0,1)", "p.put(matrix,'16f',2,0,0,0,0,2,0,0,0,0,1,0,6,9,0,1)")
    for name in ('first', 'second'):
        code = replace_once(code, f"root / 'tmp/pencil2-mixed-v73/{name}.zlib'", f'Path({str(tmp / (name + ".zlib"))!r})')
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
    if scale == 2:
        code = replace_once(code, anchor, anchor.replace("16f',1,0,0,0,0,1", "16f',2,0,0,0,0,2"))
        anchor = anchor.replace("16f',1,0,0,0,0,1", "16f',2,0,0,0,0,2")
    code = replace_once(code, anchor, injected + anchor)
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
    code = replace_once(code, "Path({str(tmp / 'second.vbo')!r})", f'Path({str(tmp / "second.vbo")!r})')
    probe.write_text(code)
    env = {**os.environ, 'PYTHONPATH': str(workspace / 'scratch/apk-analysis-runtime/python')}
    result = subprocess.run(['python3', str(probe)], cwd=workspace, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    captures = list((workspace / 'tmp').glob('pencil2-queue-intervention-*/handoff/capture.json'))
    if result.returncode != 0 or len(captures) != 1:
        print(result.stdout[-8000:])
        raise SystemExit(f'capture failed or ambiguous: exit {result.returncode}, captures {len(captures)}')
    capture = json.loads(captures[0].read_text())
    target = capture['native_target_probe']
    receipt = {'scope':'ARM64 native ObjectStroke and Drawing on synthetic queue, host Mesa framebuffer; explicit native Update and Draw; no device parity', 'capture_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'base_sha256':hashlib.sha256(base.read_bytes()).hexdigest(), 'patch_sha256':hashlib.sha256(patch.read_bytes()).hexdigest(), 'first_sha256':target.get('first_sha256'), 'first_gl_error':target.get('first_error'), 'second':target.get('independent_second'), 'second_sha256':target.get('readback_sha256'), 'second_gl_error':target.get('draw_gl_error'), 'uniforms':[x for x in target.get('gl_calls',[]) if x[0]=='glUniform2fv'], 'outer_outcome':target.get('outcome'), 'outer_error':target.get('error'), 'exit_code':result.returncode}
    second = target.get('independent_second', {})
    if (target.get('outcome') is not None or target.get('error') is not None
        or target.get('first_error') != 0 or target.get('draw_gl_error') != 0
        or second.get('outcome') != 'returned' or second.get('stage') != 'draw_returned'
        or second.get('same_rt') is not True
        or second.get('update_gl_error') != 0
        or second.get('geometry_sha256') != 'ab8c231108800d1a9437783f27d12c57e59c4d81a00538268343f662a1c64389'):
        raise RuntimeError('Incomplete native capture or unexpected geometry/GL status')
    geometry = (tmp / 'second.vbo').read_bytes()
    if hashlib.sha256(geometry).hexdigest() != second['geometry_sha256']:
        raise RuntimeError('Geometry hash mismatch')
    for name, key in (('first', 'first_sha256'), ('second', 'second_sha256')):
        raw = zlib.decompress((tmp / (name + '.zlib')).read_bytes())
        if len(raw) != 192 * 192 * 4 or hashlib.sha256(raw).hexdigest() != receipt[key]:
            raise RuntimeError(f'{name} frame length or hash mismatch')
    receipt['backdrop'] = backdrop
    receipt['draw_scale'] = scale
    receipt['draw_matrices'] = [[scale,0,0,0,0,scale,0,0,0,0,1,0,6,9,0,1], [scale,0,0,0,0,scale,0,0,0,0,1,0,12,12,0,1]]
    out.mkdir(parents=False, exist_ok=False)
    (out / f'{backdrop}-{scale}-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
    if (tmp / 'second.vbo').exists():
        (out / f'{backdrop}-{scale}-second.vbo').write_bytes((tmp / 'second.vbo').read_bytes())
    for name in ('first', 'second'):
        if (tmp / (name + '.zlib')).exists():
            (out / f'{backdrop}-{scale}-{name}.zlib').write_bytes((tmp / (name + '.zlib')).read_bytes())
    print(json.dumps(receipt,indent=2))
