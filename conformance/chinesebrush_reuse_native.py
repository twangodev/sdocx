#!/usr/bin/env python3
"""Replay bounded ChineseBrush same-drawing reuse and setting-reset controls.

Run with the project-local runtime, for example:
  PYTHONPATH=conformance:scratch/apk-analysis-runtime/python \
  LD_LIBRARY_PATH=scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu \
  conformance/.venv/bin/python conformance/chinesebrush_reuse_native.py --check-fixture
"""
import argparse
import contextlib
import hashlib
import io
import json
import os
import struct
import subprocess
import sys
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
FIXTURE=ROOT/'conformance/chinesebrush-reuse-native.json'
SELF_PIN='a2cdfdb0d50271ee2afa4f87712842269fa1e118d3a51b7f6c129175916d2455'
ADAPTER=ROOT/'conformance/chinesebrush_saved_caller_native.py'
ADAPTER_SHA256='576846a4a060e3ee11eede9a3b9c626bcdb7b96a07d83a8305f119b628f3b27a'
CALLER_SHA256='66f4790e8eeb1ca867f3273a796303b6ffb108286916e1e245fc063cbd744ae0'
HELPER_PINS={
 'conformance/chinesebrush_saved_caller_native.py':ADAPTER_SHA256,
 'conformance/brushpen_saved_caller_native.py':CALLER_SHA256,
 'conformance/oblique_canvas_native.py':'1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2',
 'conformance/elf_plt.py':'816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b',
 'conformance/fountain_native.py':'40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30',
 'scratch/apk-analysis-runtime/python/unicorn/__init__.py':'9ad32123bca674cf205c308fd4d0d2a4db877c9dc2afac4feab7c84c8cf7043b',
 'scratch/apk-analysis-runtime/python/unicorn/unicorn_py3/unicorn.py':'62ffe84341f4288e309040e06555b81798031817d32fd19a945e250813f872f0',
}
DSO_PINS={
 'libSPenBase.so':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb',
 'libSPenChineseBrush.so':'25d06b71e525f129e53c3734b957cdc9279ff9bd8dbca19948351c5e5ad237f5',
 'libSPenComposer.so':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f',
 'libSPenDrawing.so':'788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd',
 'libSPenFountainPen.so':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec',
 'libSPenGraphics.so':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621',
 'libSPenModel.so':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a',
 'libSPenPenCommon.so':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d',
 'libSPenRenderer.so':'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed',
 'libc++_shared.so':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4',
}
RUNTIME_PINS={
 'scratch/apk-analysis-runtime/python/unicorn/arm64_const.py':'92f91a2b0a36cd34deee16db4f8da4ac0c9ae35c3ea1089296e5ef9d170a5be9',
 'scratch/apk-analysis-runtime/python/unicorn/unicorn_const.py':'556c6a27336dc6dd3e0d49348951bde17e1b0f69166f2217773e6b3a7199f45d',
 'scratch/apk-analysis-runtime/python/unicorn/unicorn.py':'e823b17649d0610c6f52b665902ac9dc46962785feebcac8465a0d7c477f25b7',
 'scratch/apk-analysis-runtime/python/unicorn/unicorn_py3/__init__.py':'ee70d3a7de9ea545406d727436ee835f9d644aeb7428f0e4f1510f9190b28726',
 'scratch/apk-analysis-runtime/python/unicorn/unicorn_py3/arch/__init__.py':'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
 'scratch/apk-analysis-runtime/python/unicorn/unicorn_py3/arch/arm64.py':'62963378864bae452f7cd2a784ddcd64ab6bdc8a6a69e033d48e8faaab0b2515',
 'scratch/apk-analysis-runtime/python/unicorn/unicorn_py3/arch/types.py':'2e103854e371ef576680073f44ead45b8177a73c450a6125e609b7c54e0bc844',
 'scratch/apk-analysis-runtime/python/unicorn/lib/libunicorn.so.2':'ddb196ec82b52e502c18e4a34478bf7b9f61c83c2ebaa95c74d8ded45a95da9c',
}
TOOL_PINS={
 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump':'b9ae08c9c02c91ced02b80a140c937ba119fe073872fff701f67477bf597a5eb',
 '/usr/bin/nm':'ddb55c0685721bf494aa2d090056b8b32a40614ad4464b305896f527d882c3c1',
 '/usr/bin/c++filt':'7d54ea6797a5e8c69255f711b79577efd9934c3502c6edee4bc25b7dc7d97bd2',
}
def sha(raw): return hashlib.sha256(raw).hexdigest()
def normalized_self_hash():
 text=Path(__file__).read_text()
 text=text.replace(f"SELF_PIN='{SELF_PIN}'", "SELF_PIN='<SELF_PIN>'")
 return sha(text.encode())
