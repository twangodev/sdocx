import importlib.util,sys,contextlib,io,hashlib,struct,os,subprocess,re,json,argparse,base64
from pathlib import Path
from unicorn import arm64_const as R
ROOT=Path(__file__).resolve().parents[1]
FIXTURE=ROOT/'conformance/defaultpen-joined-paint-native.json'
parser=argparse.ArgumentParser(description=__doc__)
g=parser.add_mutually_exclusive_group(required=True)
g.add_argument('--write-fixture',action='store_true');g.add_argument('--check-fixture',action='store_true');g.add_argument('--print-capture',action='store_true')
args=parser.parse_args(); os.environ.setdefault('EGL_PLATFORM','surfaceless'); old_argv=sys.argv[:];sys.argv=['defaultpen_saved_caller_native.py','--print-capture']
spec=importlib.util.spec_from_file_location('caller','conformance/defaultpen_saved_caller_native.py');m=importlib.util.module_from_spec(spec)
with contextlib.redirect_stdout(io.StringIO()):spec.loader.exec_module(m)
sys.argv=old_argv
p=m.p; LIB=m.LIBS; base=0x9000000; renderer=LIB/'libSPenRenderer.so'; b=renderer.read_bytes(); assert hashlib.sha256(b).hexdigest()=='f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed'
# Map Renderer at a disjoint emulator base.
ph=struct.unpack_from('<Q',b,32)[0]; ent,n=struct.unpack_from('<HH',b,54); extent=0
for i in range(n):
 t,fl,o,va,pa,fs,ms,al=struct.unpack_from('<II6Q',b,ph+i*ent)
 if t==1:extent=max(extent,va+ms)
p.u.mem_map(base,(extent+0xfff)&~0xfff)
for i in range(n):
 t,fl,o,va,pa,fs,ms,al=struct.unpack_from('<II6Q',b,ph+i*ent)
 if t==1 and ms:p.u.mem_write(base+va,b[o:o+fs]);p.u.mem_write(base+va+fs,bytes(ms-fs) if ms>fs else b'')
