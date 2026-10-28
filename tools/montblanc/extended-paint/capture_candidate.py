#!/usr/bin/env python3
"""Controlled Montblanc RTV1 native API paint experiment on host Mesa.

This composes the pinned V2 Drawing-produced Add* vectors with the exported
RTV1 Init/SetBuffer/SendDataToGPU/Update/Draw APIs. The SetBuffer and
SendDataToGPU calls are explicit test sequencing: the V2 Drawing capture did
not naturally invoke those methods. They receive the exact Add-owned vector
objects; this script never writes RTV1 member slots directly.
"""
from __future__ import annotations

import argparse, ast, base64, hashlib, json, os, runpy, struct, subprocess, sys, zlib
import ctypes as C
from pathlib import Path
from uc_region_coverage import region_union_covers, region_union_covers_all
from gl_object_ledger import generated_ids_since, rgba8_readback_sha, target_gl_evidence, wrap_id_generator
from native_runtime_api import common_return_stop
from native_call_plan import build_native_call_plan
from host_gles_api import bind_host_gles, prepare_target, finish_target_readback
from draw_observability import (bind_draw_query_api, snapshot_draw_attributes, annotate_buffer_upload, compact_shader_sources, compact_uniform_trace, join_draw_provenance, rgba8_statistics, current_array_buffer, require_native_vector_draw_joins)
from shader_source_observer import capture_gl_shader_source
from runtime_interface_audit import audit_runtime_interface

ROOT=Path(__file__).resolve().parents[3]
V2=ROOT/'tmp/vector-parity-plan/montblanc-rtv1-mesh-capture-root-v2/montblanc_state_controls.py'
CAPTURE_DIR=V2.parent/'captures'
LIB=ROOT/'scratch/apk-analysis-native/arm64-v8a'
CONFORMANCE=V2.parent/'conformance'
MARKER3=CONFORMANCE/'marker3_paint_scale_native.py'
EXPECTED={
 'runner': 'a8647b96fa001dbe387de8c231ed4895d2f7b5ef619240835c17867366f8c0c8',
 'graphics': 'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621',
 'renderer': 'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed',
 'base': 'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb',
 'pencommon': 'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d',
 'montblanc': '4c808cf7a1c3685e6c99e654bb8132e3ad95db4d28fef931f6936bed3d5b8350',
 'drawing': '788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd',
 'libcxx': '4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4',
 'marker3_adapter': '9b1eafd3dd07d1b491148d2635aad004c83ac401416f3c8a9c62b1e60099b7a8',
}
HOST_GL_EXPECTED={'libEGL.so.1':'f4b60a4618fbf760f395f64dfd53320446ba1011151a80e411742b3232b194e6',
                  'libGLESv2.so.2':'a705c66de30ca4e883120c70acae0f1011d7c3f97605eef331e03493beae3812'}
SYMBOLS={
 'init':'SPen::MontblancFountainPenStrokeDrawableRTV1::Init()',
 'create_canvas':'SPen::MontblancFountainPenStrokeDrawableRTV1::CreatePenCanvas(int, int, SPen::IGLMsgQueue*)',
 'set_buffer':'SPen::MontblancFountainPenStrokeDrawableRTV1::SetBuffer(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)',
 'send':'SPen::MontblancFountainPenStrokeDrawableRTV1::SendDataToGPU(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)',
 'update':'SPen::MontblancFountainPenStrokeDrawableRTV1::Update(bool)',
 'draw':'SPen::MontblancFountainPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',
 'set_pen_data':'SPen::PenDrawableRT::SetPenData(float, int)',
 'subbitmap_ctor':'SPen::SPSubBitmapRT::SPSubBitmapRT(SPen::IGLMsgQueue*, int, SPen::Rect const&, int, int, char const*, void const*, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)',
 'subbitmap_create_framebuffer':'SPen::SPSubBitmapRT::CreateFrameBuffer()',
}
PARSER=argparse.ArgumentParser()
PARSER.add_argument('--case',choices=('baseline','pressure','size','far_endpoint'),default='baseline')
PARSER.add_argument('--output',type=Path,required=True)
PARSER.add_argument('--width',type=int,default=256); PARSER.add_argument('--height',type=int,default=256)
PARSER.add_argument('--argb',type=lambda s:int(s,0),default=0xff336699)
PARSER.add_argument('--translation',type=float,nargs=2,default=(128.0,128.0))
ARGS=PARSER.parse_args()
if (ARGS.width,ARGS.height)!=(256,256): raise SystemExit('candidate fixes the controlled RGBA8 target at 256x256')
if ARGS.argb<0 or ARGS.argb>0xffffffff: raise SystemExit('ARGB must fit u32')