def preflight():
 got=normalized_self_hash()
 if got!=SELF_PIN: raise SystemExit(f'runner self-pin mismatch: {got} != {SELF_PIN}')
 for rel,expected in {**HELPER_PINS,**RUNTIME_PINS,**TOOL_PINS}.items():
  path=Path(rel) if rel.startswith('/') else ROOT/rel
  actual=sha(path.read_bytes())
  if actual!=expected: raise SystemExit(f'pre-import pin mismatch {rel}: {actual} != {expected}')
 libdir=ROOT/'scratch/apk-analysis-native/arm64-v8a'
 for name,expected in DSO_PINS.items():
  actual=sha((libdir/name).read_bytes())
  if actual!=expected: raise SystemExit(f'pre-import DSO pin mismatch {name}: {actual} != {expected}')

def replace_once(source, old, new, count=1):
 actual=source.count(old)
 if actual!=count: raise RuntimeError(f'patch anchor count {actual}, expected {count}: {old[:120]!r}')
 return source.replace(old,new,count)

preflight()

reuse_code=r'''
# Reuse probe: a distinct prelude ObjectStroke is drawn before the wrapper's
# unchanged target ObjectStroke on the same native ObjectDrawing/canvas/pen.
reuse_config=json.loads(os.environ['CHINESE_BRUSH_REUSE_CONFIG'])
reuse_sequence=[]
release_symbol='SPen::ChineseBrushStrokeDrawableRTV1::Release()'
release_address=p.syms[release_symbol]
release_events=[]
def _release_trace(_uc,_a,_sz,_data):
 if _a in (release_address,release_address+0x1ac):
  row={'phase':phase,'kind':'entry' if _a==release_address else 'normal_epilogue','pc':hex(_a),'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))}
  if _a==release_address:row['receiver']=hex(p.x(0))
  else:row['restored_caller_x19']=hex(p.x(19))
  release_events.append(row)
p.u.hook_add(UC_HOOK_CODE,_release_trace)
p.pen_canvas_detach_calls=[]
_pen_vtable=struct.unpack('<Q',p.u.mem_read(p.pen_canvas,8))[0]
p.u.mem_write(_pen_vtable+208,struct.pack('<Q',0x200f2080))
p.u.mem_write(0x200f2080,bytes.fromhex('1f2003d5c0035fd6'))
def _pen_canvas_detach(_uc,_a,_sz,_data):
 p.pen_canvas_detach_calls.append({'canvas':hex(p.x(0)),'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})
 p.x(0,0);p.u.reg_write(R.UC_ARM64_REG_PC,p.u.reg_read(R.UC_ARM64_REG_LR))
p.u.hook_add(UC_HOOK_CODE,_pen_canvas_detach,begin=0x200f2080,end=0x200f2080)
def _make_reuse_stroke(spec):
 _o=p.alloc(0x100); _impl=p.alloc(0x400)
 p.call('SPen::ObjectStroke::ObjectStroke()',[_o])
 p.call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)',[_impl,_o])
 p.put(_o+32,'Q',_impl)
 p.f(0,float(spec.get('size',input_case['size'])));p.call('SPen::ObjectStroke::SetPenSize(float)',[_o])
 p.call('SPen::ObjectStroke::SetSizeLevel(int)',[_o,int(spec.get('size_level',30))])
 p.call('SPen::ObjectStroke::SetColor(unsigned int)',[_o,int(spec.get('color_argb',0xff6cdd8f))])
 p.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[_o,int(spec.get('tool_type',2))])
 p.call('SPen::ObjectStroke::SetStrokeType(SPen::StrokeType)',[_o,int(spec.get('stroke_type',0))])
 p.call('SPen::ObjectStroke::SetCurveEnabled(bool)',[_o,int(spec.get('curve_enabled',True))])
 _pts=spec['points'];_n=len(_pts)
 _point_mem=p.alloc(_n*8);_pressure_mem=p.alloc(_n*4);_time_mem=p.alloc(_n*4);_tilt_mem=p.alloc(_n*4);_orient_mem=p.alloc(_n*4)
 p.u.mem_write(_point_mem,struct.pack('<'+'f'*(2*_n),*(v for xy in _pts for v in xy)))
 p.u.mem_write(_pressure_mem,struct.pack('<'+'f'*_n,*spec['pressure']))
 p.u.mem_write(_time_mem,struct.pack('<'+'i'*_n,*spec['timestamps']))
 p.u.mem_write(_tilt_mem,struct.pack('<'+'f'*_n,*spec['tilt']))
 p.u.mem_write(_orient_mem,struct.pack('<'+'f'*_n,*spec['orientation']))
 p.call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[_o,_point_mem,_pressure_mem,_time_mem,_tilt_mem,_orient_mem,_n])
 p.call('SPen::ObjectBase::SetVisibility(bool)',[_o,1])
 p.call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[_o,pname])
 _setting=spec.get('advanced_setting',args.advanced_setting)
 _str=p.alloc(32);p.call('SPen::String::String()',[_str]);p.call('SPen::String::Construct()',[_str])
 _cstr=p.alloc(128);p.u.mem_write(_cstr,_setting.encode('ascii')+b'\0');p.call('SPen::String::Set(char const*)',[_str,_cstr])
 p.call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[_o,_str])
 return _o,{'object':hex(_o),'impl':hex(_impl),'setting':_setting,'source_arrays':snapshot_source_arrays(_o)}
if reuse_config.get('skip_draw',False):
 _prelude=0;_pre_meta=None
else:
 _prelude,_pre_meta=_make_reuse_stroke(reuse_config)
if not reuse_config.get('skip_draw',False):
 _phase_before=phase
 phase='reuse_prelude'
 _factory_before=len(factory_created_pens)
 _event_before=len(chinesebrush_buffer_events)
 try:
  _pre_result=p.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[p.drawing,p.draw_canvas,p.pen_canvas,_prelude,1],limit=5000000)
 except Exception as _e:
  raise RuntimeError(f'prelude drawObjectStroke failed: {_e!r}')
 _pens_after=list(factory_created_pens[_factory_before:])
 _pre_pen=_pens_after[-1] if _pens_after else (factory_created_pens[-1] if factory_created_pens else 0)
 _pre_drawable=0;_pre_rt=0;_pre_v=0
 if _pre_pen:
  p.call('SPen::ChineseBrush::GetStrokeDrawableGL()',[_pre_pen]);_pre_drawable=p.x(0)
  p.call('SPen::Pen::getVersion()',[_pre_pen]);_pre_v=p.x(0)
  if _pre_drawable:_pre_rt=p.get(_pre_drawable+16,'Q')[0]
 _pre_vector=p.get(_pre_rt+80,'Q')[0] if _pre_rt else 0
 _pre_bounds=p.get(_pre_vector,'3Q') if _pre_vector else (0,0,0)
 _pre_bytes=bytes(p.u.mem_read(_pre_bounds[0],_pre_bounds[1]-_pre_bounds[0])) if _pre_vector and _pre_bounds[1]>=_pre_bounds[0] else b''
 reuse_sequence.append({'phase':'prelude','source':_pre_meta,'draw_return':_pre_result,'phase_value_before':_phase_before,
  'factory_created_pen_addresses':[hex(v) for v in _pens_after],'selected_pen':hex(_pre_pen) if _pre_pen else None,
  'selected_native_version':_pre_v,'selected_drawable':hex(_pre_drawable) if _pre_drawable else None,'selected_rt':hex(_pre_rt) if _pre_rt else None,
  'rt_vector_object':hex(_pre_vector) if _pre_vector else None,'vector_descriptor':[hex(v) for v in _pre_bounds],
  'vector_bytes':_pre_bytes.hex(),'vector_sha256':hashlib.sha256(_pre_bytes).hexdigest(),
  'phase_addpoint_count':len(chinesebrush_buffer_events)-_event_before,
  'phase_addpoint_events':chinesebrush_buffer_events[_event_before:],
  'drawable_memory':bytes(p.u.mem_read(_pre_drawable,0x180)).hex() if _pre_drawable else None,
  'rt_memory':bytes(p.u.mem_read(_pre_rt,0x130)).hex() if _pre_rt else None})
else:
 _pre_result=None;_pre_pen=0;_pre_drawable=0;_pre_rt=0;_pre_v=0;_pre_vector=0;_pre_bounds=(0,0,0);_pre_bytes=b'';_event_before=len(chinesebrush_buffer_events)
phase='draw_object_stroke'
'''
# The wrapper calls the transformed canonical caller once per private child.
# Insert the prelude before its one target Draw call and append evidence to its JSON.
base_draw="phase='draw_object_stroke';"
output_anchor=" encoded=json.dumps(capture,indent=2,sort_keys=True)+'\\n'"
inner_anchor='exec(compile(source, str(CALLER), "exec"), namespace)'
output_anchor=" encoded=json.dumps(capture,indent=2,sort_keys=True)+'\\n'"
_native_init_old=" _original_native_init(self, library_dir, objdump, tuple('ChineseBrush' if item=='ObliquePen' else item for item in additional_libraries))"
_native_init_new=""" _original_native_init(self, library_dir, objdump, tuple('ChineseBrush' if item=='ObliquePen' else item for item in additional_libraries))
 _renderer_path=pathlib.Path(library_dir)/'libSPenRenderer.so'
 _renderer_blob=_renderer_path.read_bytes()
 if hashlib.sha256(_renderer_blob).hexdigest()!='f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed': raise RuntimeError('unverified Renderer DSO')
 _renderer_base=0x9000000
 self.u.mem_map(_renderer_base,0x200000)
 _phoff=struct.unpack_from('<Q',_renderer_blob,32)[0];_phentsize,_phnum=struct.unpack_from('<HH',_renderer_blob,54)
 for _i in range(_phnum):
  _kind,_flags,_fo,_va,_pa,_fs,_ms,_align=struct.unpack_from('<II6Q',_renderer_blob,_phoff+_i*_phentsize)
  if _kind==1 and _fs:self.u.mem_write(_renderer_base+_va,_renderer_blob[_fo:_fo+_fs])
 for _line in subprocess.check_output(['nm','-D','-C',str(_renderer_path)],text=True).splitlines():
  _parts=_line.strip().split(' ',2)
  if len(_parts)==3 and _parts[1] in ('T','W'):self.syms[_parts[2]]=_renderer_base+int(_parts[0],16)
 _dis=subprocess.check_output([str(oc.OBJDUMP),'-d','-C','-j','.plt',str(_renderer_path)],text=True)
 for _a,_sym in re.findall('^([0-9a-f]+) <(.+)@plt>:',_dis,re.M):self.plt[_renderer_base+int(_a,16)]=_sym
 self._emutls_current_manager=self.alloc(8);self.u.mem_write(self._emutls_current_manager,bytes(8))
 self._emutls_events=[]
 def _host_emutls_get_address():
  self._emutls_events.append({'tls_descriptor':hex(self.x(0)),'manager_slot':hex(self._emutls_current_manager)})
  self.x(0,self._emutls_current_manager)
 self.callbacks['__emutls_get_address']=_host_emutls_get_address
 # Match the pinned Crayon2 Renderer lifecycle prerequisite: construct the real
 # ShaderManager and publish it to Renderer::GetInstance's verified global.
 self.callbacks['operator new(unsigned long, std::nothrow_t const&)']=lambda:self.x(0,self.alloc(self.x(0)))
 self._host_mutex_events=[]
 def _host_mutex_call(operation):
  def _call():
   self._host_mutex_events.append({'operation':operation,'address':hex(self.x(0))})
   self.x(0,0)
  return _call
 self.callbacks['pthread_mutex_init']=_host_mutex_call('init')
 self.callbacks['pthread_mutex_destroy']=_host_mutex_call('destroy')
 self.callbacks['pthread_mutex_lock']=_host_mutex_call('lock')
 self.callbacks['pthread_mutex_unlock']=_host_mutex_call('unlock')
 self.shader_manager=self.alloc(512)
 self.call('SPen::ShaderManager::ShaderManager()',[self.shader_manager])
 self.shader_manager_global=_renderer_base+0x6d480
 self.u.mem_write(self.shader_manager_global,struct.pack('<Q',self.shader_manager))
 self.call('SPen::ShaderManager::GetInstance()',[])
 _got_manager=self.x(0)
 if _got_manager!=self.shader_manager:
  raise RuntimeError(f'native ShaderManager singleton mismatch got={_got_manager:#x} expected={self.shader_manager:#x} global={struct.unpack("<Q",self.u.mem_read(self.shader_manager_global,8))[0]:#x}')
 self.renderer_setup={'constructor_symbol':'SPen::ShaderManager::ShaderManager()','constructor_rva':'0x45bdc','get_instance_rva':'0x45bd0','singleton_global_rva':'0x6d480','global_address':hex(self.shader_manager_global),'constructed_pointer':hex(self.shader_manager),'global_pointer':hex(struct.unpack('<Q',self.u.mem_read(self.shader_manager_global,8))[0]),'get_instance_matches':True}
 self.renderer_unregister_events=[];self.renderer_trace=[];_unregister=self.syms['SPen::ManagedObject::Unregister()']
 _unregister_pending=[]
 def _renderer_trace(_uc,_a,_sz,_data):
  if _renderer_base <= _a < _renderer_base+0x200000:
   self.renderer_trace.append({'pc':hex(_a-_renderer_base),'x0':hex(self.x(0)),'x1':hex(self.x(1)),'x8':hex(self.x(8)),'x9':hex(self.x(9)),'lr':hex(self.u.reg_read(R.UC_ARM64_REG_LR))})
   self.renderer_trace=self.renderer_trace[-80:]
 self.u.hook_add(UC_HOOK_CODE,_renderer_trace)
 self.all_trace=[]
 def _all_code(_uc,_a,_sz,_data):
  self.all_trace.append({'pc':hex(_a),'x0':hex(self.x(0)),'x1':hex(self.x(1)),'x2':hex(self.x(2)),'lr':hex(self.u.reg_read(R.UC_ARM64_REG_LR))})
  self.all_trace=self.all_trace[-120:]
 self.u.hook_add(UC_HOOK_CODE,_all_code)
 def _unregister_trace(_uc,_a,_sz,_data):
  if _a==_unregister:
   obj=self.x(0);subscription=struct.unpack('<Q',self.u.mem_read(obj+16,8))[0]
   event={'kind':'entry','address':hex(_a),'receiver':hex(obj),'managed_subscription':'null' if subscription==0 else hex(subscription),'tls_calls_before':len(self._emutls_events)}
   self.renderer_unregister_events.append(event);_unregister_pending.append(event)
  elif _a==_unregister+0x8c and _unregister_pending:
   entry=_unregister_pending.pop()
   self.renderer_unregister_events.append({'kind':'fastpath_return' if entry['managed_subscription']=='null' else 'return','address':hex(_a),'receiver':entry['receiver'],'return_pc':hex(_a),'tls_calls_before':entry['tls_calls_before'],'tls_calls_after':len(self._emutls_events)})
 self.u.hook_add(UC_HOOK_CODE,_unregister_trace)
 _original_call=self.call
 def _debug_call(_name,*_a,**_kw):
  try:return _original_call(_name,*_a,**_kw)
  except Exception as _error:raise RuntimeError(f'native call {_name} failed at PC={self.u.reg_read(R.UC_ARM64_REG_PC):#x} LR={self.u.reg_read(R.UC_ARM64_REG_LR):#x} SP={self.u.reg_read(R.UC_ARM64_REG_SP):#x} x0={self.x(0):#x} x1={self.x(1):#x}: {_error!r}; renderer trace={self.renderer_trace!r}; all trace={self.all_trace!r}') from _error
 self.call=_debug_call
"""
_target_input_old="input_case={'points':[[1.,-2.],[3.,1.],[6.,4.]],'size':3.144,'curve_enabled':True,'tool_type':2,'stroke_type':0,'pressure':[.3,.6,.85],'timestamps':[300,315,340],'tilt':[.12,.32,.55],'orientation':[.2,.35,.55]}"
_target_input_new="input_case={'points':[[0.,0.],[6.,3.],[9.,12.],[17.,13.],[23.,15.]],'size':3.144,'curve_enabled':True,'tool_type':2,'stroke_type':0,'pressure':[.3,.55,.75,.35,.95],'timestamps':[300,311,324,338,355],'tilt':[.12,.32,.55,.22,.65],'orientation':[.2,.35,.55,.18,.7]}"
_indented_reuse='\n'.join(' '+line if line else '' for line in reuse_code.splitlines())
_reuse_insert="phase='draw_object_stroke';\n"+_indented_reuse+"\n target_object_addr=obj; phase='draw_object_stroke';"
inner_patch=(f"if source.count({base_draw!r}) != 1: raise RuntimeError('phase anchor changed')\n"
            f"if source.count({_target_input_old!r}) != 1: raise RuntimeError('target source input anchor changed')\nsource=source.replace({_target_input_old!r}, {_target_input_new!r}, 1)\n"
            f"source=source.replace({base_draw!r}, {_reuse_insert!r}, 1)\n"
            f"if source.count({output_anchor!r}) != 1: raise RuntimeError('capture output anchor changed')\n"
            f"source=source.replace({output_anchor!r}, \" capture['reuse_object_addresses']={{'prelude':hex(_prelude),'target':hex(target_object_addr),'distinct':_prelude!=target_object_addr}}; capture['reuse_sequence']=reuse_sequence; capture['renderer_unregister_events']=p.renderer_unregister_events; capture['pen_canvas_detach_calls']=p.pen_canvas_detach_calls; capture['release_events']=release_events; capture['prelude_drawable_after_target_hex']=bytes(p.u.mem_read(_pre_drawable,0x180)).hex() if _pre_drawable else None; capture['prelude_rt_after_target_hex']=bytes(p.u.mem_read(_pre_rt,0x130)).hex() if _pre_rt else None; capture['renderer_setup']=p.renderer_setup; capture['renderer_sha256']='f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed'; capture['host_mutex_events']=p._host_mutex_events; capture['emutls_events']=p._emutls_events\\n\"+{output_anchor!r}, 1)\n"
            + "source=source.replace(\" if len(stream) != 80 or len(chinesebrush_buffer_events) != 5:\", \" if len(stream) == 0 or len(chinesebrush_buffer_events) == 0:\", 1)\n"
            + "source=source.replace(\"if screen_resolution_calls != [{'width':args.width,'height':args.height}]:\", \"if not screen_resolution_calls or any(v != {'width':args.width,'height':args.height} for v in screen_resolution_calls):\", 1)\n"
            + "source=source.replace(\"if not chinesebrush_buffer_events or any(event['phase'] != 'draw_object_stroke' for event in chinesebrush_buffer_events):\", \"if not chinesebrush_buffer_events or any(event['phase'] not in ('draw_object_stroke','reuse_prelude') for event in chinesebrush_buffer_events):\", 1)\n"
            + f"if source.count({_native_init_old!r}) != 1: raise RuntimeError('native init hook anchor changed: '+repr([l for l in source.splitlines() if '_original_native_init(self' in l or 'HASHES[' in l][:4]))\nsource=source.replace({_native_init_old!r}, {_native_init_new!r}, 1)\n"
            )
