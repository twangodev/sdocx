#!/usr/bin/env python3
"""Replay pinned synthetic serialized BrushPen bytes in native Model/Drawing.

Uses project-local native DSOs and existing host adapters; writes JSON only to
stdout. Run with PYTHONPATH=scratch/apk-analysis-runtime/python.
"""
import argparse, contextlib, hashlib, io, json, struct, sys, zlib, os
from pathlib import Path
parser=argparse.ArgumentParser()
parser.add_argument("case", choices=("v1", "v10"))
parser.add_argument("--repo", type=Path, default=Path.cwd())
parser.add_argument("--fixtures", type=Path)
parser.add_argument("--caller", type=Path, required=True, help="Path to the pinned project-local BrushPen caller helper")
parser.add_argument("--attachment-template", type=Path, required=True, help="Path to the pinned project-local serialized attachment helper")
parser.add_argument("--variant", choices=("base","size1","size4","size16","pressure","tilt","duplicate","nearzero","stationary","curveoff","boundary1.5896","boundary1.5898"), default="base")
parser.add_argument("--width", type=int, default=1920)
parser.add_argument("--height", type=int, default=1080)
args=parser.parse_args()
ROOT=args.repo.resolve()
HERE=args.fixtures.resolve() if args.fixtures else ROOT/'conformance/brushpen-serialized'
if not (0 < args.width < 65536 and 0 < args.height < 65536): parser.error('dimensions must be positive u16 values')
if args.case != 'v10' and (args.variant != 'base' or (args.width,args.height)!=(1920,1080)): parser.error('controls require v10')
v3=args.case == "v10"
OLD=args.attachment_template.resolve()
CALLER=args.caller.resolve()
table=(HERE/f'{args.case}-table.bin').read_bytes()
frame=(HERE/f'{args.case}-frame.bin').read_bytes()
newname='com.samsung.android.sdk.pen.pen.preload.BrushPen'
sha=lambda b:hashlib.sha256(b).hexdigest()
ledger=json.loads((HERE/'receipt.json').read_text())
receipt=ledger['cases'][args.case]
for name, digest in ledger['native_dso_sha256'].items():
    assert sha((ROOT/'scratch/apk-analysis-native/arm64-v8a'/name).read_bytes()) == digest, name
assert sha(table)==receipt['table_sha256'] and sha(frame)==receipt['frame_sha256']
assert sha(CALLER.read_bytes())=='66f4790e8eeb1ca867f3273a796303b6ffb108286916e1e245fc063cbd744ae0'
assert sha(OLD.read_bytes())=='4dbb6400cfe0032471aa61091f2035f86530e8f5086c9d24b7a448f3eab531c1'
MODEL_SHA='4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a'
CPP_SHA='4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'
# Reuse the established native table/attachment sequence, replacing its InkPen draw accounting.
s=OLD.read_text(); template=s.split("    inject=f'''",1)[1].split("'''\n    src=",1)[0]
template=template.split(' _factory_before=',1)[0]+'''\n snapshot_object_stroke('serialized',_obj)
 _before=len(factory_created_pens); phase='serialized_drawing'\n p.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[p.drawing,p.draw_canvas,p.pen_canvas,_obj,1],limit=5000000); _draw=p.x(0)\n _entries=[name for row in native_draw_entries if row['phase']=='serialized_drawing' for name in row['signatures'] if '::RedrawPen(' in name]\n _probe_resolved_result['frame_hex']=_frame.hex(); _probe_resolved_result['stream_hex']=b''.join(bytes.fromhex(e['delta_bytes']) for e in brushpen_buffer_events if e['phase']=='serialized_drawing').hex(); _probe_resolved_result['drawing']={{'return':_draw,'factory_count':len(factory_created_pens)-_before,'redraw_entries':_entries,'native_color':hex(_native_color),'stream_sha256':hashlib.sha256(b''.join(bytes.fromhex(e['delta_bytes']) for e in brushpen_buffer_events if e['phase']=='serialized_drawing')).hexdigest(),'stream_count':len([e for e in brushpen_buffer_events if e['phase']=='serialized_drawing']),'native_snapshot':object_stroke_snapshots[-1],'native_arrays':{{k:{{'count':v['count'],'sha256':hashlib.sha256(bytes.fromhex(v['bytes'])).hexdigest()}} for k,v in _array_state.items()}}}}\n raise SystemExit(77)\n'''
