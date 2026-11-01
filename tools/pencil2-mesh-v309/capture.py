"""Synthetic saved payload through native Model and Pencil2 Drawing."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import struct
import zlib
import tempfile

root = Path(__file__).resolve().parents[2]
scratch_dir = tempfile.TemporaryDirectory(prefix='sdocx-pencil2-control-')
scratch = Path(scratch_dir.name)
source = root / 'tools/pencil2-independent-drawing/capture.py'
inputs = root / 'conformance/fountain-v14-saved-scene-inputs.json'
original = source.read_text()
original = original.replace("root = Path(__file__).resolve().parents[2]", f"root = Path({str(root)!r})", 1)
original = original.replace("here = Path(__file__).resolve().parent", f"here = Path({str(scratch)!r})", 1)
clear = os.environ.get('SDOCX_PENCIL2_CLEAR_RGBA')
if clear:
    rgba = tuple(int(value) for value in clear.split(','))
    assert len(rgba) == 4 and all(0 <= value <= 255 for value in rgba)
    assert all(value <= rgba[3] for value in rgba[:3])
    marker = "    code = probe.read_text().replace("
    assert original.count(marker) == 1
    original = original.replace(marker, "    code = probe.read_text().replace(\"f['glClearColor'](0.25,0.5,0.75,1)\", "+repr("f['glClearColor']("+','.join(str(value / 255) for value in rgba)+")")+").replace(", 1)
start = original.index("   obj2=p.alloc(0x100); impl2=p.alloc(0x400)")
end = original.index("   second['model_getters']=", start)
replacement = '''   for symbol in ('uuid_create','uuid_make','uuid_export','uuid_destroy'): p.callbacks[symbol]=lambda:p.x(0,0)
   source=json.loads(Path(INPUT_PATH).read_text())['strokes'][2]
   name='com.samsung.android.sdk.pen.pen.preload.Pencil2'.encode('utf-16le')
   setting='14;'.encode('utf-16le')
   manager_bytes=struct.pack('<HIH',2,0,len(name)//2)+name+struct.pack('<IH',1,len(setting)//2)+setting
   mgr=p.alloc(256); p.call('SPen::StringIDManager::StringIDManager()',[mgr]); p.call('SPen::StringIDManager::Construct()',[mgr])
   mb=p.alloc(len(manager_bytes)); p.u.mem_write(mb,manager_bytes); p.call('SPen::StringIDManager::ApplyBinary(unsigned char*, int)',[mgr,mb,len(manager_bytes)])
   ctx=p.alloc(0x800); p.call('SPen::ModelContext::ModelContext()',[ctx]); p.call('SPen::ModelContext::SetStringIDManager(SPen::StringIDManager*)',[ctx,mgr])
   obj2=p.alloc(256); p.call('SPen::ObjectStroke::ObjectStroke()',[obj2]); p.call('SPen::ObjectStroke::Construct()',[obj2])
   payload=bytes.fromhex(source['payload_hex']); data=p.alloc(len(payload)); p.u.mem_write(data,payload)
   p.call('SPen::ObjectStroke::NewApplyBinary(unsigned char*, int, SPen::Orientation, int)',[obj2,data,len(payload),source['orientation'],source['page_width']],limit=5000000)
   second['consumed']=p.x(0); second['payload_size']=len(payload)
   if second['consumed']!=len(payload): raise RuntimeError('incomplete payload consumption')
   p.call('SPen::ObjectStroke::OnAttach(SPen::ModelContext*, int)',[obj2,ctx,0])
   vals=source['stroke']; n=len(vals['points']); arrays=[]
   for field,fmt in [('points','f'),('pressures','f'),('timestamps','i'),('tilts','f'),('orientations','f')]:
    values=vals[field]; values=[v for pt in values for v in (pt['x'],pt['y'])] if field=='points' else values
    b=struct.pack('<'+fmt*len(values),*values); ptr=p.alloc(len(b)); p.u.mem_write(ptr,b); arrays.append(ptr)
   obj2=p.alloc(0x100); impl2=p.alloc(0x400)
   p.call('SPen::ObjectStroke::ObjectStroke()',[obj2]); p.call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)',[impl2,obj2]); p.put(obj2+32,'Q',impl2)
   p.call('SPen::ObjectStroke::SetColor(unsigned int)',[obj2,4294425088]); p.f(0,struct.unpack('<f',bytes.fromhex('6ade1141'))[0]); p.call('SPen::ObjectStroke::SetPenSize(float)',[obj2]); p.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj2,2]); p.call('SPen::ObjectStroke::SetStrokeType(SPen::StrokeType)',[obj2,0]); p.call('SPen::ObjectStroke::SetCurveEnabled(bool)',[obj2,1]); p.call('SPen::ObjectBase::SetVisibility(bool)',[obj2,1]); p.call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj2,pname]); p.call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[obj2,advanced])
   p.call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj2,*arrays,n])
   second['stage']='deserialized'
'''.replace('INPUT_PATH',repr(str(inputs)))
original = original[:start] + replacement + original[end:]
with tempfile.TemporaryDirectory(dir=root / 'tmp', prefix='pencil2-serialized-wrapper-') as d:
    script = Path(d) / 'capture.py'
    script.write_text(original)
    result = subprocess.run(['python3', str(script)], cwd=root, text=True, capture_output=True)
    if result.returncode:
        print(result.stderr[-5000:])
        raise SystemExit(result.returncode)

receipt_path = scratch / 'receipt.json'
receipt = json.loads(receipt_path.read_text())
row = receipt['second']
source_row = json.loads(inputs.read_text())['strokes'][2]
assert result.returncode == 0 and receipt['outer_outcome'] is None and receipt['outer_error'] is None
assert row['outcome'] == 'returned' and row['stage'] == 'draw_returned' and row['drawing_result'] == 1
assert row['same_rt'] and row['advanced_setting'] == '14;' and row['consumed'] == row['payload_size']
assert receipt['first_gl_error'] == receipt['second_gl_error'] == row['update_gl_error'] == 0
assert row['model_getters']['point_count'] == source_row['point_count']
channel_hashes = {}
for field in ('points','pressures','timestamps','tilts','orientations'):
    values = source_row['stroke'][field]
    if field == 'points': values = [v for point in values for v in (point['x'],point['y'])]
    expected = struct.pack('<' + ('i' if field == 'timestamps' else 'f') * len(values), *values)
    actual = bytes.fromhex(row['source_arrays'][field]['data_hex'])
    assert actual == expected, field
    channel_hashes[field] = hashlib.sha256(actual).hexdigest()
first = zlib.decompress((scratch / 'first.zlib').read_bytes())
second = zlib.decompress((scratch / 'second.zlib').read_bytes())
if os.environ.get('SDOCX_PENCIL2_FRESH_FRAME_DIR'):
    frame_dir = Path(os.environ['SDOCX_PENCIL2_FRESH_FRAME_DIR'])
    frame_dir.mkdir(parents=True, exist_ok=True)
    (frame_dir / 'first.rgba').write_bytes(first)
    (frame_dir / 'second.rgba').write_bytes(second)
    (frame_dir / 'second.vbo').write_bytes((scratch / 'second.vbo').read_bytes())
changed = sum(first[i:i+4] != second[i:i+4] for i in range(0,len(first),4))
assert changed > 0
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
compact = {'scope':'Fresh setter-built ObjectStroke using decoded Fountain payload channels and observed style, replacing the earlier deserialized object before Drawing selection; deferred host queue and explicit Update and Draw on ARM64 emulation and host Mesa. The first stroke is synthetic and the second is drawn over it. No original saved Pencil2, outer page loading, normal queue scheduling, device or production parity is established.', 'command':'python3 tools/pencil2-mesh-v309/capture.py', 'source_index':source_row['index'], 'source_payload_sha256':source_row['payload_sha256'], 'inputs_sha256':sha(inputs), 'script_sha256':sha(Path(__file__)), 'base_sha256':sha(source), 'drawing_base_sha256':receipt['base_sha256'], 'patch_sha256':receipt['patch_sha256'], 'point_count':source_row['point_count'], 'discarded_deserialized_object_consumed':row['consumed'], 'channel_sha256':channel_hashes, 'native_getters':row['model_getters'], 'advanced_setting':row['advanced_setting'], 'drawing_return':row['drawing_result'], 'same_renderer':row['same_rt'], 'geometry_bytes':row['geometry_bytes'], 'geometry_sha256':row['geometry_sha256'], 'uniforms':receipt['uniforms'], 'first_sha256':receipt['first_sha256'], 'second_sha256':receipt['second_sha256'], 'changed_pixels':changed, 'gl_error_checks':{'first_draw':receipt['first_gl_error'],'second_update':row['update_gl_error'],'second_draw':receipt['second_gl_error']}, 'limitations':'UUID callbacks are stubbed; StringID manager is synthesized from Pencil2 and 14; names. Transitive native harness and runtime libraries are not fully pinned.'}


print(json.dumps(compact, indent=2))