patch_code=inner_patch
adapter_source=ADAPTER.read_text()
if adapter_source.count(inner_anchor)!=1: raise SystemExit('committed adapter exec anchor changed')
anchor=inner_anchor
wrapper_source=adapter_source.replace(anchor,patch_code+anchor,1)
CASES=[]
for version in (1,2,3):
 target=f'curve-v{version}'
 for kind in ('short','long','different'):
  CASES.append({'id':f'same-v{version}-{kind}','target':target,'prelude':kind,'prelude_setting':f'{version};','relation':'same_setting'})
for target,kind,pre_setting in (
 ('curve-v1','short','2;'),('curve-v2','short','1;'),('curve-v3','short','2;'),
 ('curve-v1','long','2;'),('curve-v1','different','2;')):
 CASES.append({'id':f'transition-{kind}-{pre_setting[0]}-to-{target[-1]}','target':target,'prelude':kind,'prelude_setting':pre_setting,'relation':'setting_transition'})
for version in (1,2,3):
 CASES.append({'id':f'fresh-v{version}','target':f'curve-v{version}','prelude':'none','prelude_setting':f'{version};','relation':'fresh'})
CASES=[row for row in CASES if row['relation']=='fresh']+[row for row in CASES if row['relation']!='fresh']
CASE_BY_ID={row['id']:row for row in CASES}
PRELUDES={
 'short':{'points':[[1.,2.],[4.,3.],[8.,7.]],'pressure':[.3,.7,.9],'timestamps':[100,111,125],'tilt':[.1,.3,.6],'orientation':[.2,.4,.7]},
 'long':{'points':[[0.,0.],[4.,2.],[7.,9.],[12.,11.],[18.,13.],[24.,16.],[29.,21.],[35.,23.]],'pressure':[.3,.4,.6,.8,.7,.5,.9,1.],'timestamps':[100,109,121,132,145,159,171,186],'tilt':[.1,.2,.4,.3,.6,.4,.7,.5],'orientation':[.2,.3,.5,.4,.7,.6,.8,.9]},
 'different':{'points':[[30.,24.],[22.,20.],[14.,18.],[7.,9.],[1.,2.]],'pressure':[.9,.2,.6,.4,.8],'timestamps':[200,209,223,240,256],'tilt':[.6,.3,.7,.1,.5],'orientation':[.8,.4,.6,.2,.9]},
 'none':None,
}
EXPECTED_TARGET_ARRAYS={
 'orientations':'cdcc4c3e3333b33ecdcc0c3fec51383e3333333f',
 'points':'00000000000000000000c04000004040000010410000404100008841000050410000b84100007041',
 'pressures':'9a99993ecdcc0c3f0000403f3333b33e3333733f',
 'tilts':'8fc2f53d0ad7a33ecdcc0c3fae47613e6666263f',
 'timestamps':'2c01000037010000440100005201000063010000',
}