def digest(path:Path)->str: return hashlib.sha256(path.read_bytes()).hexdigest()
REGION_HELPER=Path(__file__).resolve().with_name('uc_region_coverage.py')
REGION_HELPER_SHA256='33904108846b1812ba06555265a08df8658b2e9352a5279a3303b34e8f7f5ce3'
if Path(__import__('uc_region_coverage').__file__).resolve()!=REGION_HELPER or digest(REGION_HELPER)!=REGION_HELPER_SHA256:
 raise RuntimeError('UC region coverage helper identity/hash mismatch')
GL_LEDGER_HELPER=Path(__file__).resolve().with_name('gl_object_ledger.py')
GL_LEDGER_HELPER_SHA256='e573df810448cfc0212fbf20411d5cba439fd6a4f3f410083b8356ba5ebffb08'
if Path(__import__('gl_object_ledger').__file__).resolve()!=GL_LEDGER_HELPER or digest(GL_LEDGER_HELPER)!=GL_LEDGER_HELPER_SHA256:
 raise RuntimeError('GL object ledger helper identity/hash mismatch')
NATIVE_RUNTIME_API=Path(__file__).resolve().with_name('native_runtime_api.py')
NATIVE_RUNTIME_API_SHA256='845e06680d9e5bd58664e92de16ece8f45153e08a8365a7d57a7eace74b4cf01'
NATIVE_CALL_PLAN=Path(__file__).resolve().with_name('native_call_plan.py')
NATIVE_CALL_PLAN_SHA256='9f686be8aa412045da73d318bd2a2001a5ca745e75ca75a01c9571cf6e8735a4'
HOST_GLES_API=Path(__file__).resolve().with_name('host_gles_api.py')
HOST_GLES_API_SHA256='265a6bba846eec2b7f220da6f66d7d4113710c1f9f56c5b28d52ade4da5a96bf'
DRAW_OBSERVABILITY=Path(__file__).resolve().with_name('draw_observability.py')
DRAW_OBSERVABILITY_SHA256='149c5db124be29d1601690fd9124c49c9c28beb725cdb4c2ee23b5530c3e7afb'
SHADER_SOURCE_OBSERVER=Path(__file__).resolve().with_name('shader_source_observer.py')
SHADER_SOURCE_OBSERVER_SHA256='2dbab39745ea0471a99b9a2c056d40bfaa4c5dda88a64ec8f212a87aa8ad6732'
RUNTIME_INTERFACE_AUDIT=Path(__file__).resolve().with_name('runtime_interface_audit.py')
RUNTIME_INTERFACE_AUDIT_SHA256='eb1c4d8d4cfce0eceec29c760db3cc3e03a0ff29aecce3236355702d88d7670a'
if Path(__import__('native_runtime_api').__file__).resolve()!=NATIVE_RUNTIME_API or digest(NATIVE_RUNTIME_API)!=NATIVE_RUNTIME_API_SHA256:
 raise RuntimeError('native runtime API helper identity/hash mismatch')
if Path(__import__('native_call_plan').__file__).resolve()!=NATIVE_CALL_PLAN or digest(NATIVE_CALL_PLAN)!=NATIVE_CALL_PLAN_SHA256:
 raise RuntimeError('native call plan helper identity/hash mismatch')
if Path(__import__('host_gles_api').__file__).resolve()!=HOST_GLES_API or digest(HOST_GLES_API)!=HOST_GLES_API_SHA256:
 raise RuntimeError('host GLES API helper identity/hash mismatch')
if Path(__import__('draw_observability').__file__).resolve()!=DRAW_OBSERVABILITY or digest(DRAW_OBSERVABILITY)!=DRAW_OBSERVABILITY_SHA256:
 raise RuntimeError('draw observability helper identity/hash mismatch')
if Path(__import__('shader_source_observer').__file__).resolve()!=SHADER_SOURCE_OBSERVER or digest(SHADER_SOURCE_OBSERVER)!=SHADER_SOURCE_OBSERVER_SHA256:
 raise RuntimeError('shader source observer helper identity/hash mismatch')
if Path(__import__('runtime_interface_audit').__file__).resolve()!=RUNTIME_INTERFACE_AUDIT or digest(RUNTIME_INTERFACE_AUDIT)!=RUNTIME_INTERFACE_AUDIT_SHA256:
 raise RuntimeError('runtime interface audit helper identity/hash mismatch')
for label,path in [('runner',V2),('graphics',LIB/'libSPenGraphics.so'),('renderer',LIB/'libSPenRenderer.so'),
                   ('base',LIB/'libSPenBase.so'),('pencommon',LIB/'libSPenPenCommon.so'),
                   ('montblanc',LIB/'libSPenMontblancFountainPen.so'),('drawing',LIB/'libSPenDrawing.so'),
                   ('libcxx',LIB/'libc++_shared.so'),('marker3_adapter',MARKER3)]:
 if digest(path)!=EXPECTED[label]: raise RuntimeError(f'{label} input pin mismatch: {digest(path)}')
