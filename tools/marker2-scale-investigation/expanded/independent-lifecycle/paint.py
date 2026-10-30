"""Capture bounded Marker2 RTV2 mask and composite readbacks for vector work.

This is a fresh ignored runner derived from the frozen zero-paint component
harness. It keeps its PNGs/receipts outside conformance and never updates an
existing oracle. Run --matrix only after reviewing the exact output directory.
"""
import argparse, ast, base64, contextlib, hashlib, importlib.util, io, json, os, re, struct, subprocess, sys, tempfile, types, zlib
from pathlib import Path
REPO=Path(__file__).resolve().parents[4]
ROOT=REPO/'tmp/vector-parity-plan/inkpen2-current-a241793'
LIB=ROOT/'scratch/apk-analysis-native/arm64-v8a'
MANIFEST=Path(__file__).with_name('manifest.json')
PINNED_LIBRARIES={
 'libSPenBase.so':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb',
 'libSPenComposer.so':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f',
 'libSPenDrawing.so':'788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd',
 'libSPenFountainPen.so':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec',
 'libSPenGraphics.so':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621',
 'libSPenMarker2.so':'f1b2ceea921baac78b722cbd5423732f42d7bec19ccb63d428cf429582412269',
 'libSPenModel.so':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a',
 'libSPenPenCommon.so':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d',
 'libSPenRenderer.so':'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed',
 'libc++_shared.so':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4',
}
PINNED_HELPERS={
 'marker2-saved-caller-native.json':'3f8ae6cc90ddc6bcef07809ea22ad9bd5b41b55b6fc53804b3a4a296972cfd43',
 'marker2_saved_caller_native.py':'01dc7f77dff725d5959d0c4a7533bee0cd0c8df948a5b1dbf7203569eb29a2c9',
 'marker3_paint_scale_native.py':'9b1eafd3dd07d1b491148d2635aad004c83ac401416f3c8a9c62b1e60099b7a8',
 'oblique_canvas_native.py':'1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2',
 'elf_plt.py':'816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b',
 'fountain_native.py':'40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30',
 'airbrush_texture_native.py':'76bb130daef795be53296cff4f778be6e7be5f487c8b1d4c4b6d159bb688e470',
 'inkpen_paint_native.py':'a3e65b2b37b2913b4bb2787b72ab2dbad0a9660cb66435b142f5acfa4d5051ae',
}
WITNESS_DOC=REPO/'tmp/vector-parity-plan/marker2-empty-paint/witness-before-empty-controls.md'
PINNED_WITNESS_SHA256='303803a65c7025bdca0fd121010ec3651214e690777a1d64880644ae7b27dd10'
PROJECTION_CONTROL=REPO/'tmp/vector-parity-plan/pen-projection-contract/matrix-product-control.json'
PINNED_PROJECTION_CONTROL_SHA256='aab355ec274e84f5f4351262348bf9d0e6fc35a64c7368ac4ec8b8a19bb21867'

def sha256_file(path):
 return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def build_caller_source(caller_original,color_argb):
 source=caller_original.decode('utf-8').replace('[[10.0,20.0],[18.0,25.0],[26.0,20.0],[34.0,25.0]]', '[[10.0,20.0],[12.0,33.0],[28.0,12.0],[34.0,25.0]]')
 replacements=(
  ("FIXTURE=ROOT/'conformance/marker2-saved-caller-native.json'",'FIXTURE=SOURCE_FIXTURE',1),
  ("print(f'wrote {FIXTURE.relative_to(ROOT)} case {slot}',_always=True)","print(f'wrote temporary source case {slot}',_always=True)",1),
  ('0xff336699',f'0x{color_argb:08x}',2),
 )
 for old,new,count in replacements:
  if source.count(old)!=count: raise RuntimeError(f'caller transform precondition failed for {old!r}: expected {count}, got {source.count(old)}')
  source=source.replace(old,new)
 if source.count(f'0x{color_argb:08x}')!=2:
  raise RuntimeError('caller color transform did not replace the exact two native color literals')
 ast.parse(source,filename='marker2_saved_caller_native.transformed.py')
 return source

def png_bytes(width,height,raw,channels):
 stride=width*channels
 if len(raw)!=stride*height: raise RuntimeError('PNG input plane has an unexpected byte length')
 scanlines=b''.join(b'\0'+raw[(height-1-y)*stride:(height-y)*stride] for y in range(height))
 color_type=0 if channels==1 else 6
 def chunk(kind,payload):
  return struct.pack('>I',len(payload))+kind+payload+struct.pack('>I',zlib.crc32(kind+payload)&0xffffffff)
 return (b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',width,height,8,color_type,0,0,0))+
         chunk(b'IDAT',zlib.compress(scanlines,9))+chunk(b'IEND',b''))

def verify_png_roundtrip():
 # Decode only the writer's constrained filter-0 output and verify dimensions,
 # chunk CRCs, channel type, decompression, and top-down row reversal.
 for width,height,channels in ((7,5,1),(5,3,4)):
  raw=bytes((i*37+11)&255 for i in range(width*height*channels))
  encoded=png_bytes(width,height,raw,channels)
  pos=8; chunks=[]; ihdr=None; compressed=bytearray()
  while pos<len(encoded):
   size=struct.unpack_from('>I',encoded,pos)[0];kind=encoded[pos+4:pos+8]
   payload=encoded[pos+8:pos+8+size]; crc=struct.unpack_from('>I',encoded,pos+8+size)[0]
   if zlib.crc32(kind+payload)&0xffffffff!=crc: raise RuntimeError(f'PNG self-check CRC mismatch in {kind!r}')
   chunks.append(kind)
   if kind==b'IHDR': ihdr=struct.unpack('>IIBBBBB',payload)
   if kind==b'IDAT': compressed.extend(payload)
   pos+=12+size
  if chunks!=[b'IHDR',b'IDAT',b'IEND'] or ihdr!=(width,height,8,0 if channels==1 else 6,0,0,0):
   raise RuntimeError('PNG self-check chunk envelope or IHDR mismatch')
  scanlines=zlib.decompress(compressed); stride=width*channels
  if len(scanlines)!=(stride+1)*height: raise RuntimeError('PNG self-check decompressed row length mismatch')
  rows=[]
  for y in range(height):
   row=scanlines[y*(stride+1):(y+1)*(stride+1)]
   if row[0]!=0: raise RuntimeError('PNG writer emitted an unsupported filter')
   rows.append(row[1:])
  restored=b''.join(reversed(rows))
  if restored!=raw: raise RuntimeError('PNG writer roundtrip changed pixels or row orientation')