def expected_arrays(spec):
 return {
  'points':struct.pack('<'+'f'*(2*len(spec['points'])),*(value for point in spec['points'] for value in point)).hex(),
  'pressures':struct.pack('<'+'f'*len(spec['pressure']),*spec['pressure']).hex(),
  'timestamps':struct.pack('<'+'i'*len(spec['timestamps']),*spec['timestamps']).hex(),
  'tilts':struct.pack('<'+'f'*len(spec['tilt']),*spec['tilt']).hex(),
  'orientations':struct.pack('<'+'f'*len(spec['orientation']),*spec['orientation']).hex(),
 }

def run_capture(case):
 target=case['target'];kind=case['prelude'];spec=PRELUDES[kind]
 config={'skip_draw':kind=='none','advanced_setting':case['prelude_setting']}
 if spec is not None:config.update(spec)
 os.environ['CHINESE_BRUSH_REUSE_CONFIG']=json.dumps(config,separators=(',',':'))
 old_argv=sys.argv[:]
 target_capture={'curve-v1':'one','curve-v2':'two','curve-v3':'three'}[target]
 sys.argv=[str(ADAPTER),'--_capture',target_capture]
 out=io.StringIO()
 try:
  with contextlib.redirect_stdout(out):
   exec(compile(wrapper_source,str(ADAPTER),'exec'),{'__name__':'__main__','__file__':str(ADAPTER)})
 finally:sys.argv=old_argv
 value=json.loads(out.getvalue())
 pre=value['reuse_sequence'][0] if value['reuse_sequence'] else None
 value['reuse_probe']={'case':case,'prelude_drawn':kind!='none','prelude_setting':case['prelude_setting'],'target_setting':f"{target[-1]};",'prelude_source_object':pre['source'] if pre else None,'prelude_callback_count':pre['phase_addpoint_count'] if pre else 0,'prelude_callback_rows':pre['phase_addpoint_events'] if pre else [],'target_callback_rows':value['producer']['append_rows'][pre['phase_addpoint_count']:] if pre else value['producer']['append_rows'],'source_adapter_sha256':ADAPTER_SHA256,'canonical_caller_sha256':CALLER_SHA256,'patch_contract':'Pinned source text was adapted only at exact asserted anchors in memory; no repository source file is modified.'}
 return value