runtime_interface=audit_runtime_interface(Path(__file__),V2,CONFORMANCE/'fountain_native.py',CONFORMANCE/'oblique_canvas_native.py',MARKER3,CONFORMANCE/'airbrush_texture_native.py',CONFORMANCE/'airbrush_saved_native.py')
HOST_GL_PATHS={name:str((Path('/lib/x86_64-linux-gnu')/name).resolve()) for name in HOST_GL_EXPECTED}
HOST_GL_HASHES={name:digest(Path(path)) for name,path in HOST_GL_PATHS.items()}
if HOST_GL_HASHES!=HOST_GL_EXPECTED: raise RuntimeError(f'host EGL/GLES input pin mismatch: {HOST_GL_HASHES!r}')

# Re-run the exact saved-stroke Drawing input in this process and retain its
# selected live RT receiver and native std::vector objects.
sys.path[:0]=[str(ROOT/'scratch/apk-analysis-runtime/python'),str(CONFORMANCE)]
old_argv=sys.argv[:]
sys.argv=[str(V2),'--case',ARGS.case,'--check-fixture']
try: ns=runpy.run_path(str(V2),run_name='__main__')
finally: sys.argv=old_argv
p=ns['p']; runtime=ns['rt']; R=ns['R']; queue=ns['queue']; qvt=ns['qvt']; record=ns['records'][0]; RUNTIME_INPUTS=ns['RUNTIME_INPUTS']
native_runtime_stop=common_return_stop(runtime)
at,ip,fn=ns['at'],ns['ip'],ns['fn']
receiver=int(record['actual_montblanc_rtv1_addpoint_records'][-1]['receiver'],16)
if any(int(row['receiver'],16)!=receiver for row in record['actual_montblanc_rtv1_addpoint_records']): raise RuntimeError('Add* receiver changed within source Drawing generation')
if record['rtv1_final_vectors']['add_vectors']['this']!=hex(receiver): raise RuntimeError('final Add vectors do not belong to selected RT receiver')
vector_rows=record['rtv1_final_vectors']['add_vectors']['vectors']['fields']
vector_ptrs=[int(row['vector_object_ptr'],16) for row in vector_rows]
vector_hashes=[row['data_sha256'] for row in vector_rows]
if len(vector_ptrs)!=3 or any(not x for x in vector_ptrs): raise RuntimeError('native Add vector object missing')

# Reuse the exact Renderer-aware adapter and image already installed by the
# parent V2 runtime. That runtime pinned the isolated DSO before loading it;
# resetting its map metadata or mapping another copy would overlap its UC.
adapter=ns['_adapter']; renderer_base=ns['_renderer_base']
if adapter.u is not p.u: raise RuntimeError('parent Renderer adapter does not use the active Drawing UC')
if adapter.extra_bases.get('Renderer')!=renderer_base: raise RuntimeError('parent Renderer mapping metadata is missing or inconsistent')
if renderer_base!=0xa000000: raise RuntimeError(f'unexpected pinned Renderer base: {renderer_base:#x}')
for copy_path,label in [(RUNTIME_INPUTS/'libSPenRenderer.so','runtime Renderer'),(RUNTIME_INPUTS/'libSPenGraphics.so','runtime Graphics')]:
 expected=EXPECTED['renderer' if label.endswith('Renderer') else 'graphics']
 if not copy_path.is_file() or digest(copy_path)!=expected: raise RuntimeError(f'{label} mapped-image input does not match exact DSO pin')
renderer_blob=(LIB/'libSPenRenderer.so').read_bytes()
phoff=struct.unpack_from('<Q',renderer_blob,32)[0]; phentsize,phcount=struct.unpack_from('<HH',renderer_blob,54)
renderer_ranges=[]
for i in range(phcount):
 typ,flags,off,vaddr,paddr,filesz,memsz,align=struct.unpack_from('<II6Q',renderer_blob,phoff+i*phentsize)
 if typ==1 and memsz: renderer_ranges.append((vaddr,vaddr+memsz,off,filesz,flags))
if not renderer_ranges or any(end>0x200000 for start,end,*_ in renderer_ranges): raise RuntimeError(f'pinned Renderer PT_LOAD range exceeds parent 0x200000 mapping: {renderer_ranges!r}')
try: uc_regions=tuple(adapter.u.mem_regions())
except Exception as exc: raise RuntimeError(f'cannot validate parent UC Renderer mapped-region metadata: {exc!r}') from exc
if not region_union_covers_all(iter(uc_regions),renderer_base,[(start,end) for start,end,*_ in renderer_ranges]):
 raise RuntimeError('one-shot UC region iterator did not cover all base-adjusted Renderer PT_LOAD spans')
for start,end,*_ in renderer_ranges:
 absolute_start,absolute_end=renderer_base+start,renderer_base+end
 if not region_union_covers(uc_regions,absolute_start,absolute_end):
  nearby=[(hex(begin),hex(region_end),perms) for begin,region_end,perms in uc_regions
          if begin<=absolute_end and region_end+1>=absolute_start]
  raise RuntimeError(f'parent UC mapped-region union misses Renderer PT_LOAD absolute [{absolute_start:#x},{absolute_end:#x}) (RVA [{start:#x},{end:#x})); nearby inclusive regions={nearby!r}')
