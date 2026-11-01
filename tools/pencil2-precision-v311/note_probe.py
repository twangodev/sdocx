"""Compare grain selection against the fresh v309 white note-derived capture."""
import contextlib, hashlib, io, json, pathlib, struct, sys, tempfile, types, zlib
root = pathlib.Path(__file__).resolve().parents[2]
frames = pathlib.Path(sys.argv[1]); receipt = json.loads(pathlib.Path(sys.argv[2]).read_text())
assert receipt['first_sha256'] == '6c8e9466ca9e473812a5d5cf87b448a359b3008d9861c2c404c223c536bba642'
assert receipt['second_sha256'] == '1576d32a0d7592c3cc82da4ac60791ba5fd59e44f8806239488eba69924237a5'
assert receipt['geometry_sha256'] == '7d88c4f7e2f8fa148de58e515194d59736ceda859abd6297a0de7b330cc987cf'
assert [v[3] for v in receipt['uniforms'] if v[:3] == ['glUniform2fv',4,1]][-1] == [170.24111938476562,145.2706298828125]
script = root/'tools/pencil2-mesh-v308/pixel_probe.py'
s = script.read_text(); assert hashlib.sha256(s.encode()).hexdigest() == '5adebdb89b9f1e1954aee7e60bd2413a89c968bcf19591ce8b83e1a136c5d90c'
s = s.replace('gx,gy=12,(13.999998 if scale==1 else 16.0000019)', 'gx,gy=170.24111938476562,145.2706298828125').replace("for backdrop,scale in [('white',1),('white',2),('blue',2)]:go(scale,backdrop)", "go(1,'white')")
def f32(v): return struct.unpack('<f',struct.pack('<f',v))[0]
def normalized_index(pixel,offset,scale):
 import math
 q=f32(f32(f32(pixel)-f32(offset))/f32(75*scale)); q=f32(q-math.floor(q))
 return math.floor(f32(q*150))%150
with tempfile.TemporaryDirectory(prefix='pencil-v311-') as d:
 base=pathlib.Path(d)
 for prefix in ('first','second'):
  raw=(frames/f'{prefix}.rgba').read_bytes(); assert hashlib.sha256(raw).hexdigest()==receipt[prefix+'_sha256']
  (base/f'white-1-{prefix}.zlib').write_bytes(zlib.compress(raw))
 (base/'white-1-receipt.json').write_text(json.dumps(receipt))
 native=(frames/'second.vbo').read_bytes(); assert hashlib.sha256(native).hexdigest()==receipt['geometry_sha256']
 scene=types.ModuleType('scene');scene.BASE=base
 scene.GRAIN=(root/'crates/sdocx/src/ink/pencil2/assets/grain.r8').read_bytes();scene.POINT=(root/'crates/sdocx/src/ink/pencil2/assets/point.r8').read_bytes()
 assert hashlib.sha256(scene.GRAIN).hexdigest()=='dbe4f006debde4b765c1e5ee6966798dd8b14cdc939b149c9e22770a2c0f97f9'
 assert hashlib.sha256(scene.POINT).hexdigest()=='475d71a47445657c58c944fb14a4746a248b57d33aff7bff864f30dc72d9ad6f'
 scene.records=list(struct.iter_unpack('<14f',native));sys.modules['scene']=scene
 for mode in ('ideal','f32'):
  code=s
  if mode=='f32':
   code=code.replace('math.floor((xx-gx)/(scale*.5))%150','normalized_index(xx,gx,scale)').replace('math.floor((yy-gy)/(scale*.5))%150','normalized_index(yy,gy,scale)')
  output=io.StringIO()
  with contextlib.redirect_stdout(output):exec(compile(code,str(script),'exec'),{'normalized_index':normalized_index})
  assert output.getvalue().splitlines() == ['white-1 center sample approximate float: 76 7 41'];print(mode,output.getvalue(),end='')