def validate_case(case,value,fresh_hashes):
 native=value['native_selection'];producer=value['producer'];probe=value['reuse_probe'];target_version=int(case['target'][-1])
 setup=value['renderer_setup']
 if not setup['get_instance_matches'] or setup['constructed_pointer']!=setup['global_pointer'] or setup['constructor_rva']!='0x45bdc':
  raise RuntimeError(f"{case['id']}: Renderer ShaderManager singleton prerequisite did not verify")
 if native['version']!=target_version:raise RuntimeError(f"{case['id']}: target GLV {native['version']} != {target_version}")
 if len(bytes.fromhex(producer['buffer_hex']))!=producer['buffer_bytes'] or sha(bytes.fromhex(producer['buffer_hex']))!=producer['buffer_sha256']:
  raise RuntimeError(f"{case['id']}: vector bytes/hash mismatch")
 if producer['buffer_bytes']!=256 or len(probe['target_callback_rows'])!=16 or producer['callback_count']!=16+probe['prelude_callback_count']:
  raise RuntimeError(f"{case['id']}: expected sixteen target RTV1 Vector4 records after the prelude")
 source=value['input']
 if source['point_count']!=5 or source['size_f32_bits']!='4c374940' or not source['curve_enabled'] or source['tool_type']!=2 or source['stroke_type']!=0:
  raise RuntimeError(f"{case['id']}: captured native source getters differ from the pinned five-point target")
 if source['advanced_setting_ascii']!=f'{target_version};' or source['native_model_getter_snapshot']['advanced_setting']!=f'{target_version};':
  raise RuntimeError(f"{case['id']}: advanced-setting source getter does not match target GLV")
 if {key:row['data_hex'] for key,row in source['source_arrays'].items()}!=EXPECTED_TARGET_ARRAYS:
  raise RuntimeError(f"{case['id']}: native target source arrays differ from the five-point witness")
 if any(len(row['float_args'])!=5 for row in producer['append_rows']):raise RuntimeError(f"{case['id']}: AddPoint arguments do not retain Vector4 plus scalar")
 if probe['prelude_drawn']:
  pre=value['reuse_sequence'][0]
  if not value['reuse_object_addresses']['distinct']:raise RuntimeError(f"{case['id']}: prelude and target ObjectStroke alias")
  if pre['selected_pen']!=native['pen_address']:raise RuntimeError(f"{case['id']}: pen identity changed")
  if {key:row['data_hex'] for key,row in pre['source']['source_arrays'].items()}!=expected_arrays(PRELUDES[case['prelude']]):
   raise RuntimeError(f"{case['id']}: native prelude getters differ from their explicit source arrays")
  if case['relation']=='same_setting':
   if pre['selected_native_version']!=target_version:raise RuntimeError(f"{case['id']}: prelude GLV differs")
   if pre['selected_rt']!=native['rt_address']:raise RuntimeError(f"{case['id']}: same-setting redraw replaced RT")
   expected={'short':6,'long':25,'different':21}[case['prelude']]
   if pre['phase_addpoint_count']!=expected:raise RuntimeError(f"{case['id']}: prelude AddPoint count changed")
   if value.get('release_events') or value.get('renderer_unregister_events'):raise RuntimeError(f"{case['id']}: unexpected release on same-setting redraw")
  else:
   if pre['selected_native_version']==target_version:raise RuntimeError(f"{case['id']}: transition control has same GLV")
   if pre['selected_rt']==native['rt_address']:raise RuntimeError(f"{case['id']}: setting transition did not replace RT")
   releases=value.get('release_events',[])
   if [row['kind'] for row in releases]!=['entry','normal_epilogue']:raise RuntimeError(f"{case['id']}: RT Release did not return normally")
   if releases[0].get('receiver')!=pre['selected_rt'] or releases[1].get('pc')!='0x503b580':raise RuntimeError(f"{case['id']}: Release receiver/epilogue evidence does not identify the old RTV1")
   if len(value.get('renderer_unregister_events',[]))!=2:raise RuntimeError(f"{case['id']}: expected Unregister entry and return")
   unreg=value['renderer_unregister_events']
   if unreg[0]['kind']!='entry' or unreg[0]['managed_subscription']!='null' or unreg[1]['tls_calls_before']!=unreg[1]['tls_calls_after']:
    raise RuntimeError(f"{case['id']}: Unregister did not take recorded null-subscription fast path")
   if unreg[0]['receiver']!=pre['selected_rt'] or unreg[1]['receiver']!=unreg[0]['receiver']:
    raise RuntimeError(f"{case['id']}: Unregister receiver does not identify the old RTV1")
   if unreg[1]['kind']!='fastpath_return' or len(value.get('pen_canvas_detach_calls',[]))!=1:
    raise RuntimeError(f"{case['id']}: expected one host detach callback and native fast return")
 else:
  if value.get('release_events') or value.get('renderer_unregister_events') or value.get('pen_canvas_detach_calls'):
   raise RuntimeError(f"{case['id']}: fresh control unexpectedly released a drawable")
 if case['relation']=='fresh':fresh_hashes[target_version]=producer['buffer_sha256']
 elif target_version in fresh_hashes and producer['buffer_sha256']!=fresh_hashes[target_version]:
  raise RuntimeError(f"{case['id']}: target vector differs from fresh GLV{target_version}")