for rva,word in {0x5ae38:0xa9be7bfd,0x5475c:0xa9be7bfd,0x547ec:0xd10103ff,0x54900:0xd10603ff}.items():
 if struct.unpack('<I',adapter.u.mem_read(renderer_base+rva,4))[0]!=word:
  raise RuntimeError(f'parent Renderer mapped bytes differ from pinned DSO at RVA {rva:#x}')
renderer_exports=[
 ('SPen::GLES::RendererFactoryImpl::CreateOffscreenRenderTarget(SPen::ITextureObject*)',0x5ae38),
 ('SPen::GLES::OffscreenRenderTarget::OffscreenRenderTarget(SPen::ITextureObject*)',0x5475c),
 ('SPen::GLES::OffscreenRenderTarget::CreateFbo()',0x547ec),
 ('SPen::GLES::OffscreenRenderTarget::AttachColorToFbo()',0x54900),
]
for symbol,rva in renderer_exports:
 if p.syms.get(symbol)!=renderer_base+rva: raise RuntimeError(f'parent Renderer export {symbol!r} is not at pinned RVA {rva:#x}')
if adapter.gles is None or not adapter.callbacks: raise RuntimeError('parent typed GLES/Renderer adapter state is incomplete')
# The parent V2 setup already relocated and initialized Renderer and installed
# its production factory methods; do not map or run its initializers again.
graphics_base=runtime.bases.get('Graphics')
if graphics_base is None: raise RuntimeError('parent Drawing runtime has no mapped Graphics DSO')
for rva,word in {0x8d89c:0xd101c3ff,0x8ddd0:0xf9400008,0x8dddc:0xd61f0040,0x8ddf0:0xa9bd7bfd}.items():
 if struct.unpack('<I',p.u.mem_read(graphics_base+rva,4))[0]!=word:
  raise RuntimeError(f'parent Graphics mapped bytes differ from pinned DSO at RVA {rva:#x}')
for symbol,rva in [(SYMBOLS['subbitmap_ctor'],0x8d89c),(SYMBOLS['subbitmap_create_framebuffer'],0x8ddd0)]:
 if p.syms.get(symbol)!=graphics_base+rva: raise RuntimeError(f'parent Graphics export {symbol!r} is not at pinned RVA {rva:#x}')
factory_callbacks=('SPen::RendererFactory::CreateGraphicsObject(unsigned int, SPen::PrimitiveType, SPen::VertexDescriptor const*, unsigned int, unsigned int)',
                   'SPen::RendererFactory::CreateBlendStateObject(SPen::BlendStateDescriptor const&)')
if any(name in p.callbacks for name in factory_callbacks): raise RuntimeError('parent native Renderer factory implementation is unexpectedly replaced by a callback')
# Match the already-reviewed renderer callback surface that the adapter class
# does not install itself; these remain typed direct host queries/forwards.
p.callbacks['glGetString']=adapter.get_string
p.callbacks['glGetIntegerv']=adapter.gl_get_integer
p.callbacks['glGetError']=lambda:p.x(0,adapter.gles.f['glGetError']())
p.callbacks['glPixelStorei']=lambda:(adapter.gles.f['glPixelStorei'](p.x(0),p.x(1)),adapter.gl_calls.append({'call':'glPixelStorei','pname':p.x(0),'value':p.x(1)}))
# Preserve the actual active-unit texture binding beside each native storage
# allocation so the target texture can be joined to its RGBA8 allocation.
_raw_tex_image=p.callbacks.get('glTexImage2D')
if _raw_tex_image is None: raise RuntimeError('pinned Mesa adapter lacks glTexImage2D')
def _tex_image_with_binding():
 bound=adapter.gl_state().get('texture2d')
 _raw_tex_image()
 if adapter.gl_calls and adapter.gl_calls[-1].get('call')=='glTexImage2D': adapter.gl_calls[-1]['boundTexture']=bound
p.callbacks['glTexImage2D']=_tex_image_with_binding
# The canonical Marker3 adapter has no framebuffer_ids/texture_ids members.
# Passively observe the existing typed glGen* callback output arrays instead;
# each wrapper invokes its exact prior callback once and records the real IDs
# written by that callback into the native output span.
generated_framebuffer_rows=[]; generated_texture_rows=[]
def _install_gl_name_observer(name,sink):
 prior=p.callbacks.get(name)
 if not callable(prior): raise RuntimeError(f'canonical Marker3 adapter lacks {name} callback')
 def read_args(): return int(p.x(0)),int(p.x(1))
 def read_output(pointer,count): return p.get(pointer,'I'*count)
 p.callbacks[name]=wrap_id_generator(name,prior,read_args,read_output,sink,max_count=64)
 return prior