# install exports and PLT
env=dict(os.environ);env['LD_LIBRARY_PATH']=str((ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu').resolve())
for line in subprocess.check_output(['nm','-D','-C',str(renderer)],text=True,env=env).splitlines():
 q=line.strip().split(' ',2)
 if len(q)==3 and q[1] in ('T','W'):p.syms[q[2]]=base+int(q[0],16)
dis=subprocess.check_output([str(ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'),'-d','-C','-j','.plt',str(renderer)],text=True,env=env)
for a,name in re.findall(r'^([0-9a-f]+) <(.+)@plt>:',dis,re.M):p.plt[base+int(a,16)]=name
# apply Renderer dynamic relocs with caller's resolved symbols
# dynamic relocs relevant to internal/static and imported symbols
rawsyms={}
for line in subprocess.check_output(['nm','-D',str(renderer)],text=True,env=env).splitlines():
 q=line.split()
 if len(q)==3 and q[1].upper() in ('T','W','D','B','R','V'):rawsyms[q[2]]=base+int(q[0],16)
shoff=struct.unpack_from('<Q',b,40)[0]; se,sn,ss=struct.unpack_from('<HHH',b,58);secs=[struct.unpack_from('<IIQQQQIIQQ',b,shoff+i*se) for i in range(sn)]; names=b[secs[ss][4]:secs[ss][4]+secs[ss][5]];by={names[x[0]:names.find(b'\0',x[0])].decode():x for x in secs};ds,st=by['.dynsym'],by['.dynstr'];strings=b[st[4]:st[4]+st[5]];sy=[]
for off in range(ds[4],ds[4]+ds[5],ds[9]):
 no,inf,oth,ndx,val,size=struct.unpack_from('<IBBHQQ',b,off);sy.append((strings[no:strings.find(b'\0',no)].decode() if no else '',val))
for secname in ('.rela.dyn','.rela.plt'):
 sec=by.get(secname)
 if not sec:continue
 for off in range(sec[4],sec[4]+sec[5],sec[9]):
  where,info,add=struct.unpack_from('<QQq',b,off);typ,idx=info&0xffffffff,info>>32
  if typ==0x403:p.u.mem_write(base+where,struct.pack('<Q',base+add))
  elif typ in (0x401,0x402,0x101,0x1025,0x1026):
   name,val=sy[idx]; target=base+val if val else rawsyms.get(name,0) or p.syms.get(name,0)
   if target:p.u.mem_write(base+where,struct.pack('<Q',target+(add if typ in (0x401,0x101) else 0)))
# Continue on the exact RT selected by the actual Drawing caller.
rt=m.selected_rt
# Install native Renderer exports at distinct base and bounded factory products.
from pathlib import Path
import types
# DSO relocations/init for Renderer handled separately in this staged harness.
source=m.LIBS/'libSPenRenderer.so'
# factory callback implementations
state={'descriptor':None,'graphics_uploads':[],'draws':[],'calls':[]}
def cap_blend():
 src=p.x(0);state['descriptor']=bytes(p.u.mem_read(src,40));out=p.alloc(128);p.x(0,out)
def fake(name,fn):
 at=0x200f3000+len(state['calls'])*16;p.u.mem_write(at,bytes.fromhex('1f2003d5c0035fd6'));state['calls'].append({'name':name,'fn':fn,'address':at});return at
def fake_hook(u,a,z,d):
 for row in state['calls']:
  if row.get('address')==a:
   row['fn']();u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR));return
p.u.hook_add(4,fake_hook)
# callback native synthetic RT interfaces
def depth_create():
 o=p.alloc(16);v=p.alloc(32);p.put(o,'Q',v);p.put(v+24,'Q',fake('depth.activate',lambda:None));p.x(0,o)
def graphics_create():
 o=p.alloc(16);v=p.alloc(64);p.put(o,'Q',v)
 def update():
  idx,ptr,count=p.x(1),p.x(2),p.x(3); raw=bytes(p.u.mem_read(ptr,count*12)) if idx==1 else bytes(p.u.mem_read(ptr,64));state['graphics_uploads'].append({'buffer_index':idx,'byte_count':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),'data':raw.hex()})
 p.put(v+16,'Q',fake('graphics.update',update));p.put(v+56,'Q',fake('graphics.draw',lambda:None));state['graphics']=o;state['graphics_vt']=v;p.x(0,o)
p.callbacks['SPen::RendererFactory::CreateBlendStateObject(SPen::BlendStateDescriptor const&)']=cap_blend
p.callbacks['SPen::RendererFactory::CreateDepthStencilState(SPen::DepthStencilStateDescriptor const&)']=depth_create
p.callbacks['SPen::RendererFactory::CreateGraphicsObject(unsigned int, SPen::PrimitiveType, SPen::VertexDescriptor const*, unsigned int, unsigned int)']=graphics_create
# Preserve actual same RT; capture geometry vector's native object from latest AddPoint callback.
ev=next(x for x in reversed(m.defaultpen_buffer_events) if x['phase']=='draw_object_stroke')
vecobj=int(ev['vector_object'],16); native_stream=bytes.fromhex(ev['after_bytes']); assert len(native_stream)==132
assert hashlib.sha256(native_stream).hexdigest()=='671037a8ed962d44401b86915e7a6361cd185db1491eff9e571c53a5c29e2475'
vbegin,vend,vcap=struct.unpack('<QQQ',p.u.mem_read(vecobj,24)); assert vend-vbegin==132 and bytes(p.u.mem_read(vbegin,132))==native_stream
# Initialize the selected RT native object. Do not allocate another RT or replace its data.
p.put(rt+200,'Q',0);p.put(rt+216,'Q',p.alloc(8));p.put(rt+208,'Q',0)
try:p.call('SPen::DefaultPenStrokeDrawableRT::Init()',x=(rt,),limit=5_000_000)
except Exception as e:print('RT_INIT_FAIL',repr(e),'pc',hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'plt',p.plt.get(p.u.reg_read(R.UC_ARM64_REG_PC)),'syms',[k for k in p.syms if 'RendererFactory' in k][:20]);raise
# Point native Update to the actual vector object captured from this RT's AddPoint callback.
orig_rt80=p.get(rt+80,'Q')[0]; orig_rt88=p.get(rt+88,'Q')[0]
assert orig_rt80==0 and orig_rt88==0, 'unexpected pre-bridge RT vector slots'
p.put(rt+88,'Q',vecobj)
assert p.get(rt+88,'Q')[0]==vecobj
try:p.call('SPen::DefaultPenStrokeDrawableRT::Update(bool)',x=(rt,1),limit=5_000_000)
except Exception as e:print('RT_UPDATE_FAIL',repr(e),'pc',hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'plt',p.plt.get(p.u.reg_read(R.UC_ARM64_REG_PC)),'entries',state['calls']);raise
# Native BlendStateObject and RT::Draw call with explicitly bounded shader/bitmap adapters.
descptr=p.alloc(40);p.u.mem_write(descptr,state['descriptor']);blendobj=p.alloc(128);p.call('SPen::GLES::BlendStateObject::BlendStateObject(SPen::BlendStateDescriptor const&)',x=(blendobj,descptr));renderer_vt=base+0x6a078;p.put(blendobj,'Q',renderer_vt);p.put(renderer_vt+24,'Q',p.syms['SPen::GLES::BlendStateObject::Activate()']);p.put(rt+208,'Q',blendobj)
state['gl_calls']=[]
def glcb(name,n):
 def fn():state['gl_calls'].append([name,*[p.x(i) for i in range(n)]])
 return fn
for name,n in [('glBlendEquationSeparate',2),('glBlendFuncSeparate',4),('glEnable',1),('glDisable',1),('glColorMask',4)]:p.callbacks[name]=glcb(name,n)
# shader uniform facades
shader={}
proj=p.alloc(8);pvt=p.alloc(8);p.put(proj,'Q',pvt);p.put(pvt,'Q',fake('uniform.projection',lambda:state['calls'].append({'name':'uniform.projection.args','args':[hex(p.x(i)) for i in (0,1,8)]})))
col=p.alloc(8);cvt=p.alloc(8);p.put(col,'Q',cvt);p.put(cvt,'Q',fake('uniform.color',lambda:state['calls'].append({'name':'uniform.color.args','args':[hex(p.x(i)) for i in (0,1,8)]})))
prog=p.alloc(8);progvt=p.alloc(24);p.put(prog,'Q',progvt);p.put(progvt+16,'Q',fake('shader.predraw',lambda:state['calls'].append({'name':'shader.predraw'})))
wrapper=p.alloc(24);p.put(wrapper,'QQQ',proj,col,prog);p.put(rt+216,'Q',wrapper)
bitmap=p.alloc(8);bvt=p.alloc(64);p.put(bitmap,'Q',bvt)
def bt(): p.put(p.x(8),'16f',1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1)
p.put(bvt+16,'Q',fake('bitmap.transform',bt));p.put(bvt+40,'Q',fake('bitmap.activate',lambda:state['calls'].append({'name':'bitmap.activate'})));p.put(bvt+48,'Q',fake('bitmap.deactivate',lambda:state['calls'].append({'name':'bitmap.deactivate'})))
# The host-backed Draw below is the only Draw execution retained in the output.
# Run the same native RT::Update and RT::Draw through an explicit host EGL/GLES context.
import types
h=types.ModuleType('host_gl')
exec('"""Explicit synthetic host EGL/GLES bindings."""\nimport ctypes as C\n# Import performs bounded native saved redraw and joined native RT Update/Draw capture.\nfrom defaultpen_draw_native import embedded_shader\n\ndef decl(lib,name,ret,args):\n f=getattr(lib,name);f.restype=ret;f.argtypes=args;return f\nE=C.CDLL(\'libEGL.so.1\'); G=C.CDLL(\'libGLESv2.so\')\nP=C.c_void_p; I=C.c_int; U=C.c_uint; F=C.c_float; S=C.c_char_p\ngetd=decl(E,\'eglGetDisplay\',P,[P]); init=decl(E,\'eglInitialize\',U,[P,C.POINTER(I),C.POINTER(I)]);choose=decl(E,\'eglChooseConfig\',U,[P,C.POINTER(I),C.POINTER(P),I,C.POINTER(I)]);bindapi=decl(E,\'eglBindAPI\',U,[U]);ctxfn=decl(E,\'eglCreateContext\',P,[P,P,P,C.POINTER(I)]);surf_fn=decl(E,\'eglCreatePbufferSurface\',P,[P,P,C.POINTER(I)]);current=decl(E,\'eglMakeCurrent\',U,[P,P,P,P])\nW,H=128,64\ndisplay=getd(P(0));major=I();minor=I()\nif not init(display,C.byref(major),C.byref(minor)):raise RuntimeError(\'EGL init failed\')\nattrs=(I*15)(0x3033,1,0x3040,0x40,0x3024,8,0x3023,8,0x3022,8,0x3021,8,0x3038);config=P();n=I()\nif not choose(display,attrs,C.byref(config),1,C.byref(n)) or n.value!=1:raise RuntimeError(\'no EGL config\')\nif not bindapi(0x30A0):raise RuntimeError(\'GLES bind failed\')\nca=(I*3)(0x3098,3,0x3038);ctx=ctxfn(display,config,P(0),ca);sa=(I*5)(0x3057,W,0x3056,H,0x3038);surf=surf_fn(display,config,sa)\nif not ctx or not surf or not current(display,surf,surf,ctx):raise RuntimeError(\'EGL context failed\')\ndef gl(n,r,a):return decl(G,n,r,a)\ngetstr=gl(\'glGetString\',P,[U]);getin=gl(\'glGetIntegerv\',None,[U,C.POINTER(I)]);getattachment=gl(\'glGetFramebufferAttachmentParameteriv\',None,[U,U,U,C.POINTER(I)]);create_shader=gl(\'glCreateShader\',U,[U]);shader_src=gl(\'glShaderSource\',None,[U,I,C.POINTER(S),C.POINTER(I)]);compile=gl(\'glCompileShader\',None,[U]);getsi=gl(\'glGetShaderiv\',None,[U,U,C.POINTER(I)]);slog=gl(\'glGetShaderInfoLog\',None,[U,I,C.POINTER(I),S]);create_prog=gl(\'glCreateProgram\',U,[]);attach=gl(\'glAttachShader\',None,[U,U]);link=gl(\'glLinkProgram\',None,[U]);getpi=gl(\'glGetProgramiv\',None,[U,U,C.POINTER(I)]);plog=gl(\'glGetProgramInfoLog\',None,[U,I,C.POINTER(I),S]);use=gl(\'glUseProgram\',None,[U]);loc=gl(\'glGetUniformLocation\',I,[U,S]);um4=gl(\'glUniformMatrix4fv\',None,[I,I,U,C.POINTER(F)]);u4fv=gl(\'glUniform4fv\',None,[I,I,C.POINTER(F)])\ngenbuf=gl(\'glGenBuffers\',None,[I,C.POINTER(U)]);bindbuf=gl(\'glBindBuffer\',None,[U,U]);bufdata=gl(\'glBufferData\',None,[U,C.c_ssize_t,P,U]);bufsub=gl(\'glBufferSubData\',None,[U,C.c_ssize_t,C.c_ssize_t,P]);enable=gl(\'glEnableVertexAttribArray\',None,[U]);attrib=gl(\'glVertexAttribPointer\',None,[U,I,U,U,I,P]);divisor=gl(\'glVertexAttribDivisor\',None,[U,U]);gentex=gl(\'glGenTextures\',None,[I,C.POINTER(U)]);bindtex=gl(\'glBindTexture\',None,[U,U]);texparam=gl(\'glTexParameteri\',None,[U,U,I]);teximage=gl(\'glTexImage2D\',None,[U,I,I,I,I,I,U,U,P]);genfb=gl(\'glGenFramebuffers\',None,[I,C.POINTER(U)]);bindfb=gl(\'glBindFramebuffer\',None,[U,U]);attachtex=gl(\'glFramebufferTexture2D\',None,[U,U,U,U,I]);checkfb=gl(\'glCheckFramebufferStatus\',U,[U]);viewport=gl(\'glViewport\',None,[I,I,I,I]);clearc=gl(\'glClearColor\',None,[F,F,F,F]);clear=gl(\'glClear\',None,[U]);beq=gl(\'glBlendEquationSeparate\',None,[U,U]);bf=gl(\'glBlendFuncSeparate\',None,[U,U,U,U]);enablegl=gl(\'glEnable\',None,[U]);colormask=gl(\'glColorMask\',None,[U,U,U,U]);draw=gl(\'glDrawArraysInstanced\',None,[U,I,I,I]);read=gl(\'glReadPixels\',None,[I,I,I,I,U,U,P]);finish=gl(\'glFinish\',None,[]);geterr=gl(\'glGetError\',U,[])\ndef shader(kind,src):\n sh=create_shader(kind);p=C.c_char_p(src.encode());shader_src(sh,1,C.byref(p),None);compile(sh);ok=I();getsi(sh,0x8B81,C.byref(ok))\n if not ok.value:\n  b=C.create_string_buffer(2048);l=I();slog(sh,2048,C.byref(l),b);raise RuntimeError(b.value.decode())\n return sh\n',h.__dict__,h.__dict__)
vs_src,fs_src=h.embedded_shader();vs=h.shader(0x8B31,vs_src);fs=h.shader(0x8B30,fs_src);program=h.create_prog();h.attach(program,vs);h.attach(program,fs);h.link(program);ok=h.I();h.getpi(program,0x8B82,h.C.byref(ok))
if not ok.value: raise RuntimeError('DefaultPen shader program failed to link')
h.use(program);locmat=h.loc(program,b'uProjectionMatrix');loccolor=h.loc(program,b'inputColor')
# Bind native quad and the exact native 132-byte instance stream to host VBOs.
quad=bytes.fromhex(state['graphics_uploads'][0]['data']);instances=bytes.fromhex(state['graphics_uploads'][1]['data']);assert len(quad)==64 and len(instances)==132
bufs=(h.U*2)();h.genbuf(2,bufs)
h.bindbuf(0x8892,bufs[0]);qb=h.C.create_string_buffer(quad);h.bufdata(0x8892,len(quad),qb,0x88E4);h.enable(0);h.attrib(0,4,0x1406,0,16,h.P(0));h.divisor(0,0)
h.bindbuf(0x8892,bufs[1]);ib=h.C.create_string_buffer(instances);h.bufdata(0x8892,len(instances),ib,0x88E4);h.enable(1);h.attrib(1,3,0x1406,0,12,h.P(0));h.divisor(1,1)
tex=(h.U*1)();h.gentex(1,tex);h.bindtex(0x0DE1,tex[0]);h.teximage(0x0DE1,0,0x8058,h.W,h.H,0,0x1908,0x1401,None)
fb=(h.U*1)();h.genfb(1,fb);h.bindfb(0x8D40,fb[0]);h.attachtex(0x8D40,0x8CE0,0x0DE1,tex[0],0);status=h.checkfb(0x8D40)
if status!=0x8CD5:raise RuntimeError(f'incomplete RGBA8 FBO {status:#x}')
h.viewport(0,0,h.W,h.H)
frame_state={}; frame_state['draw_fbo']=h.I();frame_state['read_fbo']=h.I();frame_state['viewport']=(h.I*4)();frame_state['attachment_type']=h.I();frame_state['attachment_name']=h.I()
h.getin(0x8CA6,h.C.byref(frame_state['draw_fbo']));h.getin(0x8CAA,h.C.byref(frame_state['read_fbo']));h.getin(0x0BA2,frame_state['viewport'])
h.getattachment(0x8D40,0x8CE0,0x8CD0,h.C.byref(frame_state['attachment_type']));h.getattachment(0x8D40,0x8CE0,0x8CD1,h.C.byref(frame_state['attachment_name']))
assert frame_state['draw_fbo'].value==fb[0] and frame_state['read_fbo'].value==fb[0]
assert list(frame_state['viewport'])==[0,0,h.W,h.H]
assert frame_state['attachment_type'].value==0x1702 and frame_state['attachment_name'].value==tex[0]
assert bufs[0]!=bufs[1] and fb[0]!=0 and tex[0]!=0
h.clearc(0,0,0,0);h.clear(0x4000)
# Replace host boundary callbacks with actual GLES calls, while RT::Draw and blend activation stay native.
for row in state['calls']:
 if row.get('name')=='graphics.update':
  def gl_update():
   idx,ptr,count=p.x(1),p.x(2),p.x(3);raw=bytes(p.u.mem_read(ptr,64 if idx==0 else count*12));state['host_gl_uploads'].append({'buffer_index':idx,'byte_count':len(raw),'sha256':hashlib.sha256(raw).hexdigest()});h.bindbuf(0x8892,bufs[idx]);payload=p.u.mem_read(ptr,len(raw));temp=h.C.create_string_buffer(bytes(payload));h.bufsub(0x8892,0,len(raw),temp)
  row['fn']=gl_update
 elif row.get('name')=='graphics.draw':
  def gl_draw():
   args=[p.x(i) for i in range(6)];state['draws'].append(args);h.draw(5,0,4,args[3])
  row['fn']=gl_draw
 elif row.get('name')=='shader.predraw':row['fn']=lambda:h.use(program)
 elif row.get('name')=='uniform.projection':
  def set_matrix():
   ptr=p.x(1);raw=bytes(p.u.mem_read(ptr,64));vals=struct.unpack('<16f',raw);state.setdefault('uniform_matrices',[]).append([v.hex() for v in vals]);h.um4(locmat,1,0,(h.F*16)(*vals))
  row['fn']=set_matrix
 elif row.get('name')=='uniform.color':
  def set_color():
   ptr=p.x(1);vals=struct.unpack('<4f',p.u.mem_read(ptr,16));state.setdefault('uniform_colors',[]).append([v.hex() for v in vals]);h.u4fv(loccolor,1,(h.F*4)(*vals))
  row['fn']=set_color
state['host_gl_uploads']=[]
for name,fn,nargs in [('glBlendEquationSeparate',h.beq,2),('glBlendFuncSeparate',h.bf,4),('glEnable',h.enablegl,1),('glDisable',h.gl('glDisable',None,[h.U]),1),('glColorMask',h.colormask,4)]:
 def make(name,fn,nargs):
  def cb():
   values=[p.x(i) for i in range(nargs)];state['gl_calls'].append([name,*values]);fn(*values)
  return cb
 p.callbacks[name]=make(name,fn,nargs)
# Native Update dispatches the same RT's native CPU stream through the host VBO update boundary.
p.call('SPen::DefaultPenStrokeDrawableRT::Update(bool)',x=(rt,1),limit=5_000_000)
# Give native Draw an explicit host projection; the shader uniform callback consumes its native matrix pointer.
mat2=p.alloc(64);projection=(2/h.W,0,0,0,0,2/h.H,0,0,0,0,1,0,-1,-1,0,1);p.put(mat2,'16f',*projection)
try:p.call('SPen::DefaultPenStrokeDrawableRT::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',x=(rt,bitmap,mat2),limit=5_000_000)
except Exception as e:print('HOST_GL_DRAW_FAIL',repr(e),'pc',hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'plt',p.plt.get(p.u.reg_read(R.UC_ARM64_REG_PC)));raise
h.finish();pixels=h.C.create_string_buffer(h.W*h.H*4);h.read(0,0,h.W,h.H,0x1908,0x1401,pixels);image=pixels.raw
post_state={'draw_fbo':h.I(),'read_fbo':h.I(),'viewport':(h.I*4)()};h.getin(0x8CA6,h.C.byref(post_state['draw_fbo']));h.getin(0x8CAA,h.C.byref(post_state['read_fbo']));h.getin(0x0BA2,post_state['viewport'])
assert post_state['draw_fbo'].value==fb[0] and post_state['read_fbo'].value==fb[0] and list(post_state['viewport'])==[0,0,h.W,h.H]
nonzero=[(i%h.W,i//h.W) for i in range(h.W*h.H) if image[i*4:i*4+4]!=bytes(4)]
report={'framebuffer':{'kind':'host RGBA8 texture-backed FBO','width':h.W,'height':h.H,'status':hex(status),'viewport':[0,0,h.W,h.H],'read_format':'GL_RGBA/GL_UNSIGNED_BYTE'},'host_renderer':h.C.string_at(h.getstr(0x1F01)).decode(),'host_gles_version':h.C.string_at(h.getstr(0x1F02)).decode(),'same_native_rt':True,'actual_drawing_rt_address':hex(rt),'native_initialization_uploads':state['graphics_uploads'],'host_gl_uploads_from_native_update':state['host_gl_uploads'],'instance_count':p.get(rt+72,'I')[0],'native_drawing_phase_stream_sha256':hashlib.sha256(bytes.fromhex(ev['after_bytes'])).hexdigest(),'native_stream_sha256':hashlib.sha256(instances).hexdigest(),'native_quad_sha256':hashlib.sha256(quad).hexdigest(),'shader':{'vertex_source':vs_src,'fragment_source':fs_src,'vertex_sha256':hashlib.sha256(vs_src.encode()).hexdigest(),'fragment_sha256':hashlib.sha256(fs_src.encode()).hexdigest(),'uniform_locations':{'uProjectionMatrix':locmat,'inputColor':loccolor}},'explicit_draw_matrix_f32':[float(x) for x in projection],'uniform_matrix_values':state.get('uniform_matrices'),'uniform_color_values':state.get('uniform_colors'),'blend_calls':state['gl_calls'],'native_draw_args':state['draws'],'draw_register_values_meaning':'x0 synthetic graphics interface; x1 first vertex; x2 vertex count; x3 instance count; x4/x5 native trailing ABI values captured but not mapped to host GLES.','readback_sha256':hashlib.sha256(image).hexdigest(),'nonzero_pixels':len(nonzero),'bbox':[min((x for x,y in nonzero),default=-1),min((y for x,y in nonzero),default=-1),max((x for x,y in nonzero),default=-1),max((y for x,y in nonzero),default=-1)],'gl_error':h.geterr(),'boundary':'Native DefaultPen ObjectDrawing/Redraw, same RT::Init/Update/Draw, Renderer BlendStateObject::Activate, and shader uniform call sites execute in Unicorn. Host supplies EGL/GLES context/FBO, native renderer factory products (graphics/depth/shader bindings), VBO attributes, bitmap identity transform, projection, and GLES entrypoints. No Samsung device GPU/runtime claim.'}
assert h.geterr()==0, 'host GL reported an error after draw/readback'
assert len(image)==128*64*4 and hashlib.sha256(image).hexdigest()=='78f845598c0f76be4e7cc98e25f82885bea13e4a9fedc23b92d163e09c58fab8'
report.update({'case_id':'opaque_default','native_route':{'selected_class':'SPen::DefaultPenStrokeDrawableGL','same_rt_pointer':hex(rt),'drawing_phase_vector_object_pointer':hex(vecobj),'rt_plus80_before_bridge':hex(orig_rt80),'rt_plus88_before_bridge':hex(orig_rt88),'rt_update_input_slot_offset':88,'host_assigned_pointer':hex(vecobj),'same_vector_object_identity':True,'stream_length':len(native_stream),'stream_sha256':hashlib.sha256(native_stream).hexdigest(),'wrapper_clears_rt_plus80_after_return':p.get(rt+80,'Q')[0]==0,'SendDataToGPU_observed_in_Drawing_route':False,'transport_scope':'Drawing route records native AddPoint vector; wrapper clears RT+80. Host explicitly assigns that same vector object to RT+88 before native Update. This is a bounded same-object join, not evidence that Drawing invoked SendDataToGPU or queued a GPU update.'},'rt_init_descriptor_hex':state['descriptor'].hex(),'native_draw_rva':hex(p.syms['SPen::DefaultPenStrokeDrawableRT::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)']-0x5000000),'draw_native_abi_registers':state['draws'],'host_draw_mapping':{'gl_mode':'GL_TRIANGLE_STRIP','mode_value':5,'first':0,'vertex_count':4,'instances_register_x3':p.get(rt+72,'I')[0]},'host_objects':{'framebuffer_id':int(fb[0]),'attachment_texture_id':int(tex[0]),'draw_framebuffer_binding_before_draw':frame_state['draw_fbo'].value,'read_framebuffer_binding_before_draw':frame_state['read_fbo'].value,'viewport_before_draw':list(frame_state['viewport']),'attachment_object_type_before_draw':hex(frame_state['attachment_type'].value),'attachment_object_name_before_draw':frame_state['attachment_name'].value,'draw_framebuffer_binding_after_readback':post_state['draw_fbo'].value,'read_framebuffer_binding_after_readback':post_state['read_fbo'].value,'viewport_after_readback':list(post_state['viewport']),'vbo_ids':[int(bufs[i]) for i in range(2)],'vertex_attributes':[{'location':0,'components':4,'stride':16,'divisor':0,'source':'native 64-byte quad'},{'location':1,'components':3,'stride':12,'divisor':1,'source':'native 132-byte instance stream'}]},'readback_length':len(image),'rgba_base64':base64.b64encode(image).decode()})
caller_case=m.result
fixture={'schema':'defaultpen-joined-paint-native-fixture-v1','sources':{'caller_loaded_dsos':m.native_hashes,'caller_profile':caller_case['profile'],'caller_input':caller_case['input'],'caller_route':caller_case['route'],'caller_sha256':hashlib.sha256((ROOT/'conformance/defaultpen_saved_caller_native.py').read_bytes()).hexdigest(),'shader_source_helper_sha256':hashlib.sha256((ROOT/'conformance/defaultpen_draw_native.py').read_bytes()).hexdigest(),'renderer_sha256':hashlib.sha256(b).hexdigest()},'cases':{'opaque_default':report}}
serialized=json.dumps(fixture,indent=2,sort_keys=True)+'\n'
if args.write_fixture: FIXTURE.write_text(serialized); print('wrote',FIXTURE)
elif args.check_fixture:
 actual=json.loads(FIXTURE.read_text())
 if actual!=fixture: raise SystemExit('fixture mismatch: '+str(FIXTURE))
 print('fixture matches',FIXTURE)
else: print(serialized,end='')