def run_preflight():
 runner_hash=sha256_file(__file__)
 actual_libs={name:sha256_file(LIB/name) for name in PINNED_LIBRARIES}
 actual_helpers={name:sha256_file(ROOT/'conformance'/name) for name in PINNED_HELPERS}
 if actual_libs!=PINNED_LIBRARIES: raise RuntimeError(f'native library pins changed: {actual_libs!r}')
 if actual_helpers!=PINNED_HELPERS: raise RuntimeError(f'helper pins changed: {actual_helpers!r}')
 if not MANIFEST.is_file(): raise RuntimeError(f'frozen capture manifest missing: {MANIFEST}')
 manifest=json.loads(MANIFEST.read_text())
 witness_hash=sha256_file(WITNESS_DOC)
 projection_hash=sha256_file(PROJECTION_CONTROL)
 expected={'schema':'marker2-rtv2-mask-matrix-freeze-v2','runner_sha256':runner_hash,
           'libraries_sha256':PINNED_LIBRARIES,
           'helpers_sha256':{f'conformance/{name}':digest for name,digest in PINNED_HELPERS.items()},
           'witnesses_sha256':{'docs/reverse-engineering/marker2-rendering-findings.md':PINNED_WITNESS_SHA256},
           'controls_sha256':{'tmp/vector-parity-plan/pen-projection-contract/matrix-product-control.json':PINNED_PROJECTION_CONTROL_SHA256}}
 if manifest!=expected: raise RuntimeError('frozen manifest does not exactly match runner and source pins')
 if witness_hash!=PINNED_WITNESS_SHA256: raise RuntimeError(f'Marker2 width-1 native witness doc changed: {witness_hash}')
 witness_text=WITNESS_DOC.read_text()
 for claim in ('(128.25,128.5)','85,215','Drawing size | V1 scale | V2 scale'):
  if claim not in witness_text: raise RuntimeError(f'Marker2 native width-1 witness claim missing: {claim}')
 projection_control=json.loads(PROJECTION_CONTROL.read_text())
 caller_basis=projection_control['caller_matrix_column_major']
 if (projection_hash!=PINNED_PROJECTION_CONTROL_SHA256 or
     not projection_control.get('instruction_equivalent_matches_formula') or
     caller_basis[12:14]!=[-5,13]):
  raise RuntimeError('pinned column-major projection control no longer validates translation at Matrix4 elements 12/13')
 caller_path=ROOT/'conformance/marker2_saved_caller_native.py'
 caller_original=caller_path.read_bytes()
 ast.parse(caller_original,filename=str(caller_path))
 # Confirm the pinned native caller's cumulative saved-redraw snapshots end at
 # exactly 22 samples; this check launches no guest.
 fixture=json.loads((ROOT/'conformance/marker2-saved-caller-native.json').read_text())
 baseline=fixture['cases']['marker2_nontrivial_saved3:2:tool2:curve1:size00002041']
 baseline_vector=baseline['phase_final_vectors']['saved_redraw']
 saved_rows=[row for row in baseline['callback_add_point_calls'] if row['phase']=='saved_redraw']
 if (baseline_vector['vector_count']!=22 or len(bytes.fromhex(baseline_vector['vector_bytes']))!=22*8 or
     len(saved_rows)!=22 or [row['vector_count'] for row in saved_rows]!=list(range(1,23)) or
     [len(row['xy']) for row in saved_rows]!=list(range(1,23))):
  raise RuntimeError('frozen native caller baseline no longer proves the 22-sample saved-redraw stream')
 if [row['sample_end'] for row in baseline['sample_boundaries_from_native_drawLine_returns'] if row['phase']=='saved_redraw'][-1:]!=[22]:
  raise RuntimeError('frozen native caller baseline saved-redraw final boundary is not 22')
 source=build_caller_source(caller_original,(255<<24)|0x994422)
 ast.parse(source,filename='marker2_saved_caller_native.transformed.py')
 baseline_bytes=bytes.fromhex(baseline_vector['vector_bytes'])
 baseline_xy=[struct.unpack_from('<2f',baseline_bytes,i) for i in range(0,len(baseline_bytes),8)]
 for phase,delta,expected_xy in (
  ('canonical',(118.25,108.5),(128.25,128.5)),
  ('translated',(118.5,109.0),(128.5,129.0)),
 ):
  caller_matrix=[1.0,0.0,0.0,0.0,0.0,1.0,0.0,0.0,0.0,0.0,1.0,0.0,delta[0],delta[1],0.0,1.0]
  transformed=b''.join(struct.pack('<2f',x,y) for x,y in baseline_xy)
  projected=(struct.unpack('<f',struct.pack('<f',baseline_xy[0][0]+caller_matrix[12]))[0],
             struct.unpack('<f',struct.pack('<f',baseline_xy[0][1]+caller_matrix[13]))[0])
  replayed=b''.join(struct.pack('<2f',x,y) for x,y in baseline_xy)
  if len(transformed)!=22*8 or transformed!=replayed: raise RuntimeError(f'{phase} caller-matrix phase changed the native source vector')
  if projected!=expected_xy: raise RuntimeError(f'{phase} caller-matrix phase does not preserve its declared f32 alignment')
 verify_png_roundtrip()
 return {'runner_sha256':runner_hash,'manifest_sha256':sha256_file(MANIFEST),
         'pinned_libraries':len(actual_libs),'pinned_helpers':len(actual_helpers),
         'pinned_witness_doc':witness_hash,'pinned_projection_control':projection_hash,
         'baseline_saved_redraw_samples':22,'phase_f32_controls':2,'png_roundtrips':2,
         'guest_launched':False,'gles_initialized':False}

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--preflight',action='store_true',help='verify frozen sources and pure-Python controls without loading Unicorn or initializing GL')
parser.add_argument('--matrix',action='store_true',help='capture widths 1/2/3 x canonical/translated x alpha 255/128 sequentially')
parser.add_argument('--one-case',action='store_true',help=argparse.SUPPRESS)
parser.add_argument('--size',choices=('1','2','3'))
parser.add_argument('--phase',choices=('canonical','translated'))
parser.add_argument('--alpha',choices=('128','255'))
parser.add_argument('--output-dir',type=Path)
CLI=parser.parse_args()
if CLI.preflight:
 if CLI.matrix or CLI.one_case or CLI.size or CLI.phase or CLI.alpha or CLI.output_dir: parser.error('--preflight cannot be combined with capture selectors')
 print(json.dumps(run_preflight(),sort_keys=True))
 raise SystemExit(0)