_prior_gen_framebuffers=_install_gl_name_observer('glGenFramebuffers',generated_framebuffer_rows)
_prior_gen_textures=_install_gl_name_observer('glGenTextures',generated_texture_rows)
# Renderer-facing callbacks remain methods bound to this adapter, whose Uc is
# the live Drawing Uc. Its state/event lists are the authoritative host ledger.
gl_error_checks=[]
for name,callback in list(p.callbacks.items()):
 if name.startswith('gl') and name!='glGetError':
  def guarded_factory(glname,fn):
   def guarded():
    before=adapter.gles.f['glGetError']()
    if before: raise RuntimeError(f'preexisting GLES error before {glname}: {before:#x}')
    fn(); after=adapter.gles.f['glGetError']()
    gl_error_checks.append({'call':glname,'before':before,'after':after})
    if after: raise RuntimeError(f'GLES error after {glname}: {after:#x}')
   return guarded
  p.callbacks[name]=guarded_factory(name,callback)
# Capture actual ARRAY_BUFFER identity for each buffer upload without changing the prior callback.
draw_query_api=bind_draw_query_api(adapter.gles.gl)
candidate_gl_start=len(adapter.gl_calls)
prior_buffer_data=p.callbacks.get('glBufferData')
if not callable(prior_buffer_data): raise RuntimeError('canonical callback table lacks glBufferData')
def observe_buffer_data():
 before=len(adapter.gl_calls); bound=current_array_buffer(draw_query_api)
 prior_buffer_data()
 annotate_buffer_upload(adapter.gl_calls,before,bound)
p.callbacks['glBufferData']=observe_buffer_data
prior_gl_shader_source=p.callbacks.get('glShaderSource')
if not callable(prior_gl_shader_source): raise RuntimeError('actual parent callback table lacks glShaderSource')
shader_source_observer,shader_source_events,shader_source_errors=capture_gl_shader_source(lambda i:p.x(i),lambda a,n:bytes(p.u.mem_read(a,n)))
p.callbacks['glShaderSource']=shader_source_observer(prior_gl_shader_source)
# Renderer was initialized once by the pinned parent V2 setup and is reused.

# Require native constructor state: Add arrays exist and Init-owned graphics/
# program slots are still null. Init is allowed to allocate only through the
# shipped native factory path.
def read_vec(ptr):
 begin,end,cap=p.get(ptr,'3Q')
 if not begin or end<begin or cap<end or (end-begin)%4 or end-begin>1_048_576: raise RuntimeError('invalid or oversized native vector')
 data=bytes(p.u.mem_read(begin,end-begin))
 return data,hashlib.sha256(data).hexdigest(),len(data)//4
pre_vectors=[read_vec(ptr) for ptr in vector_ptrs]
if [x[1] for x in pre_vectors]!=vector_hashes: raise RuntimeError('live Add vector bytes differ from pinned V2 receipt')
resource_offsets=(248,256,264,272,280,288)
pre_resources={off:p.get(receiver+off,'Q')[0] for off in resource_offsets}
if any(pre_resources.values()): raise RuntimeError(f'RTV1 already has Init-owned resources; refusing reinitialization: {pre_resources!r}')

# The V2 capture already owns the queue/context adapter and synchronous task
# hooks; preserve those exact objects for the added native paint calls.
call=ns['call']
rect=p.alloc(16); p.put(rect,'4i',0,0,ARGS.width,ARGS.height)
namebuf=p.alloc(32); p.u.mem_write(namebuf,b'montblanc-rtv1-mesa\0')
subbitmap=p.alloc(0x200)
matrix=p.alloc(64)
for label,rva in [('subbitmap_ctor',0x8d89c),('subbitmap_create_framebuffer',0x8ddd0)]:
 actual=p.syms.get(SYMBOLS[label])
 if actual!=graphics_base+rva: raise RuntimeError(f'{label} symbol/RVA mismatch: actual={actual!r} expected={graphics_base+rva:#x}')
call_plan=build_native_call_plan(record,SYMBOLS,receiver=receiver,vector_ptrs=vector_ptrs,queue=queue,subbitmap=subbitmap,rect=rect,namebuf=namebuf,matrix=matrix,width=ARGS.width,height=ARGS.height,argb=ARGS.argb)
call(SYMBOLS['init'],call_plan['init']['regs'])
post_resources={off:p.get(receiver+off,'Q')[0] for off in resource_offsets}
if any(not x for x in post_resources.values()): raise RuntimeError(f'native Init did not produce all three graphics/program resources: {post_resources!r}')
after_init=[read_vec(ptr) for ptr in vector_ptrs]
if after_init!=pre_vectors: raise RuntimeError('native Init changed Add-owned geometry vectors')
call(SYMBOLS['create_canvas'],call_plan['create_canvas']['regs'])