inject=eval('f'+repr(template),globals()).replace('\\n # Native', '\n # Native', 1)
mutation = r"""
 if variant != 'base':
  p.call('SPen::ObjectStroke::GetPointCount() const',[_obj]); count=p.x(0)
  if count != 204: raise RuntimeError('unexpected point count before control mutation')
  p.call('SPen::ObjectStroke::GetPoint() const',[_obj]); point_ptr=p.x(0)
  p.call('SPen::ObjectStroke::GetTilt() const',[_obj]); tilt_ptr=p.x(0)
  p.call('SPen::ObjectStroke::GetPressure() const',[_obj]); pressure_ptr=p.x(0)
  if not all((point_ptr, tilt_ptr, pressure_ptr)): raise RuntimeError('null native channel pointer')
  if variant == 'tilt':
   p.u.mem_write(tilt_ptr+4,struct.pack('<f',0.2)); p.u.mem_write(tilt_ptr+8,struct.pack('<f',0.8))
  if variant == 'pressure':
   p.u.mem_write(pressure_ptr+4,struct.pack('<f',0.4)); p.u.mem_write(pressure_ptr+8,struct.pack('<f',0.8))
  if variant in ('size1','size4','size16'):
   p.f(0,float(variant[4:])); p.call('SPen::ObjectStroke::SetPenSize(float)',[_obj])
  if variant == 'nearzero':
   x,y=struct.unpack('<ff',p.u.mem_read(point_ptr,8)); p.u.mem_write(point_ptr+8,struct.pack('<ff',x+0.001,y))
  if variant == 'stationary':
   p.u.mem_write(point_ptr+8*50,bytes(p.u.mem_read(point_ptr+8*49,8))*12)
  if variant == 'curveoff':
   p.call('SPen::ObjectStroke::SetCurveEnabled(bool)',[_obj,0])
   spec=json.loads(os.environ['PROBE_POINTS'])
   for i,xy in enumerate(spec): p.u.mem_write(point_ptr+8*i,struct.pack('<ff',*xy))
  if variant in ('boundary1.5896','boundary1.5898'):
   x,y=struct.unpack('<ff',p.u.mem_read(point_ptr,8)); delta=float(variant[len('boundary'):]); p.u.mem_write(point_ptr+8,struct.pack('<ff',x+delta,y))
  if variant == 'duplicate': p.u.mem_write(point_ptr+16,bytes(p.u.mem_read(point_ptr,8)))
 """
if inject.count(' snapshot_object_stroke(')!=1: raise RuntimeError('missing unique mutation anchor')
inject=inject.replace(' snapshot_object_stroke(',mutation+'\n snapshot_object_stroke(')
src=CALLER.read_text(); anchor=" p.call('SPen::ObjectStroke::GetAdvancedPenSetting() const',[obj]); advanced_getter=p.x(0);"
assert src.count(anchor)==1
src=src.replace(anchor,inject+'\n'+anchor)
sys.argv=[str(CALLER),'--print-capture','--advanced-setting','null','--width',str(args.width),'--height',str(args.height)]
ns={'__name__':'__main__','__file__':str(CALLER),'variant':args.variant}
try:
 with contextlib.redirect_stdout(io.StringIO()),contextlib.redirect_stderr(io.StringIO()): exec(compile(src,str(CALLER),'exec'),ns)
except SystemExit as e:
 if e.code!=77: raise
out=ns['_probe_resolved_result']; assert out['pen_name_utf8']==newname and out['advanced_setting_utf8']==('10;' if v3 else None)
assert out['drawing']['return'] == 1
assert any(('BrushPenStrokeDrawableGLV10::RedrawPen' if v3 else 'BrushPenStrokeDrawableGLV1::RedrawPen') in x for x in out['drawing']['redraw_entries'])
stream=bytes.fromhex(out['stream_hex'])
if args.variant=='base' and (args.width,args.height)==(1920,1080):
 assert len(stream)==receipt['native_stream_bytes']
 assert sha(stream)==receipt['native_stream_sha256']
 assert stream==zlib.decompress((HERE/f'{args.case}-native-stream.zlib').read_bytes())
# modified source arrays
assert out['drawing']['native_color']==receipt['model_color']
# Keep native source arrays for decoded channel comparison.
out['provenance']={'table_sha256':sha(table),'frame_sha256':sha(frame),'caller_sha256':sha(CALLER.read_bytes()),'template_sha256':sha(OLD.read_bytes()),'input':'synthetic FountainPen-derived serialized frame, BrushPen name and '+('explicit 10; and black' if v3 else 'absent advanced setting'),'boundary':'native string table, ModelContext, binary handler, OnAttach, getters and Drawing execute under host Unicorn adapters; no complete note loader or device paint'}
print(json.dumps(out,indent=2,sort_keys=True))