if not CLI.output_dir: parser.error('--output-dir is required for capture')
PRECHECK=run_preflight()
from unicorn import arm64_const as R
for filename,digest in PINNED_LIBRARIES.items():
 actual=sha256_file(LIB/filename)
 if actual!=digest: raise RuntimeError(f'unpinned native library {filename}: {actual}')
for filename,digest in PINNED_HELPERS.items():
 actual=sha256_file(ROOT/'conformance'/filename)
 if actual!=digest: raise RuntimeError(f'unpinned helper {filename}: {actual}')
sys.path.insert(0,str(ROOT/'conformance'))
if CLI.matrix:
 if CLI.one_case or CLI.size or CLI.phase or CLI.alpha: parser.error('--matrix cannot be combined with individual case selectors')
 CLI.output_dir=CLI.output_dir.resolve()
 if CLI.output_dir.exists() and any(CLI.output_dir.iterdir()): parser.error(f'output directory must be empty: {CLI.output_dir}')
 CLI.output_dir.mkdir(parents=True,exist_ok=True)
 records=[]
 for size in ('1','2','3'):
  for phase in ('canonical','translated'):
   for alpha in ('255','128'):
    command=[sys.executable,str(Path(__file__).resolve()),'--one-case','--size',size,'--phase',phase,'--alpha',alpha,'--output-dir',str(CLI.output_dir)]
    subprocess.run(command,check=True,env=os.environ.copy())
    case_key=f'w{size}-{phase}-a{alpha}'
    record=json.loads((CLI.output_dir/f'{case_key}.json').read_text())
    records.append({'case':case_key,'receipt_sha256':hashlib.sha256((CLI.output_dir/f'{case_key}.json').read_bytes()).hexdigest(),
                    'mask_png_sha256':record['mask']['png_sha256'],'composite_png_sha256':record['composite']['png_sha256']})
 receipt={'schema':'marker2-rtv2-mask-matrix-v2','case_count':len(records),'expected_output_files':37,
          'freeze_manifest_sha256':sha256_file(MANIFEST),'preflight':PRECHECK,'cases':records,
          'scope':{'native':'Pinned ARM64 Marker2 Drawing and RTV2 methods run under Unicorn; synthetic ObjectStroke inputs and profile.',
                   'guest_egl_callback':'eglGetCurrentContext is a host callback returning guest token 0xabc; this is not proof of a native Android EGL context.',
                   'host_services':['Task queue registration/unregistration uses explicit host hooks.','Captured saved-redraw callback points are explicitly replayed once with native AddPoint.','The exact native vector bytes are explicitly materialized at native RT+88 by the host.','Fractional phase is applied through the caller Matrix4 so native mask projection and native bounds projection receive the same transform.'],
                   'gles':'Real host GLES/Mesa performs rasterization; this does not establish Android device EGL/GPU behavior.',
                   'exclusions':['No Android device renderer parity.','No complete Samsung Notes page composition.','No archive deserialization claim.']}}
 dest=CLI.output_dir/'matrix.json'
 if dest.exists(): parser.error(f'refusing to overwrite {dest}')
 dest.write_text(json.dumps(receipt,indent=2,sort_keys=True)+'\n')
 print(json.dumps({'output_dir':str(CLI.output_dir),'cases':len(records),'matrix_sha256':hashlib.sha256(dest.read_bytes()).hexdigest()},sort_keys=True))
 raise SystemExit(0)
if not CLI.one_case or not all((CLI.size,CLI.phase,CLI.alpha)):
 parser.error('select --matrix or provide --one-case with --size, --phase, and --alpha')
CLI.output_dir=CLI.output_dir.resolve(); CLI.output_dir.mkdir(parents=True,exist_ok=True)
SIZE=int(CLI.size); TOOL='2'; ADVANCED_SETTING='empty'; COLOR_ARGB=(int(CLI.alpha)<<24)|0x994422
CASE_KEY=f'w{SIZE}-{CLI.phase}-a{CLI.alpha}'
if any((CLI.output_dir/name).exists() for name in (f'{CASE_KEY}.json',f'{CASE_KEY}-mask-red.png',f'{CASE_KEY}-composite-rgba.png',
                                                       f'{CASE_KEY}-failure-v2.json',f'{CASE_KEY}-failure-mask-red.png',f'{CASE_KEY}-failure-composite-rgba.png')):
 parser.error(f'refusing to overwrite existing capture {CASE_KEY} in {CLI.output_dir}')
# Keep the one-case source caller's generated output outside the repository and
# remove it automatically; it is not a conformance dependency or sidecar.
source_tmp=tempfile.TemporaryDirectory(prefix='marker2-source-case-')
source_fixture=Path(source_tmp.name)/'source-case.json'
caller_path=ROOT/'conformance/marker2_saved_caller_native.py'
caller_original=caller_path.read_bytes()
caller_source=build_caller_source(caller_original,COLOR_ARGB)
if 'FIXTURE=SOURCE_FIXTURE' not in caller_source:
 raise RuntimeError('temporary caller transform did not isolate its output')