# Construct an actual render-target subbitmap. The ninth formal argument
# (render-target bool) is passed on the stack at entry SP+0, followed by layout
# enums at SP+8 and SP+16; it is not an X8 argument. The ctor creates the
# texture (+0x50) but explicitly zeros the framebuffer (+0x58). The separate
# source API CreateFrameBuffer() dispatches the native CreateFrameBufferRT
# implementation, which constructs SPFrameBuffer and stores +0x58.

def call_stack(symbol,reg_args,stack_args):
 for i,value in enumerate(reg_args):p.x(i,value)
 sp=0x200ef000; p.u.reg_write(R.UC_ARM64_REG_SP,sp)
 if stack_args:p.u.mem_write(sp,struct.pack('<'+'Q'*len(stack_args),*stack_args))
 stop=native_runtime_stop; p.u.reg_write(R.UC_ARM64_REG_LR,stop)
 p.u.emu_start(p.syms[symbol],stop,count=5_000_000)
 if p.u.reg_read(R.UC_ARM64_REG_PC)!=stop: raise RuntimeError(f'{symbol} failed PC={p.u.reg_read(R.UC_ARM64_REG_PC):#x}')
framebuffer_generation_start=len(generated_framebuffer_rows)
texture_generation_start=len(generated_texture_rows)
base_calls=len(adapter.gl_calls)
call_stack(call_plan['subbitmap_ctor']['symbol'],call_plan['subbitmap_ctor']['regs'],call_plan['subbitmap_ctor']['stack'])
if p.get(subbitmap,'Q')[0]==0: raise RuntimeError('native SPSubBitmapRT constructor did not install its vtable')
texture_after_ctor=p.get(subbitmap+0x50,'Q')[0]
fbo_after_ctor=p.get(subbitmap+0x58,'Q')[0]
if not texture_after_ctor or fbo_after_ctor:
 raise RuntimeError(f'SPSubBitmapRT constructor state differs from source: texture={texture_after_ctor:#x} framebuffer={fbo_after_ctor:#x}')
call(call_plan['subbitmap_create_framebuffer']['symbol'],call_plan['subbitmap_create_framebuffer']['regs'])
framebuffer_object=p.get(subbitmap+0x58,'Q')[0]
if not framebuffer_object or p.get(subbitmap+0x50,'Q')[0]!=texture_after_ctor:
 raise RuntimeError('native CreateFrameBuffer did not retain the constructor texture and create the +0x58 framebuffer object')
offscreen_target=p.get(framebuffer_object+0x10,'Q')[0]
if not offscreen_target: raise RuntimeError('native SPFrameBuffer has no RendererFactory offscreen target at +0x10')
native_target_fbo=p.get(offscreen_target+0x10,'I')[0]
if not native_target_fbo: raise RuntimeError('native OffscreenRenderTarget has no source-created framebuffer id at +0x10')

# Host-controlled style test, explicitly not a recovered Drawing color. The
# recorded size matches the selected V2 model input; color is chosen here.
color_setter_calls=[]
set_data_addr=p.syms[SYMBOLS['set_pen_data']]
def style_trace(u,pc,size,user):
 if pc==set_data_addr: color_setter_calls.append({'receiver':hex(p.x(0)),'argb':hex(p.x(1)),'size_s0_bits':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)).hex()})
p.u.hook_add(ns['UC_HOOK_CODE'],style_trace)
call(call_plan['set_pen_data']['symbol'],call_plan['set_pen_data']['regs'],call_plan['set_pen_data']['floats'])
call(call_plan['set_buffer']['symbol'],call_plan['set_buffer']['regs'])
call(call_plan['send']['symbol'],call_plan['send']['regs'])
if [p.get(receiver+off,'Q')[0] for off in (200,208,216)]!=vector_ptrs: raise RuntimeError('native SetBuffer did not retain exact Add-owned vectors')
if [p.get(receiver+off,'Q')[0] for off in (224,232,240)]!=vector_ptrs: raise RuntimeError('native SendDataToGPU did not retain exact Add-owned vectors')
upload_vectors=[read_vec(ptr) for ptr in vector_ptrs]
if upload_vectors!=pre_vectors: raise RuntimeError('native upload vectors differ from Add-owned geometry')
call(call_plan['update']['symbol'],call_plan['update']['regs'])
upload_calls=[x for x in adapter.gl_calls if x.get('call')=='glBufferData']
if len(upload_calls)<3: raise RuntimeError(f'native Update produced fewer than three VBO uploads: {len(upload_calls)}')
upload_hashes={x.get('dataSha256') for x in upload_calls}
if not set(vector_hashes).issubset(upload_hashes): raise RuntimeError('Mesa VBO uploads do not contain all exact native Add vectors')