def main():
 parser=argparse.ArgumentParser(description=__doc__)
 mode=parser.add_mutually_exclusive_group()
 mode.add_argument('--write-fixture',action='store_true',help='explicitly regenerate the complete reuse/reset fixture')
 mode.add_argument('--check-fixture',action='store_true',help='replay all cases and compare with fixture (default)')
 parser.add_argument('--_capture',help=argparse.SUPPRESS)
 args=parser.parse_args()
 if args._capture:
  if args._capture not in CASE_BY_ID:raise SystemExit(f'unknown capture case {args._capture}')
  print(json.dumps(run_capture(CASE_BY_ID[args._capture]),indent=2,sort_keys=True))
  return
 generated={};fresh_hashes={}
 for case in CASES:
  result=subprocess.run([sys.executable,str(Path(__file__).resolve()),'--_capture',case['id']],cwd=ROOT,text=True,capture_output=True,check=True)
  try:value=json.loads(result.stdout)
  except json.JSONDecodeError as error:raise RuntimeError(f"{case['id']}: capture produced invalid JSON: {result.stdout[:300]!r}; stderr={result.stderr[:300]!r}") from error
  validate_case(case,value,fresh_hashes)
  generated[case['id']]=value
 manifest={'schema':'chinesebrush-reuse-reset-native-v1','provenance':{'runner_sha256':sha(Path(__file__).read_bytes()),'committed_source_adapter_sha256':ADAPTER_SHA256,'canonical_caller_sha256':CALLER_SHA256,'helpers_sha256':HELPER_PINS,'runtime_components_sha256':RUNTIME_PINS,'mapped_and_called_dsos_sha256':DSO_PINS,'tools_sha256':TOOL_PINS,'python':{'executable':sys.executable,'version':sys.version.split()[0]},'source_adaptation':'The committed ChineseBrush adapter and BrushPen canonical caller are read only after all pins pass. The canonical three-point input literal is replaced at one asserted anchor with the explicit five-point witness below. Exact source anchors are asserted before in-memory adaptation; no generated source or fixture outside this conformance pair is imported.'},'scope':'Synthetic ObjectStroke inputs are sent through the pinned Model/ObjectDrawing/ChineseBrush GLV1/2/3 Drawing route and RTV1 AddPoint. Reuse controls run sequentially in one Drawing/canvas/Pen instance with separate ObjectStroke inputs. Fresh, same-setting and setting-transition cases are included. Renderer ShaderManager is natively constructed and installed at the pinned Renderer global, but this reproduces an explicit host loader prerequisite and is not Android loader/runtime evidence. The mock Canvas detach method and serialized pthread mutex routines are host lifecycle adapters. This validates bounded native saved redraw state behavior, not SDK deserialization, Android thread scheduling, GPU paint, export, or device parity.','target_source_contract':{'point_count':5,'size_f32_bits':'4c374940','curve_enabled':True,'tool_type':2,'stroke_type':0,'arrays':EXPECTED_TARGET_ARRAYS},'renderer_lifecycle':{'renderer_sha256':DSO_PINS['libSPenRenderer.so'],'shader_manager_constructor_rva':'0x45bdc','get_instance_rva':'0x45bd0','singleton_global_rva':'0x6d480','unregister_rva':'0x45480','unregister_fastpath_return_rva':'0x4550c','release_rtv1_rva':'0x3b3d4','release_normal_epilogue_rva':'0x3b580','host_boundaries':['The host installs the constructed ShaderManager pointer at the verified Renderer global, following the pinned Crayon2 paint harness setup.','pthread_mutex_init/lock/unlock/destroy are serialized single-thread host adapters; contention and Android bionic mutex semantics are not tested.','CanvasProbe vtable slot +208 is populated with a logged host detach callback so native Drawable destruction can return; its app implementation is not recovered here.','Unregister records show the null subscription fast path, which does not call the TLS manager accessor. This is not evidence for subscribed-object unregister behavior.']},'cases':generated,'case_index':[{'id':c['id'],'relation':c['relation'],'prelude':c['prelude'],'prelude_setting':c['prelude_setting'],'target':c['target'],'target_vector_sha256':generated[c['id']]['producer']['buffer_sha256'],'target_vector_bytes':generated[c['id']]['producer']['buffer_bytes']} for c in CASES]}
 encoded=json.dumps(manifest,indent=2,sort_keys=True)+'\n'
 if args.write_fixture:
  FIXTURE.write_text(encoded);print(f'wrote {FIXTURE.relative_to(ROOT)} ({len(CASES)} cases)')
 else:
  if not FIXTURE.exists() or json.loads(FIXTURE.read_text())!=manifest:raise SystemExit('ChineseBrush reuse fixture mismatch; use --write-fixture to explicitly refresh')
  print(f'verified {len(CASES)} ChineseBrush reuse/reset cases')

if __name__=='__main__':main()