compile(caller_source,str(caller_path),'exec')
caller_transform_sha256=hashlib.sha256(caller_source.encode()).hexdigest()
# Execute the selected Marker2 caller into its pinned native ObjectDrawing and RTV.
sys.argv=['marker2-saved-caller','--write-fixture','--case','marker2_nontrivial_saved3','--advanced-setting',ADVANCED_SETTING,'--size',CLI.size,'--tool',TOOL,'--curve','1']
spec=importlib.util.spec_from_file_location('m2caller',caller_path); m=importlib.util.module_from_spec(spec); m.__file__=str(caller_path); m.SOURCE_FIXTURE=source_fixture
with contextlib.redirect_stdout(io.StringIO()): exec(compile(caller_source,str(caller_path),'exec'),m.__dict__)
p=m.p; rt=m.selected_rt
first_rows=list(m.captured_callbacks); first_vector=m.captured_final_vectors['saved_redraw']['vector_bytes']
obj2=p.alloc(0x100); impl2=p.alloc(0x400)
p.call('SPen::ObjectStroke::ObjectStroke()', [obj2]); p.call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)',[impl2,obj2]); p.put(obj2+32,'Q',impl2)
p.f(0,SIZE); p.call('SPen::ObjectStroke::SetPenSize(float)',[obj2])
p.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj2,2]); p.call('SPen::ObjectStroke::SetStrokeType(SPen::StrokeType)',[obj2,m.input_case['stroke_type']])
p.call('SPen::ObjectStroke::SetColor(unsigned int)',[obj2,COLOR_ARGB]); p.call('SPen::ObjectStroke::SetCurveEnabled(bool)',[obj2,1])
p.call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj2,m.pts,m.prs,m.ts,m.tilt,m.orient,m.count])
p.call('SPen::ObjectBase::SetVisibility(bool)',[obj2,1]); p.call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj2,m.pname]); p.call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[obj2,m.advanced])
drawing2=p.alloc(256); p.call('SPen::ObjectDrawing::ObjectDrawing()',[drawing2]); p.call('SPen::ObjectDrawing::Construct(SPen::IContext const*)',[drawing2,p.context])
m.phase='draw_object_stroke'; before=len(m.factory_created_pens)
p.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[drawing2,p.draw_canvas,p.pen_canvas,obj2,1],limit=5000000)
if p.x(0)!=1 or len(m.factory_created_pens)!=before+1: raise RuntimeError('second Drawing admission did not produce a new pen')
pen2=m.factory_created_pens[-1]; p.call('SPen::Marker2::GetStrokeDrawableGL()',[pen2]); drawable2=p.x(0); rt2=p.get(drawable2+16,'Q')[0]
if len({m.selected_pen,pen2})!=2 or len({rt,rt2})!=2: raise RuntimeError('second native pen or renderer is not independent')
subvt=p.get(drawable2+8,'Q')[0]; redraw2=p.get(subvt+40,'Q')[0]; m.phase='saved_redraw'; m.call_address(redraw2,[drawable2+8,obj2,m.rect,m.rect])
if p.x(0)!=1: raise RuntimeError('second saved redraw failed')
second_vector=bytes.fromhex(m.captured_final_vectors['saved_redraw']['vector_bytes']); print('SECOND_VECTOR',len(second_vector),hashlib.sha256(second_vector).hexdigest(),'FIRST',len(bytes.fromhex(first_vector)),hashlib.sha256(bytes.fromhex(first_vector)).hexdigest(),flush=True)
if second_vector != bytes.fromhex(first_vector): raise RuntimeError('independent native vectors differ')
rows=first_rows
source_vector_data=bytes.fromhex(first_vector)
source_xy=[struct.unpack_from('<2f',source_vector_data,i) for i in range(0,len(source_vector_data),8)]
saved_redraw_rows=[row for row in rows if row['phase']=='saved_redraw']
if len(saved_redraw_rows)!=len(source_xy) or [row['vector_count'] for row in saved_redraw_rows]!=list(range(1,len(source_xy)+1)):
 raise RuntimeError(f'expected cumulative saved-redraw snapshots, vector has {len(source_xy)}')
if [tuple(xy) for xy in saved_redraw_rows[-1]['xy']]!=[tuple(xy) for xy in source_xy]:
 raise RuntimeError('final native saved-redraw callback snapshot differs from captured final vector')
for index,row in enumerate(saved_redraw_rows,1):
 if [tuple(xy) for xy in row['xy']]!=[tuple(xy) for xy in source_xy[:index]]:
  raise RuntimeError(f'saved-redraw callback snapshot {index} is not the expected cumulative source prefix')
# Preserve native source XY bytes. Apply phase in Draw's caller matrix so the
# same transform also reaches native bounds used to build the composite quad.
dx,dy=(107.83,87.17) if CLI.phase=='canonical' else (118.37,108.23)
vector_data=source_vector_data
# Bring the paint oracle's GL callback methods into this emulator without running its separate RT pipeline.
import ctypes as C,zlib,airbrush_texture_native as at,inkpen_paint_native as ip,fountain_native as fn
ARGS=types.SimpleNamespace(width=256,height=256)
source=ROOT/'conformance/marker3_paint_scale_native.py'; tree=ast.parse(source.read_text())
klass=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='P')
env={'at':at,'fn':fn,'ip':ip,'C':C,'zlib':zlib,'hashlib':hashlib,'struct':struct,'subprocess':subprocess,'re':re,'json':json,'Path':Path,'LIB':LIB,'ARGS':ARGS,'SOURCE_HASHES':{},'SOURCE_FILES':{},'sys':sys,'ROOT':ROOT,'base64':__import__('base64')}
exec(compile(ast.Module(body=[klass],type_ignores=[]),str(source),'exec'),env)
Paint=env['P']
for owner in (at.TextureOracle,Paint):
 for name,value in owner.__dict__.items():
  if callable(value) and not name.startswith('__'): setattr(p,name,types.MethodType(value,p))
p.gles=ip.GLES(); p.errno_ptr=p.alloc(4); p.gl_calls=[]; p.shader_sources={}; p.shader_source_calls=[]; p.native_samplers={};p.native_syncs={};p.native_logs=[];p.next_gl_id=100
p.callbacks['glGetString']=p.get_string
p.callbacks['glGetIntegerv']=p.get_integer
p.callbacks['glGetError']=lambda:p.x(0,p.gles.f['glGetError']())
p.callbacks['glPixelStorei']=lambda:(p.gl_calls.append({'call':'glPixelStorei','pname':p.x(0),'value':p.x(1)}),p.gles.f['glPixelStorei'](p.x(0),p.x(1)))
p.callbacks['glDeleteTextures']=lambda:(lambda n,ptr: (lambda ids: (ip.cfunc(p.gles.gl,'glDeleteTextures',None,[C.c_int,C.POINTER(C.c_uint)])(n,(C.c_uint*n)(*ids)),p.gl_calls.append({'call':'glDeleteTextures','ids':ids})))(list(p.get(ptr,'I'*n))))(p.x(0),p.x(1))
p.callbacks['glDeleteSamplers']=lambda:(lambda n,ptr: (lambda ids: (p._sync_proc('glDeleteSamplers',None,[C.c_int,C.POINTER(C.c_uint)])(n,(C.c_uint*n)(*[p.native_samplers.pop(i) for i in ids])),p.gl_calls.append({'call':'glDeleteSamplers','ids':ids})))(list(p.get(ptr,'I'*n))))(p.x(0),p.x(1))
p.callbacks['eglGetCurrentContext']=lambda:p.x(0,0xabc)
p.callbacks['strstr']=p.strstr; p.callbacks['strncpy']=p.strncpy
p.callbacks['__strncpy_chk']=lambda: (p.u.mem_write(p.x(0),bytes(p.u.mem_read(p.x(1),p.x(2)))),p.x(0,p.x(0)))
# Evaluate the exact GL entrypoint callback map from the pinned paint adapter.
init=next(n for n in klass.body if isinstance(n,ast.FunctionDef) and n.name=='__init__')
updates=[]
for n in ast.walk(init):
 if isinstance(n,ast.Call) and isinstance(n.func,ast.Attribute) and n.func.attr=='update' and isinstance(n.func.value,ast.Attribute) and n.func.value.attr=='callbacks': updates.append(n)