# Initialize the exact constructor-owned RGBA8 FBO with a known host background,
# read it once, then let native Draw activate/deactivate that same subbitmap.
new_fbos=generated_ids_since(generated_framebuffer_rows,framebuffer_generation_start,'glGenFramebuffers')
new_textures=generated_ids_since(generated_texture_rows,texture_generation_start,'glGenTextures')
ctor_gl=adapter.gl_calls[base_calls:]
if len(new_fbos)!=1 or len(new_textures)!=1:
 raise RuntimeError(f'native target lifecycle did not generate exactly one actual host FBO and texture: framebuffer_ids={new_fbos!r} texture_ids={new_textures!r}')
target_fbo=int(new_fbos[0])
target_fbo_candidates=[x['id'] for x in ctor_gl if x.get('call')=='glBindFramebuffer' and x.get('target')==0x8D40 and x.get('id')==target_fbo]
if len(target_fbo_candidates)!=1 or target_fbo_candidates[0]!=native_target_fbo:
 raise RuntimeError(f'native target FBO generation/store/bind do not uniquely join: generated={new_fbos!r} object={native_target_fbo} binds={target_fbo_candidates!r}')
native_attach_success=p.get(offscreen_target+72,'B')[0]
if native_target_fbo!=target_fbo or native_attach_success!=1:
 raise RuntimeError(f'native target object does not join the host FBO/completed color attachment: object_fbo={native_target_fbo} host_fbo={target_fbo} attach_success={native_attach_success}')
target_texture=int(new_textures[0])
setup_evidence=target_gl_evidence(adapter.gl_calls,base_calls,target_fbo,target_texture,ARGS.width,ARGS.height)
for key in ('binds','color_attachments','rgba8_allocations','complete_checks'):
 if not setup_evidence[key]: raise RuntimeError(f'native target GL event ledger lacks exact {key} evidence for FBO={target_fbo} texture={target_texture}')
gles=bind_host_gles(adapter.gles)
pre_result=prepare_target(gles,ARGS.width,ARGS.height,target_fbo,C.create_string_buffer)
pre_bytes=pre_result['bytes']; pre_sha=pre_result['sha256']
tx,ty=ARGS.translation
p.put(matrix,'16f',1,0,0,0,0,1,0,0,0,0,1,0,tx,ty,0,1)
draw_calls_before=sum(1 for x in adapter.gl_calls if x.get('call')=='glDrawArrays')
draw_attribute_snapshots=[]; prior_draw=p.callbacks.get('glDrawArrays')
if not callable(prior_draw): raise RuntimeError('canonical callback table lacks glDrawArrays')
def observe_draw_arrays():
 snapshot_error=None; snapshot=None
 try: snapshot=snapshot_draw_attributes(draw_query_api)
 except Exception as exc: snapshot_error=f'{type(exc).__name__}: {exc}'
 before=len(adapter.gl_calls); prior_draw()
 events=[row for row in adapter.gl_calls[before:] if row.get('call')=='glDrawArrays']
 draw_attribute_snapshots.append({'snapshot':snapshot,'error':snapshot_error,'event':events[-1] if len(events)==1 else None})