entry=next(n.args[0] for n in updates if isinstance(n.args[0],ast.Dict) and any(isinstance(k,ast.Constant) and k.value=='glCreateShader' for k in n.args[0].keys)); callbacks=eval(compile(ast.Expression(entry),'paint-callback-map','eval'),env|{'self':p}); p.callbacks.update(callbacks)
for n in ast.walk(init):
 if isinstance(n,ast.Assign) and any(isinstance(t,ast.Subscript) and isinstance(t.value,ast.Attribute) and t.value.attr=='callbacks' for t in n.targets):
  key=n.targets[0].slice
  if isinstance(key,ast.Constant) and isinstance(key.value,str): p.callbacks[key.value]=eval(compile(ast.Expression(n.value),'paint-callback-assignment','eval'),env|{'self':p})
# Retain the shader-name to uniform-location binding and its exact GL upload.
_get_uniform_original=p.callbacks['glGetUniformLocation']
def _get_uniform_capture():
 program=p.x(0); name=p.cstr(p.x(1),256).decode(errors='replace')
 _get_uniform_original(); location=p.x(0)
 p.gl_calls.append({'call':'glGetUniformLocation','program':program,'name':name,'location':location})
p.callbacks['glGetUniformLocation']=_get_uniform_capture
for name,method in [('glGenSamplers','gl_gen_samplers'),('glSamplerParameteri','gl_sampler_parameter'),('glFenceSync','gl_fence_sync'),('glClientWaitSync','gl_client_wait_sync'),('glWaitSync','gl_wait_sync'),('glDeleteSync','gl_delete_sync')]: p.callbacks[name]=getattr(p,method)
p.callbacks['glGetActiveUniform']=lambda:(setattr(p,'current_plt_symbol','glGetActiveUniform'),p.gl_get_active())
p.callbacks['glGetActiveAttrib']=lambda:(setattr(p,'current_plt_symbol','glGetActiveAttrib'),p.gl_get_active())
# Draw records include the actual Mesa state at each native mask/composite stage.
_basic_gl_state=p.gl_state
def _joined_gl_state(self):
 state=_basic_gl_state()
 get_i=C.CFUNCTYPE(None,C.c_uint,C.POINTER(C.c_int))(self.gles.gl.glGetIntegerv)
 get_b=C.CFUNCTYPE(None,C.c_uint,C.POINTER(C.c_ubyte))(self.gles.gl.glGetBooleanv)
 get_enabled=C.CFUNCTYPE(C.c_ubyte,C.c_uint)(self.gles.gl.glIsEnabled)
 viewport=(C.c_int*4)();get_i(0x0BA2,viewport);state['viewport']=list(viewport)
 state['blend_enabled']=bool(get_enabled(0x0BE2))
 for label,pname in [('blend_equation_rgb',0x8009),('blend_equation_alpha',0x883D),('blend_src_rgb',0x80C9),('blend_dst_rgb',0x80C8),('blend_src_alpha',0x80CB),('blend_dst_alpha',0x80CA)]:
  value=C.c_int();get_i(pname,C.byref(value));state[label]=value.value
 mask=(C.c_ubyte*4)();get_b(0x0C23,mask);state['color_mask']=[bool(x) for x in mask]
 return state
p.gl_state=types.MethodType(_joined_gl_state,p)
# Add pinned Renderer at a disjoint address, use exact dynsym addresses for defined relocations.
renderer=LIB/'libSPenRenderer.so'; blob=renderer.read_bytes(); base=0x9000000
ph=struct.unpack_from('<Q',blob,32)[0]; ent,n=struct.unpack_from('<HH',blob,54); extent=0
for i in range(n):
 t,fl,o,va,pa,fs,ms,al=struct.unpack_from('<II6Q',blob,ph+i*ent)
 if t==1:extent=max(extent,va+ms)
p.u.mem_map(base,(extent+0xfff)&~0xfff)
for i in range(n):
 t,fl,o,va,pa,fs,ms,al=struct.unpack_from('<II6Q',blob,ph+i*ent)
 if t==1 and ms:p.u.mem_write(base+va,blob[o:o+fs]);p.u.mem_write(base+va+fs,bytes(ms-fs) if ms>fs else b'')
envsub=dict(os.environ);envsub['LD_LIBRARY_PATH']=str((ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu').resolve())
for line in subprocess.check_output(['nm','-D','-C',str(renderer)],text=True,env=envsub).splitlines():
 q=line.strip().split(' ',2)
 if len(q)==3 and q[1] in ('T','W'):p.syms[q[2]]=base+int(q[0],16)
objdump=ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'
p.syms['typeinfo for SPen::IGLManagedMsgQueue']=base+0x6b400
for a,name in re.findall(r'^([0-9a-f]+) <(.+)@plt>:',subprocess.check_output([str(objdump),'-d','-C','-j','.plt',str(renderer)],text=True,env=envsub),re.M):p.plt[base+int(a,16)]=name
# Renderer external relocations are bound to pinned caller DSO exports; internal ABS64 use exact dynsym.
rawsyms={}
for line in subprocess.check_output(['nm','-D',str(renderer)],text=True,env=envsub).splitlines():
 q=line.split()
 if len(q)==3 and q[1].upper() in ('T','W','D','B','R','V'):rawsyms[q[2]]=base+int(q[0],16)
shoff=struct.unpack_from('<Q',blob,40)[0]; se,sn,ss=struct.unpack_from('<HHH',blob,58);secs=[struct.unpack_from('<IIQQQQIIQQ',blob,shoff+i*se) for i in range(sn)];names=blob[secs[ss][4]:secs[ss][4]+secs[ss][5]];by={names[x[0]:names.find(b'\0',x[0])].decode():x for x in secs};ds,st=by['.dynsym'],by['.dynstr'];strings=blob[st[4]:st[4]+st[5]];sy=[]
for off in range(ds[4],ds[4]+ds[5],ds[9]):
 no,inf,oth,ndx,val,size=struct.unpack_from('<IBBHQQ',blob,off);sy.append((strings[no:strings.find(b'\0',no)].decode() if no else '',val))
for secname in ('.rela.dyn','.rela.plt'):
 sec=by.get(secname)
 if not sec:continue
 for off in range(sec[4],sec[4]+sec[5],sec[9]):
  where,info,add=struct.unpack_from('<QQq',blob,off);typ,idx=info&0xffffffff,info>>32
  if typ==0x403:target=base+add
  elif typ in (0x401,0x402,0x101,0x1025,0x1026):
   name,val=sy[idx];target=base+val if val else rawsyms.get(name,0) or p.syms.get(name,0)
   if typ in (0x401,0x101):target+=add
  else:continue
  if target:p.u.mem_write(base+where,struct.pack('<Q',target))
# GL hosts/factory functions follow Marker3 paint adapter's normal setup.
p.callbacks.pop('SPen::RendererFactory::CreateGraphicsObject(unsigned int, SPen::PrimitiveType, SPen::VertexDescriptor const*, unsigned int, unsigned int)',None)
p.callbacks.pop('SPen::RendererFactory::CreateBlendStateObject(SPen::BlendStateDescriptor const&)',None)
# Basic Renderer dependency init array.
sec=by['.init_array']
for off in range(0,sec[5],8):
 target=struct.unpack('<Q',p.u.mem_read(base+sec[3]+off,8))[0]
 if target:
  for i in range(4):p.x(i,0)
  p.u.reg_write(R.UC_ARM64_REG_SP,0x200ef000);p.u.reg_write(R.UC_ARM64_REG_LR,m.oc.STOP);p.u.emu_start(target,m.oc.STOP,count=3000000)
  if p.u.reg_read(R.UC_ARM64_REG_PC)!=m.oc.STOP:raise RuntimeError(f'Renderer init failed at {target:#x}')
# Enforce an empty GL error queue before and after every wrapped GL entrypoint.
gl_error_checks=[]
for glname,glfn in list(p.callbacks.items()):
 if glname.startswith('gl') and glname!='glGetError':
  def make_gl_guard(name,fn):
   def guarded():
    before=p.gles.f['glGetError']()
    if before: gl_error_checks.append({'call':name,'when':'before','error':before}); raise RuntimeError(f'pending GL error before {name}: {before:#x}')
    fn()
    after=p.gles.f['glGetError']()
    gl_error_checks.append({'call':name,'when':'after','error':after})
    if after: raise RuntimeError(f'GL error after {name}: {after:#x}; gl_tail={p.gl_calls[-8:]!r}; state={p.gl_state()!r}')
   return guarded
  p.callbacks[glname]=make_gl_guard(glname,glfn)
_native_tex_image=p.callbacks['glTexImage2D']
def _record_bound_texture_tex_image():
 bound=p.gl_state()['texture2d']
 _native_tex_image()
 p.gl_calls[-1]['boundTexture']=bound
p.callbacks['glTexImage2D']=_record_bound_texture_tex_image
try:
 vao=C.c_uint(); C.CFUNCTYPE(None,C.c_int,C.POINTER(C.c_uint))(p.gles.gl.glGenVertexArrays)(1,C.byref(vao)); C.CFUNCTYPE(None,C.c_uint)(p.gles.gl.glBindVertexArray)(vao.value)
 if not vao.value: raise RuntimeError('host GLES failed to allocate the required VAO')
 p.call(f'SPen::Marker2StrokeDrawableRTV{m.selected_drawable_version}::Init()', [rt], limit=5000000)
 p.call(f'SPen::Marker2StrokeDrawableRTV{m.selected_drawable_version}::Init()', [rt2], limit=5000000)

except Exception as e:
 print('INIT FAIL',repr(e),'pc',hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'plt',p.plt.get(p.u.reg_read(R.UC_ARM64_REG_PC)),'entries',m.traced_methods[-20:]);raise
# Provide the task-queue base and managed-object subobject distinctly.
queue=m.queue; qvt=m.qvt
p.queue=queue; p.put(qvt-16,'qQ',0,p.syms['typeinfo for SPen::IGLManagedMsgQueue'])
queue_managed=p.alloc(16); managed_queue=p.alloc(32); managed_vt=p.alloc(64); managed_register=p.alloc(16); managed_unregister=p.alloc(16)
managed_tokens={}; managed_events=[]
p.put(queue_managed,'4I',0xd65f03c0,0,0,0) # function code ignored by address hook
p.put(managed_queue,'Q',managed_vt); p.put(managed_vt+16,'Q',managed_register);p.put(managed_vt+24,'Q',managed_unregister);p.put(qvt+56,'Q',queue_managed)
def queue_hooks(u,a,size,user):
 if a==queue_managed:
  p.x(0,managed_queue);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
 elif a==managed_register:
  manager0,obj=p.x(0),p.x(1); token=p.alloc(8);p.put(token,'Q',obj);managed_tokens[token]=obj;managed_events.append(['Register',manager0,obj,token]);p.x(0,token);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
 elif a==managed_unregister:
  manager0,token=p.x(0),p.x(1);obj=managed_tokens.pop(token,None);managed_events.append(['Unregister',manager0,obj,token]);p.x(0,0);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
p.u.hook_add(4,queue_hooks)
# Let native PenGLDataManager::GetPenCanvas create the actual internal canvas instead of CanvasProbe's matrix-only fake.
p.callbacks.pop('SPen::PenGLDataManager::GetPenCanvas(SPen::IGLMsgQueue*, SPen::String*, int, int, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)',None)
p.callbacks.pop('SPen::SPGraphicsFactory::CreatePenCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*, int)',None)
# Allocate Marker3's own mask canvas on the existing Drawing-selected RT.
try:p.call(f'SPen::Marker2StrokeDrawableRTV{m.selected_drawable_version}::CreatePenCanvas(int, int, SPen::IGLMsgQueue*)',[rt,256,256,queue],limit=5_000_000)
except Exception as e: print('CREATE_CANVAS_FAIL',repr(e),'pc',hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'lr',hex(p.u.reg_read(R.UC_ARM64_REG_LR)),'x0x8',[hex(p.x(i)) for i in range(9)],'stack',p.u.mem_read(p.u.reg_read(R.UC_ARM64_REG_SP),64).hex()); raise

p.call(f'SPen::Marker2StrokeDrawableRTV{m.selected_drawable_version}::CreatePenCanvas(int, int, SPen::IGLMsgQueue*)',[rt2,256,256,queue],limit=5_000_000)
# Build the independent native RGBA8 destination exactly as the scale oracle does.
fmt=p.alloc(8);p.u.mem_write(fmt,b'RGBA\0'); initial=p.alloc(256*256*4);p.u.mem_write(initial,bytes(256*256*4))
factory='SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)'
def call_stack(name,x,stack):
 for i,v in enumerate(x):p.x(i,v)
 sp=0x200ef000;p.u.reg_write(R.UC_ARM64_REG_SP,sp);p.u.mem_write(sp,struct.pack('<'+'Q'*len(stack),*stack));p.u.reg_write(R.UC_ARM64_REG_LR,m.oc.STOP);p.u.emu_start(p.syms[name],m.oc.STOP,count=5_000_000)
 if p.u.reg_read(R.UC_ARM64_REG_PC)!=m.oc.STOP:raise RuntimeError('stack call failed '+name)
call_stack(factory,[queue,256,256,fmt,initial,1,1024,1],[6,2]); outbmp=p.x(0)
if not outbmp:raise RuntimeError('external native RGBA bitmap factory returned null')
# Capture source-caller vector slots before adding the explicitly labeled host bridge.
pre80=p.get(rt+80,'Q')[0];pre88=p.get(rt+88,'Q')[0]
# Host materialization bridge: build the std::vector descriptor from the exact native saved-redraw callback bytes because the caller fixture clears this field before return. Native Draw consumes RT+88.
vector_obj=p.alloc(24); vector_storage=p.alloc(len(vector_data)); p.u.mem_write(vector_storage,vector_data); p.put(vector_obj,'3Q',vector_storage,vector_storage+len(vector_data),vector_storage+len(vector_data)); p.put(rt+88,'Q',vector_obj)
vector_obj2=p.alloc(24); vector_storage2=p.alloc(len(vector_data)); p.u.mem_write(vector_storage2,vector_data); p.put(vector_obj2,'3Q',vector_storage2,vector_storage2+len(vector_data),vector_storage2+len(vector_data)); p.put(rt2+88,'Q',vector_obj2)
if p.get(rt+88,'Q')[0]!=vector_obj:raise RuntimeError('Marker2 callback vector bridge failed')
canvas=p.alloc(512);p.call('SPen::SPCanvasImpl::SPCanvasImpl(SPen::IGLMsgQueue*, SPen::ISPBitmap*, bool)',[canvas,queue,outbmp,1],limit=5_000_000)
p.call('SPen::SPCanvasImpl::GetBitmap()',[canvas]);bitmap=p.x(0);p.call('SPen::SPTextureBitmap::GetSubBitmap(int)',[bitmap,0]);bitmap=p.x(0)

# The actual native callback queue has already run the destructor-created SendDataToGPU WorkerTask.
# Re-submit the final 22 native saved-redraw points exactly once. This probes
# the cleared RT+80 append vector; render input remains the host RT+88 bridge.
addpoint=f'SPen::Marker2StrokeDrawableRTV{m.selected_drawable_version}::AddPoint(SPen::Vector2<float>)'
replayed_samples=[]
for x,y in saved_redraw_rows[-1]['xy']:
  x=struct.unpack('<f',struct.pack('<f',float(x)))[0]; y=struct.unpack('<f',struct.pack('<f',float(y)))[0]
  p.x(0,rt); p.f(0,x); p.f(1,y); p.u.reg_write(R.UC_ARM64_REG_SP,0x200ef000); p.u.reg_write(R.UC_ARM64_REG_LR,m.oc.STOP); p.u.emu_start(p.syms[addpoint],m.oc.STOP,count=500000)
  if p.u.reg_read(R.UC_ARM64_REG_PC)!=m.oc.STOP: raise RuntimeError('native Marker2 AddPoint replay failed')
  replayed_samples.append([x,y])
if len(replayed_samples)!=len(source_xy):
 raise RuntimeError(f'expected exactly one replay of the final saved-redraw vector, observed {len(replayed_samples)}')
p.call(f'SPen::Marker2StrokeDrawableRTV{m.selected_drawable_version}::Update(bool)',[rt,0],limit=5_000_000)
for x,y in replayed_samples:
 p.x(0,rt2); p.f(0,x); p.f(1,y); p.u.reg_write(R.UC_ARM64_REG_SP,0x200ef000); p.u.reg_write(R.UC_ARM64_REG_LR,m.oc.STOP); p.u.emu_start(p.syms[addpoint],m.oc.STOP,count=500000)
 if p.u.reg_read(R.UC_ARM64_REG_PC)!=m.oc.STOP: raise RuntimeError('second native AddPoint replay failed')
p.call(f'SPen::Marker2StrokeDrawableRTV{m.selected_drawable_version}::Update(bool)',[rt2,0],limit=5_000_000)

# Identify and clear the actual external RGBA target attached by the native canvas.
output_uploads=[c for c in p.gl_calls if c['call']=='glTexImage2D' and c['hasPixels'] and c['format']==0x1908]
if len(output_uploads)!=1: raise RuntimeError(f'expected exactly one uploaded external RGBA target, saw {output_uploads!r}')
output_texture=output_uploads[0]['boundTexture']
output_attachments=[c for c in p.gl_calls if c['call']=='glFramebufferTexture2D' and c['args'][3]==output_texture]
if len(output_attachments)!=1: raise RuntimeError(f'external texture must attach to exactly one FBO: {output_attachments!r}')
output_fbo=output_attachments[0]['framebuffer']
def host_gl_checked(name,fn,*args):
 before=p.gles.f['glGetError']();gl_error_checks.append({'call':name,'when':'before','error':before})
 if before: raise RuntimeError(f'pending GL error before {name}: {before:#x}')
 result=fn(*args)
 after=p.gles.f['glGetError']();gl_error_checks.append({'call':name,'when':'after','error':after})
 if after: raise RuntimeError(f'GL error after {name}: {after:#x}')
 return result
host_gl_checked('hostBindOutputFbo',p.gles.f['glBindFramebuffer'],0x8d40,output_fbo)
target_status=host_gl_checked('hostCheckOutputFbo',p.gles.f['glCheckFramebufferStatus'],0x8d40)
if target_status!=0x8cd5: raise RuntimeError(f'external target FBO {output_fbo} incomplete: {target_status:#x}')
host_gl_checked('hostOutputViewport',p.gles.f['glViewport'],0,0,256,256)
host_gl_checked('hostClearColor',p.gles.f['glClearColor'],0.0,0.0,0.0,0.0)
host_gl_checked('hostClearOutput',p.gles.f['glClear'],0x00004000)
preclear=C.create_string_buffer(256*256*4); _read_rgba=C.CFUNCTYPE(None,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p)(p.gles.gl.glReadPixels)
host_gl_checked('hostReadOutputAfterClear',_read_rgba,0,0,256,256,0x1908,0x1401,preclear)
if any(preclear.raw): raise RuntimeError('external RGBA target was not fully transparent after clear')

# Column-major Matrix4 caller translation. Native Draw applies this to the
# subbitmap projection and to the native bounds-to-NDC composite quad.
caller_matrix_values=[2,0,0,0,0,2,0,0,0,0,1,0,dx,dy,0,1]
if os.environ.get('MARKER2_SHIFT_FIRST') == '1': caller_matrix_values[12]+=4.; caller_matrix_values[13]+=3.
mat=p.alloc(64);p.put(mat,'16f',*caller_matrix_values)
# Mesa core profile requires a bound VAO. Native Renderer expects its app host context to supply this state.
_real_draw_cb=p.callbacks['glDrawArraysInstanced']
draw_attribute_state={}
def _draw_attr_diag():
 global draw_attribute_state
 _getattr_now=C.CFUNCTYPE(None,C.c_uint,C.c_uint,C.POINTER(C.c_int))(p.gles.gl.glGetVertexAttribiv)
 _getprogram=C.CFUNCTYPE(None,C.c_uint,C.c_uint,C.POINTER(C.c_int))(p.gles.gl.glGetProgramiv)
 _state={}
 for _idx in range(4):
  _state[str(_idx)]={}
  for _label,_pname in [('enabled',0x8622),('size',0x8623),('type',0x8625),('stride',0x8624),('buffer',0x889F),('divisor',0x88FE)]:
   _v=C.c_int();_getattr_now(_idx,_pname,C.byref(_v));_state[str(_idx)][_label]=_v.value
 _prog=C.c_int();C.CFUNCTYPE(None,C.c_uint,C.POINTER(C.c_int))(p.gles.gl.glGetIntegerv)(0x8B8D,C.byref(_prog));_link=C.c_int();_nattr=C.c_int();_getprogram(_prog.value,0x8B82,C.byref(_link));_getprogram(_prog.value,0x8B89,C.byref(_nattr))
 query_error=p.gles.f['glGetError']()
 gl_error_checks.append({'call':'draw-state-queries','when':'after','error':query_error})
 if query_error: raise RuntimeError(f'GL error while capturing draw state: {query_error:#x}')
 draw_attribute_state={'attributes':_state,'program':_prog.value,'program_linked':_link.value==1,'active_attribute_count':_nattr.value,'gl_state':p.gl_state()}
 args=[p.x(i) for i in range(4)]
 before=p.gles.f['glGetError']()
 gl_error_checks.append({'call':'glDrawArraysInstanced','when':'before','error':before})
 if before: raise RuntimeError(f'pending GL error before glDrawArraysInstanced: {before:#x}')
 p.gl_calls.append({'call':'glDrawArraysInstanced','values':args,'state':p.gl_state()})
 p.gles.f['glDrawArraysInstanced'](*args)
 _draw_error=p.gles.f['glGetError']()
 gl_error_checks.append({'call':'glDrawArraysInstanced','when':'after','error':_draw_error})
 if _draw_error: raise RuntimeError(f'glDrawArraysInstanced itself returned {_draw_error:#x}')
 return
p.callbacks['glDrawArraysInstanced']=_draw_attr_diag
p.call(f'SPen::Marker2StrokeDrawableRTV{m.selected_drawable_version}::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[rt,bitmap,mat],limit=5_000_000)
# Read the shared framebuffer after each independent renderer draws.
def read_output(label):
 host_gl_checked(label+'Bind',p.gles.f['glBindFramebuffer'],0x8d40,output_fbo)
 host_gl_checked(label+'Finish',p.gles.gl.glFinish)
 data=C.create_string_buffer(256*256*4)
 host_gl_checked(label+'Read',_read_rgba,0,0,256,256,0x1908,0x1401,data)
 return bytes(data.raw)
first=read_output('first')
first_calls=[c for c in p.gl_calls if c['call'] in ('glDrawArrays','glDrawArraysInstanced')]
if len(first_calls)!=2: raise RuntimeError(f'first submission draw count: {len(first_calls)}')
mat2=p.alloc(64); second_matrix=list(caller_matrix_values); second_matrix[12]+=4.; second_matrix[13]+=3.; p.put(mat2,'16f',*second_matrix)
p.call(f'SPen::Marker2StrokeDrawableRTV{m.selected_drawable_version}::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[rt2,bitmap,mat2],limit=5_000_000)
second=read_output('second')
calls=[c for c in p.gl_calls if c['call'] in ('glDrawArrays','glDrawArraysInstanced')]
if len(calls)!=4: raise RuntimeError(f'two submission draw count: {len(calls)}')
first_mask,first_out,second_mask,second_out=[c['state']['framebuffer'] for c in calls]
if first_mask == second_mask or first_out != output_fbo or second_out != output_fbo or first_mask == output_fbo: raise RuntimeError('unexpected draw framebuffer order')
if calls[1]['state']['texture2d'] == calls[3]['state']['texture2d']: raise RuntimeError('composite mask texture unexpectedly shared')
if not any(first) or first==second: raise RuntimeError('draws failed to change the destination')
for label,data in [('first',first),('second',second)]:
 (CLI.output_dir/f'{label}.rgba.zlib').write_bytes(zlib.compress(data,9))
receipt={'scope':'Two independent native ObjectStroke, ObjectDrawing, pen, and RTV2 lifecycles using the same input and overlapping translated transforms into one host Mesa framebuffer.', 'source_runner_sha256':sha256_file(__file__),'input_case':CASE_KEY,'native_vector_sha256':hashlib.sha256(source_vector_data).hexdigest(),'matrices':[caller_matrix_values,second_matrix], 'calls':[{'call':c['call'],'framebuffer':c['state']['framebuffer'],'texture2d':c['state'].get('texture2d')} for c in calls], 'output_fbo':output_fbo,'output_texture':output_texture, 'first_sha256':hashlib.sha256(first).hexdigest(),'second_sha256':hashlib.sha256(second).hexdigest(), 'changed_pixels':sum(first[i:i+4]!=second[i:i+4] for i in range(0,len(first),4)), 'gl_error_checks':gl_error_checks,'nonzero_gl_errors':[c for c in gl_error_checks if c['error']], 'second_native_vector_sha256':hashlib.sha256(second_vector).hexdigest(), 'independent_object_strokes':obj2 != m.obj, 'independent_drawings':drawing2 != p.drawing, 'independent_pens':pen2 != m.selected_pen, 'independent_renderers':rt2 != rt, 'limitations':['GL error checks begin after Drawing admission and Renderer initialization; independent teardown is not exercised.', 'Both lifecycles use identical synthetic source stroke data, shared context and queue, and explicit host callback-vector bridges.','ARM64 guest executes under Unicorn; host Mesa and explicit RT vector bridge; not Android device or complete Samsung page.']}
if receipt['nonzero_gl_errors']: raise RuntimeError('GL errors recorded')
(CLI.output_dir/'receipt.json').write_text(json.dumps(receipt,indent=2,sort_keys=True)+'\n')
print(json.dumps({'changed_pixels':receipt['changed_pixels'],'draws':receipt['calls'],'gl_checks':len(gl_error_checks)}))
raise SystemExit(0)