p.callbacks['glDrawArrays']=observe_draw_arrays
call(call_plan['draw']['symbol'],call_plan['draw']['regs'],limit=call_plan['draw']['limit'])
# Typed glFinish is bound from the same pinned GLES CDLL by host_gles_api.
draws=[x for x in adapter.gl_calls if x.get('call')=='glDrawArrays'][draw_calls_before:]
if len(draws)!=3: raise RuntimeError(f'expected 3 native GraphicsObject DrawArrays calls, got {len(draws)}')
if any(x.get('state',{}).get('framebuffer')!=target_fbo for x in draws): raise RuntimeError('a native draw did not target the exact SPSubBitmapRT framebuffer')
if [x.get('values',[None,None,None])[2] for x in draws]!=[row['float_count']//5 for row in vector_rows]: raise RuntimeError('DrawArrays counts differ from source vector lengths')
if len(draw_attribute_snapshots)!=len(draws) or any(x['snapshot'] is None or x['error'] or x['event'] is None for x in draw_attribute_snapshots): raise RuntimeError(f'incomplete actual draw attribute diagnostics: {draw_attribute_snapshots!r}')
buffer_upload_rows=[x for x in adapter.gl_calls[candidate_gl_start:] if x.get('call')=='glBufferData' and x.get('boundBuffer')]
draw_provenance=join_draw_provenance(draws,[x['snapshot'] for x in draw_attribute_snapshots],buffer_upload_rows,vector_hashes)
native_vector_draw_joins=require_native_vector_draw_joins(draw_provenance,vector_hashes)
post_result=finish_target_readback(gles,ARGS.width,ARGS.height,target_fbo,target_texture,C.create_string_buffer)
post_bytes=post_result['bytes']; post_sha=post_result['sha256']; final_error=post_result['gl_error_after']
pixel_statistics=rgba8_statistics(post_bytes,ARGS.width,ARGS.height)
pixel_statistics=rgba8_statistics(post_bytes,ARGS.width,ARGS.height)
if shader_source_errors: raise RuntimeError(f'glShaderSource telemetry incomplete: {shader_source_errors!r}')
if not shader_source_events: raise RuntimeError('candidate route did not produce any glShaderSource callback rows')
shader_diagnostics=compact_shader_sources(shader_source_events)
uniform_diagnostics=compact_uniform_trace(adapter.gl_calls,candidate_gl_start)

payload={'schema':'montblanc-rtv1-controlled-native-mesa-paint.v2',
 'execution_boundary':'V2 real Drawing/ObjectStroke/RTV1 Add* source producer is replayed unchanged. Init, SetBuffer, SendDataToGPU, Update and Draw are native method calls. SetBuffer/SendDataToGPU are explicit controlled API sequencing, not observed natural Drawing-worker calls. Style argb is host-controlled. Renderer/Graphics/GL lifecycle is native; EGL/GLES and queue/context support are host adapters. Pixels are host Mesa output, not device parity.',
 'case':ARGS.case,'input_pins':EXPECTED,'native_rt_receiver':hex(receiver),'native_subbitmap_receiver':hex(subbitmap),
 'native_add_vectors': [{'field':row['field_offset'],'object':hex(ptr),'float_count':row['float_count'],'byte_count':len(data),'sha256':sha} for row,ptr,(data,sha,count) in zip(vector_rows,vector_ptrs,pre_vectors)],
 'native_resource_slots_before_init':{str(k):hex(v) for k,v in pre_resources.items()},'native_resource_slots_after_init':{str(k):hex(v) for k,v in post_resources.items()},
 'native_method_sequence':['Init','CreatePenCanvas','SPSubBitmapRT(stack bool=true,RGBA8,RGBA)','SPSubBitmapRT::CreateFrameBuffer()','PenDrawableRT::SetPenData','SetBuffer(Add vectors)','SendDataToGPU(same Add vectors)','Update(false)','Draw','RGBA8 readback'],
 'runtime_interface_audit':runtime_interface,
 'style':{'mode':'host-controlled native SetPenData input','argb':hex(ARGS.argb),'size':float(record['synthetic_model_input']['pen_size']),'observed_calls':color_setter_calls},
 'target':{'descriptor_render_target_bool':True,'constructor_stack_args_sp_plus_0_8_16':[1,6,2],'constructor_texture_object':hex(texture_after_ctor),'framebuffer_object_after_ctor':hex(fbo_after_ctor),'create_framebuffer_api':'SPSubBitmapRT::CreateFrameBuffer() -> vtable CreateFrameBufferRT(false)','framebuffer_object_after_create':hex(framebuffer_object),'renderer_offscreen_target':hex(offscreen_target),'native_offscreen_framebuffer_id':native_target_fbo,'native_color_attach_success_byte':native_attach_success,'texture_memory_layout':6,'texture_target_layout':2,'size':[ARGS.width,ARGS.height],'framebuffer':target_fbo,'texture':target_texture,'read_attachment':post_result['attachment_texture'],'completeness':post_result['framebuffer_status']},
 'matrix_f32':[1,0,0,0,0,1,0,0,0,0,1,0,tx,ty,0,1],
 'draw_calls':draws,'draw_provenance':draw_provenance,'native_vector_draw_joins':native_vector_draw_joins,'vbo_uploads':upload_calls,
 'shader_source_diagnostics':shader_diagnostics,'uniform_diagnostics':uniform_diagnostics,'pixel_statistics':pixel_statistics,
 'shader_source_diagnostics':shader_diagnostics,'uniform_diagnostics':uniform_diagnostics,'pixel_statistics':pixel_statistics,
 'gl_name_generation':{'framebuffer_rows':generated_framebuffer_rows[framebuffer_generation_start:],'texture_rows':generated_texture_rows[texture_generation_start:],'setup_evidence':setup_evidence},
 'pre_readback':{'sha256':pre_sha,'rgba8_zlib_base64':base64.b64encode(zlib.compress(pre_bytes,9)).decode(),'gl_error_before':pre_result['gl_error_before'],'gl_error_after':pre_result['gl_error_after']},
 'post_readback':{'sha256':post_sha,'rgba8_zlib_base64':base64.b64encode(zlib.compress(post_bytes,9)).decode(),'gl_error_before':post_result['gl_error_before'],'gl_error_after':final_error},
 'gl_error_checks':gl_error_checks,
 'host_gles':adapter.gles.identity,'host_gl_paths':HOST_GL_PATHS,'host_gl_sha256':HOST_GL_HASHES,'queue_events':p.queue_trace}
ARGS.output.parent.mkdir(parents=True,exist_ok=True)
with ARGS.output.open('x',encoding='utf-8') as f: f.write(json.dumps(payload,indent=2,sort_keys=True)+'\n')
print(f'wrote {ARGS.output} ({len(post_bytes)} RGBA8 bytes)')
