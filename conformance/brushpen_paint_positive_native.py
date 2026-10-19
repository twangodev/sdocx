"""Actual BrushPen RTV1 Draw and GraphicsObject routed to host Mesa GLES.

Synthetic explicit motion/profile inputs and host queue/bitmap adapters remain;
shader, texture/geometry producers, Renderer methods and Draw methods are pinned
native. This is not device capture or Samsung GPU parity.
"""
import ctypes as C
import base64
import hashlib
import json
import argparse
import os
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUN = ROOT / 'conformance'
sys.path.insert(0, str(ROOT / 'conformance'))
import brushpen_texture_native as base
import inkpen_paint_native as ip
from unicorn import arm64_const as R

GEOMETRY = ROOT / 'conformance/brushpen-saved-native.json'
TEXTURE_FIXTURE = ROOT / 'conformance/brushpen-texture-native.json'
FACTORY = ('SPen::RendererFactory::CreateTextureObject(SPen::TextureDescriptor const&, char const*, '
           'void const*, unsigned int const*, unsigned int, SPen::TextureBufferInfo const*)')
FACTORY_IMPL = ('SPen::GLES::RendererFactoryImpl::CreateTextureObject(SPen::TextureDescriptor const&, char const*, '
                'void const*, unsigned int const*, unsigned int, SPen::TextureBufferInfo const*)')
BLEND_CTOR = 'SPen::GLES::BlendStateObject::BlendStateObject(SPen::BlendStateDescriptor const&)'
BLEND_SET = 'SPen::GLES::BlendStateObject::SetDescriptor(SPen::BlendStateDescriptor const&)'
BLEND_ACTIVATE = 'SPen::GLES::BlendStateObject::Activate()'

class PositiveProbe(base.TextureProbe):
    def __init__(self, poison_variant='native-diagnostic'):
        self.poison_variant = poison_variant
        self.mesa = ip.GLES()
        self.mesa.f.update({
            'glDisableVertexAttribArray': ip.cfunc(self.mesa.gl, 'glDisableVertexAttribArray', None,[C.c_uint]),
            'glBindVertexArray': ip.cfunc(self.mesa.gl, 'glBindVertexArray', None,[C.c_uint]),
            'glGenVertexArrays': ip.cfunc(self.mesa.gl, 'glGenVertexArrays', None,[C.c_int,C.POINTER(C.c_uint)]),
            'glTexSubImage2D': ip.cfunc(self.mesa.gl, 'glTexSubImage2D', None,
                [C.c_uint,C.c_int,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p]),
            'glUniform1f': ip.cfunc(self.mesa.gl, 'glUniform1f', None, [C.c_int,C.c_float]),
            'glUniform2f': ip.cfunc(self.mesa.gl, 'glUniform2f', None, [C.c_int,C.c_float,C.c_float]),
            'glBufferSubData': ip.cfunc(self.mesa.gl, 'glBufferSubData', None,
                [C.c_uint,C.c_ssize_t,C.c_ssize_t,C.c_void_p]),
            'glGetIntegerv': ip.cfunc(self.mesa.gl, 'glGetIntegerv', None,
                [C.c_uint,C.POINTER(C.c_int)]),
            'glGenSamplers': ip.cfunc(self.mesa.gl, 'glGenSamplers', None,
                [C.c_int,C.POINTER(C.c_uint)]),
            'glBindSampler': ip.cfunc(self.mesa.gl, 'glBindSampler', None,
                [C.c_uint,C.c_uint]),
            'glSamplerParameteri': ip.cfunc(self.mesa.gl, 'glSamplerParameteri', None,
                [C.c_uint,C.c_uint,C.c_int]),
        })
        self.gl_get_string_ptr = ip.cfunc(self.mesa.gl, 'glGetString', C.c_void_p, [C.c_uint])
        ip.cfunc(self.mesa.gl, 'glColorMask', None,
                 [C.c_ubyte,C.c_ubyte,C.c_ubyte,C.c_ubyte])
        super().__init__()
        max_texture=C.c_int(); self.mesa.f['glGetIntegerv'](0x0D33,C.byref(max_texture))
        self.put(0x5073388,'II',max_texture.value,max_texture.value)
        self.u.mem_write(0x50733c0,b'\x01')
        self.gl_calls_real = []
        self.gl_errors = []
        self.texture_ids = []
        self.sampler_ids = []
        self.texture_parameter_calls = []
        self.actual_buffer_ids = []
        self.buffer_id_map = {}
        self.synthetic_buffer_id = 500
        self.texture_uploads_real = []
        self.program = 0
        self.uniform_locations = {}
        self.uniform_values = {}
        self.synthetic_bitmap_matrix=[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1]
        geometry=json.loads(GEOMETRY.read_text())
        selected=next(c for c in geometry['synthetic_cases'] if c['label']=='synthetic-motion-tool1-pressure-tilt-orientation')
        words=selected['buffer_records'][1]['instance_bits_le']
        cx,cy=struct.unpack('<2f',b''.join(bytes.fromhex(x) for x in words[:2]))
        sx,sy=16/321,16/228
        self.synthetic_caller_matrix=[sx,0,0,0,0,sy,0,0,0,0,1,0,-sx*cx,-sy*cy,0,1]
        self.synthetic_start=(cx,cy)
        self.native_blend = 0
        self.output_texture = 0
        self.output_fbo = 0
        self.output_fbo_status = 0
        self.readback = b''
        self.native_factory_calls = []
        self.callbacks.pop(FACTORY, None)
        self.callbacks['__cxa_allocate_exception'] = self.allocate_exception
        self.callbacks['__cxa_throw'] = self.native_throw
        self.callbacks['std::logic_error::logic_error(char const*)'] = self.logic_error_ctor
        self.callbacks['__strncpy_chk'] = self.checked_strncpy
        self.callbacks.update({
            'glGenTextures': self.gl_gen_textures,
            'glGenSamplers': self.gl_gen_samplers,
            'glBindSampler': self.gl_bind_sampler_real,
            'glSamplerParameteri': self.gl_sampler_parameter_real,
            'glBindTexture': self.gl_bind_texture_real,
            'glTexParameteri': self.gl_tex_parameter,
            'glPixelStorei': self.gl_pixel_store_real,
            'glTexImage2D': self.gl_tex_image_real,
            'glTexSubImage2D': self.gl_tex_sub_image_real,
            'glGetError': self.gl_get_error,
            'glGenBuffers': self.gl_gen_buffers,
            'glBindBuffer': self.gl_bind_buffer_real,
            'glBufferData': self.gl_buffer_data_real,
            'glBufferSubData': self.gl_buffer_sub_data_real,
            'glEnableVertexAttribArray': self.gl_enable_attrib,
            'glDisableVertexAttribArray': self.gl_disable_attrib,
            'glVertexAttribPointer': self.gl_vertex_attrib_pointer_real,
            'glVertexAttribDivisor': self.gl_vertex_attrib_divisor_real,
            'glDrawArraysInstanced': self.gl_draw_arrays_instanced_real,
            'glGetIntegerv': self.gl_get_integer_real,
            'glGetString': self.gl_get_string_real,
            'glActiveTexture': self.gl_active_texture,
            'glEnable': self.gl_enable_real,
            'glDisable': self.gl_disable_real,
            'glBlendEquationSeparate': self.gl_blend_equation_real,
            'glBlendFuncSeparate': self.gl_blend_func_real,
            'glColorMask': self.gl_color_mask_real,
            'eglGetCurrentContext': lambda: self.x(0, 0xabc),
            'strstr': self.gl_strstr_real,
            'strcmp': self.gl_strcmp_real,
        })

    def _bounded_cstring(self, pointer, limit=65536):
        if not pointer: return b''
        result=bytearray()
        while len(result)<limit:
            amount=min(4096,limit-len(result))
            chunk=bytes(self.u.mem_read(pointer+len(result),amount))
            zero=chunk.find(b'\0')
            if zero>=0:return bytes(result)+chunk[:zero]
            result.extend(chunk)
        raise RuntimeError(f'native GLES query string exceeds {limit} bytes')

    def allocate_exception(self):
        size=self.x(0)
        if not 0 < size <= 4096: raise RuntimeError(f'exception allocation out of bound: {size}')
        pointer=self.alloc(size); self.x(0,pointer); self.last_exception=(pointer,size)

    def logic_error_ctor(self):
        self.last_logic_error=self._bounded_cstring(self.x(1),2048).decode(errors='replace')
        self.u.mem_write(self.x(0),bytes(64))

    def checked_strncpy(self):
        count,destination=self.x(2),self.x(3)
        if count>4096 or count>destination:raise RuntimeError(f'__strncpy_chk bounds: {count}/{destination}')
        source=bytes(self.u.mem_read(self.x(1),count)).split(b'\0',1)[0]
        self.u.mem_write(self.x(0),source+b'\0'*max(0,count-len(source)))
        self.x(0,self.x(0))

    def native_throw(self):
        obj,ti,dtor=self.x(0),self.x(1),self.x(2)
        nameptr=self.get(ti+8,'Q')[0] if ti else 0
        name=self._bounded_cstring(nameptr,1024).decode(errors='replace') if nameptr else '<null>'
        raise RuntimeError(f'native C++ exception object={obj:#x}, type={name}, typeinfo={ti:#x}, dtor={dtor:#x}, allocation={getattr(self,"last_exception",None)}, message={getattr(self,"last_logic_error",None)!r}')

    def gl_strstr_real(self):
        haystack,needle=self.x(0),self.x(1)
        h,n=self._bounded_cstring(haystack),self._bounded_cstring(needle)
        offset=h.find(n)
        self.x(0,0 if offset<0 else haystack+offset)

    def gl_strcmp_real(self):
        left,right=self._bounded_cstring(self.x(0)),self._bounded_cstring(self.x(1))
        self.x(0,(left>right)-(left<right))

    def _call_gl(self, name, fn, *args):
        prior = self.mesa.f['glGetError']()
        result = fn(*args)
        error = self.mesa.f['glGetError']()
        row = {'call': name, 'args': [int(a) if isinstance(a, (int, bool)) else None for a in args],
               'prior_error': prior, 'error_after': error}
        self.gl_calls_real.append(row)
        if prior or error:
            self.gl_errors.append(row)
        return result

    def _bytes_arg(self, pointer, size):
        if not pointer or not size:
            return None, None
        raw = bytes(self.u.mem_read(pointer, size))
        return raw, C.create_string_buffer(raw)

    def gl_gen_textures(self):
        count, out = self.x(0), self.x(1)
        if not 0 < count <= 8: raise RuntimeError(f'GL texture count out of bounds: {count}')
        ids = (C.c_uint * count)()
        self._call_gl('glGenTextures', self.mesa.f['glGenTextures'], count, ids)
        for i, value in enumerate(ids):
            self.put(out + i * 4, 'I', value)
            self.texture_ids.append(value)

    def gl_bind_texture_real(self):
        self._call_gl('glBindTexture', self.mesa.f['glBindTexture'], self.x(0), self.x(1))

    def gl_gen_samplers(self):
        count,out=self.x(0),self.x(1)
        if not 0<count<=8:raise RuntimeError(f'GL sampler count out of bounds: {count}')
        ids=(C.c_uint*count)();self._call_gl('glGenSamplers',self.mesa.f['glGenSamplers'],count,ids)
        for i,value in enumerate(ids):self.put(out+4*i,'I',value);self.sampler_ids.append(value)
    def gl_bind_sampler_real(self):
        self._call_gl('glBindSampler',self.mesa.f['glBindSampler'],self.x(0),self.x(1))
    def gl_sampler_parameter_real(self):
        args=[self.x(i) for i in range(3)]
        self._call_gl('glSamplerParameteri',self.mesa.f['glSamplerParameteri'],*args)
        self.texture_parameter_calls.append({'api':'glSamplerParameteri','sampler':args[0],
                                             'pname':args[1],'param':args[2]})

    def gl_tex_parameter(self):
        args = [self.x(i) for i in range(3)]
        self._call_gl('glTexParameteri', self.mesa.f['glTexParameteri'], *args)
        self.texture_parameter_calls.append({'api': 'glTexParameteri', 'target': args[0], 'pname': args[1], 'param': args[2]})

    def gl_pixel_store_real(self):
        args = [self.x(0), self.x(1)]
        self._call_gl('glPixelStorei', self.mesa.f['glPixelStorei'], *args)

    def gl_tex_image_real(self):
        args = [self.x(i) for i in range(8)]
        ptr = self.get(self.u.reg_read(R.UC_ARM64_REG_SP), 'Q')[0]
        w, h, fmt = args[3], args[4], args[6]
        channels = {0x1903: 1, 0x1906: 1, 0x1908: 4, 0x80e1: 4}.get(fmt, 4)
        raw, keepalive = self._bytes_arg(ptr, w * h * channels)
        self._call_gl('glTexImage2D', self.mesa.f['glTexImage2D'], *args, C.cast(keepalive, C.c_void_p) if keepalive else None)
        self.texture_uploads_real.append({'kind': 'storage', 'dimensions': [w, h], 'format': fmt,
                                          'bytes': len(raw) if raw else 0,
                                          'sha256': hashlib.sha256(raw).hexdigest() if raw else None})

    def gl_tex_sub_image_real(self):
        base.TextureProbe.gl_tex_sub_image_2d(self)
        args = [self.x(i) for i in range(8)]
        ptr = self.get(self.u.reg_read(R.UC_ARM64_REG_SP), 'Q')[0]
        w, h, fmt = args[4], args[5], args[6]
        channels = {0x1903: 1, 0x1906: 1, 0x1908: 4, 0x80e1: 4}.get(fmt, 4)
        raw, keepalive = self._bytes_arg(ptr, w * h * channels)
        if raw is None: raise RuntimeError('native texture update supplied a null pixel pointer')
        # The base oracle first validates the native diagnostic allocation. The
        # alternate bytes are injected only at the host Mesa upload boundary.
        if self.poison_variant == 'alternate-diagnostic' and (w, h) == (144, 144):
            payload = bytearray(raw)
            for tile_x in (1, 2):
                for y in range(96, 144):
                    for x in range(tile_x * 48, (tile_x + 1) * 48):
                        offset = (y * 144 + x) * 4
                        payload[offset:offset + 4] = b'\x5a\xa5\x3c\xff'
            raw = bytes(payload)
            keepalive = C.create_string_buffer(raw)
        self._call_gl('glTexSubImage2D', self.mesa.f['glTexSubImage2D'], *args,
                      C.cast(keepalive, C.c_void_p))
        row = {'kind': 'upload', 'dimensions': [w, h], 'format': fmt, 'type': args[7],
               'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()}
        self.texture_uploads_real.append(row)
        if (w, h) == (144, 144) or (w, h) == (321, 228):
            pass
        else: raise RuntimeError(f'unexpected native texture upload dimensions {w}x{h}')

    def gl_get_error(self): self.x(0, self.mesa.f['glGetError']())

    def gl_gen_buffers(self):
        n, out = self.x(0), self.x(1)
        if not 0 < n <= 8: raise RuntimeError(f'GL buffer count out of bounds: {n}')
        ids = (C.c_uint * n)(); self._call_gl('glGenBuffers', self.mesa.f['glGenBuffers'], n, ids)
        for i, value in enumerate(ids): self.put(out + i * 4, 'I', value)
        self.actual_buffer_ids.extend(int(v) for v in ids)
        synthetic=list(range(self.synthetic_buffer_id,self.synthetic_buffer_id+n))
        self.graphics_buffer_ids.extend(synthetic)
        self.buffer_id_map.update({int(actual):fake for actual,fake in zip(ids,synthetic)})
        self.synthetic_buffer_id += n

    def gl_bind_buffer_real(self):
        target,actual=self.x(0),self.x(1); fake=self.buffer_id_map.get(actual,actual)
        self.graphics_record('glBindBuffer',[target,fake])
        self._call_gl('glBindBuffer',self.mesa.f['glBindBuffer'],target,actual)

    def gl_buffer_data_real(self):
        target,size,pointer,usage=[self.x(i) for i in range(4)]
        if size > 1_000_000 or (size and not pointer): raise RuntimeError(f'buffer upload outside bound: {size}')
        raw,keepalive=self._bytes_arg(pointer,size)
        self.graphics_buffer_uploads.append({'buffer_id':self.graphics_bound_array_buffer,
                                             'target':target,'size':size,'usage':usage,
                                             'data_hex':raw.hex() if raw else '',
                                             'sha256':hashlib.sha256(raw or b'').hexdigest()})
        self._call_gl('glBufferData',self.mesa.f['glBufferData'],target,size,
                      C.cast(keepalive,C.c_void_p) if keepalive else None,usage)

    def gl_buffer_sub_data_real(self):
        target,offset,size,pointer=[self.x(i) for i in range(4)]
        raw,keepalive=self._bytes_arg(pointer,size)
        self._call_gl('glBufferSubData',self.mesa.f['glBufferSubData'],target,offset,size,
                      C.cast(keepalive,C.c_void_p) if keepalive else None)

    def gl_enable_attrib(self):
        self.graphics_record('glEnableVertexAttribArray',[self.x(0)])
        self._call_gl('glEnableVertexAttribArray',self.mesa.f['glEnableVertexAttribArray'],self.x(0))
    def gl_disable_attrib(self):
        self.graphics_record('glDisableVertexAttribArray',[self.x(0)])
        self._call_gl('glDisableVertexAttribArray',self.mesa.f['glDisableVertexAttribArray'],self.x(0))
    def gl_vertex_attrib_pointer_real(self):
        args=[self.x(i) for i in range(5)]; offset=self.x(5)
        self.graphics_attributes[args[0]]={'index':args[0],'size':args[1],'type':args[2],
            'normalized':args[3],'stride':args[4],'offset':offset,
            'buffer_id':self.graphics_bound_array_buffer}
        self.graphics_record('glVertexAttribPointer',[*args,offset])
        self._call_gl('glVertexAttribPointer',self.mesa.f['glVertexAttribPointer'],*args,C.c_void_p(offset))
    def gl_vertex_attrib_divisor_real(self):
        args=[self.x(0),self.x(1)]; self.graphics_record('glVertexAttribDivisor',args)
        self._call_gl('glVertexAttribDivisor',self.mesa.f['glVertexAttribDivisor'],*args)
    def gl_draw_arrays_instanced_real(self):
        args=[self.x(i) for i in range(4)]; self.graphics_draw_calls.append(args)
        self._call_gl('glDrawArraysInstanced',self.mesa.f['glDrawArraysInstanced'],*args)

    def gl_get_integer_real(self):
        pname, out = self.x(0), self.x(1); value=C.c_int()
        self._call_gl('glGetIntegerv',self.mesa.f['glGetIntegerv'],pname,C.byref(value))
        self.put(out,'I',value.value)
    def gl_get_string_real(self):
        enum=self.x(0); value=self.gl_get_string_ptr(enum)
        text=C.string_at(value) if value else b''
        pointer=self.alloc(len(text)+1); self.u.mem_write(pointer,text+b'\0'); self.x(0,pointer)
    def gl_active_texture(self): self._call_gl('glActiveTexture',self.mesa.f['glActiveTexture'],self.x(0))
    def gl_enable_real(self):
        cap=self.x(0)
        if cap==0x0BE2:self.blend_gl_calls.append({'call':'glEnable','args':[cap]})
        self._call_gl('glEnable',self.mesa.f['glEnable'],cap)
    def gl_disable_real(self): self._call_gl('glDisable',self.mesa.f['glDisable'],self.x(0))
    def gl_blend_equation_real(self):
        args=[self.x(0),self.x(1)]; self.blend_gl_calls.append({'call':'glBlendEquationSeparate','args':args})
        self._call_gl('glBlendEquationSeparate',self.mesa.f['glBlendEquationSeparate'],*args)
    def gl_blend_func_real(self):
        args=[self.x(i) for i in range(4)]; self.blend_gl_calls.append({'call':'glBlendFuncSeparate','args':args})
        self._call_gl('glBlendFuncSeparate',self.mesa.f['glBlendFuncSeparate'],*args)
    def gl_color_mask_real(self):
        args=[bool(self.x(i)) for i in range(4)]; self.blend_gl_calls.append({'call':'glColorMask','args':[int(v) for v in args]})
        self._call_gl('glColorMask',self.mesa.gl.glColorMask,*args)

    def new_draw_stub(self, kind, **fields):
        if kind == 'blend_activate': return self.syms[BLEND_ACTIVATE]
        return super().new_draw_stub(kind, **fields)

    def call(self,name,x=(),f=(),instruction_limit=2_000_000):
        if name=='SPen::BrushPenStrokeDrawableRTV1::SetStartPoint(float, float)':
            f=self.synthetic_start
        if name=='SPen::BrushPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)' and len(x)>=3:
            self.put(x[2],'16f',*self.synthetic_caller_matrix)
        return super().call(name,x,f,instruction_limit)

    def draw_bitmap_callback(self,kind):
        if kind=='bitmap_matrix':
            ptr=self.x(8); self.put(ptr,'16f',*self.synthetic_bitmap_matrix)
            self.x(0,ptr); self.draw_capture['events'].append({'call':kind,'x8_output':True})
            return
        super().draw_bitmap_callback(kind)

    def capture_native_blend_mapping(self):
        self.native_draw_blend_calls=list(self.blend_gl_calls)
        self.blend_gl_calls.clear()
        return super().capture_native_blend_mapping()

    def dispatch_draw_stub(self, address):
        stub=self.draw_stubs[address]; kind=stub['kind']
        if kind == 'program_activate':
            self._call_gl('glUseProgram',self.mesa.f['glUseProgram'],self.program)
        elif kind == 'binding_set':
            name=stub['name']; raw=bytes(self.u.mem_read(self.x(1),stub['bytes']))
            loc=self.uniform_locations[name]
            if name == 'ProjectionMatrix':
                values=struct.unpack('<16f',raw); values_c=(C.c_float*16)(*values)
                self._call_gl('glUniformMatrix4fv',self.mesa.f['glUniformMatrix4fv'],loc,1,0,values_c)
                self.uniform_values[name]=list(values)
            elif name in ('grain','grainOffset'):
                values=struct.unpack('<2f',raw)
                self._call_gl('glUniform2f',self.mesa.f['glUniform2f'],loc,*values)
                self.uniform_values[name]=list(values)
            elif name == 'inputColor':
                values=struct.unpack('<4f',raw)
                self._call_gl('glUniform4f',self.mesa.f['glUniform4f'],loc,*values)
                self.uniform_values[name]=list(values)
            elif name in ('sampler0','sampler1'):
                value=struct.unpack('<I',raw)[0]
                self._call_gl('glUniform1i',self.mesa.f['glUniform1i'],loc,value)
                self.uniform_values[name]=value
            elif name == 'numTiles':
                value=struct.unpack('<f',raw)[0]
                self._call_gl('glUniform1f',self.mesa.f['glUniform1f'],loc,value)
                self.uniform_values[name]=value
            else: raise RuntimeError(f'unhandled native uniform {name}')
        super().dispatch_draw_stub(address)

    def hook(self,u,address,size,data):
        if address in self.plt and self.plt[address] == FACTORY:
            sp=self.u.reg_read(R.UC_ARM64_REG_SP)
            descriptor,label_ptr,source,pairs,count,info=[self.x(i) for i in range(6)]
            words=list(self.get(descriptor,'8I'))
            pair_words=list(self.get(pairs,'I'*(count*2)))
            info_words=list(self.get(info,'4I'))
            label=self.cstr(label_ptr)
            target=self.get(base.RENDERER_BASE+0x313f4+words[5]*4,'I')[0]
            self.texture_descriptors.append({'resource':label,'descriptor_u32':words,
                'u32_pairs':pair_words,'pair_count':count,'buffer_info_u32':info_words,
                'initial_source_is_null':source==0,'texture_target_gl_enum':target})
            self.native_factory_calls.append({'x0_x5':[self.x(i) for i in range(6)],
                                              'stack32_hex':bytes(self.u.mem_read(sp,32)).hex()})
            u.reg_write(R.UC_ARM64_REG_PC,self.syms[FACTORY_IMPL]); return
        super().hook(u,address,size,data)

    def capture_native_draw(self, rt):
        # Preserve the descriptor emitted by native Init, but install a real
        # Renderer BlendStateObject for the actual Draw activation call.
        descriptor=self.alloc(32); self.u.mem_write(descriptor,bytes.fromhex(self.blend_descriptor))
        self.native_blend=self.alloc(64)
        self.call(BLEND_CTOR,[self.native_blend,descriptor])
        self.call(BLEND_SET,[self.native_blend,descriptor])
        self.put(rt+208,'Q',self.native_blend)

        geometry=json.loads(GEOMETRY.read_text())
        record_case=next(c for c in geometry['synthetic_cases'] if c['label']=='synthetic-motion-tool1-pressure-tilt-orientation')
        record=record_case['buffer_records'][1]['instance_bits_le']
        texture_id=struct.unpack('<f',bytes.fromhex(record[3]))[0]
        if math_floor(texture_id) != 1: raise RuntimeError(f'instance must select initialized atlas tile 1: {texture_id}')

        shader_resources=geometry['shader_resources']
        vertex,fragment=shader_resources['RTV1_vertex_source'],shader_resources['RTV1_fragment_source']
        self.program=self.mesa.program(vertex,fragment)
        self.shader_hashes={'vertex':hashlib.sha256(vertex.encode()).hexdigest(),
                            'fragment':hashlib.sha256(fragment.encode()).hexdigest()}
        self.uniform_locations={name:self.mesa.f['glGetUniformLocation'](self.program,name.encode())
                                for name in ('ProjectionMatrix','sampler0','sampler1','inputColor','grain','grainOffset','numTiles')}
        if any(value < 0 for value in self.uniform_locations.values()): raise RuntimeError(f'missing shader uniforms: {self.uniform_locations}')
        if len(self.texture_ids)!=2: raise RuntimeError(f'expected two actual Renderer textures, got {self.texture_ids}')
        if self.output_texture==0:
            self.output_texture,self.output_fbo=create_output_fbo(self.mesa)
            setup_error=self.mesa.f['glGetError']()
            if setup_error: raise RuntimeError(f'host output FBO setup GL error: {setup_error:#x}')
        if self.output_texture in self.texture_ids: raise RuntimeError('feedback alias: output texture aliases shader input')
        self.mesa.f['glBindFramebuffer'](0x8D40,self.output_fbo)
        self.mesa.f['glViewport'](0,0,321,228)
        self.mesa.f['glDisable'](0x0BE2)
        self.mesa.f['glClearColor'](0,0,0,0); self.mesa.f['glClear'](0x4000)
        self.mesa.f['glActiveTexture'](0x84C0); self.mesa.f['glBindTexture'](0x0DE1,self.texture_ids[0])
        self.mesa.f['glActiveTexture'](0x84C1); self.mesa.f['glBindTexture'](0x0DE1,self.texture_ids[1])
        status=self.mesa.f['glCheckFramebufferStatus'](0x8D40)
        if status!=0x8CD5: raise RuntimeError(f'output FBO incomplete before native draw: {status:#x}')
        self.output_fbo_status=status
        result=super().capture_native_draw(rt)
        self.draw_capture['bitmap_matrix_f32']=self.synthetic_bitmap_matrix
        self.draw_capture['caller_matrix_f32']=self.synthetic_caller_matrix
        self.draw_capture['set_start_point_f32']=list(self.synthetic_start)
        bound=C.c_int(); self.mesa.f['glGetIntegerv'](0x8CA6,C.byref(bound))
        if bound.value!=self.output_fbo: raise RuntimeError(f'native Draw changed destination FBO to {bound.value}')
        pixels=C.create_string_buffer(321*228*4)
        self.mesa.f['glPixelStorei'](0x0D05,1)
        self.mesa.f['glReadPixels'](0,0,321,228,0x1908,0x1401,pixels)
        error=self.mesa.f['glGetError']()
        self.readback=pixels.raw
        if error: self.gl_errors.append({'call':'glReadPixels','error_after':error})
        if self.gl_errors: raise RuntimeError(f'Mesa GLES errors during native Draw: {self.gl_errors[:8]}')
        return result

    def run(self):
        result=super().run()
        if self.gl_errors: raise RuntimeError(f'host GLES errors in native paint run: {self.gl_errors[:8]}')
        geometry=json.loads(GEOMETRY.read_text())
        texture_fixture=json.loads(TEXTURE_FIXTURE.read_text())
        if len(result['uploads'])!=len(texture_fixture['uploads']):
            raise RuntimeError('native upload count differs from pinned BrushPen texture fixture')
        for observed,expected in zip(result['uploads'],texture_fixture['uploads']):
            for key in ('call','read_bytes','allocation_bytes','upload_sha256',
                        'tile_sha256_3x3','diagnostic_sentinel_pixels_3x3'):
                if key in expected and observed.get(key)!=expected[key]:
                    raise RuntimeError(f'native texture upload differs from pinned fixture at {key}')
        case=next(c for c in geometry['synthetic_cases'] if c['label']=='synthetic-motion-tool1-pressure-tilt-orientation')
        instance=case['buffer_records'][1]['instance_bits_le']
        alpha=self.readback[3::4]
        return {
            'schema':'brushpen-native-draw-mesa-v1',
            'scope':'Actual fresh V1 constructor, Init, texture/graphics native methods, AddPoint-derived fixture instance upload, RTV1 Draw, GraphicsObject::DrawInstanced, actual shader and native BlendStateObject execute. Explicit synthetic motion inputs, host queue/bitmap/binding adapters and host Mesa context. Not Samsung-device capture.',
            'libraries':result['library_sha256'],
            'mesa':self.mesa.identity,
            'shader_source':geometry['shader_resources'],
            'shader_hashes':self.shader_hashes,
            'texture_uploads':self.texture_uploads_real,
            'pattern_valid_initialized_tile_hashes':result['uploads'][0]['tile_sha256_3x3'],
            'pattern_poison_cells':result['uploads'][0]['diagnostic_sentinel_pixels_3x3'],
            'texture_ids':self.texture_ids,
            'sampler_ids':self.sampler_ids,
            'texture_state_calls':self.texture_parameter_calls,
            'native_gl_texture_bindings':[row for row in self.gl_calls_real
                if row['call'] in ('glActiveTexture','glBindTexture','glBindSampler')],
            'geometry_case':case['label'],
            'synthetic_input':case['synthetic_input'],
            'synthetic_draw_inputs':{
                'set_start_point_f32':self.draw_capture.get('set_start_point_f32'),
                'caller_matrix_f32':self.draw_capture.get('caller_matrix_f32'),
                'bitmap_matrix_f32':self.draw_capture.get('bitmap_matrix_f32'),
                'bitmap_size':self.draw_capture.get('bitmap_size'),
                'input_color_f32':self.uniform_values.get('inputColor'),
            },
            'fresh_native_pen_size_f32':case['admission']['actual_GetSize_f32'],
            'instance_record_index':1,
            'instance_bits_le':instance,
            'instance_bytes_hex':''.join(instance),
            'graphics_draw_calls':self.graphics_draw_calls,
            'native_graphics_execution':{
                'method_entries':result['native_graphics_execution']['native_method_entries'],
                'attribute_layout':result['native_graphics_execution']['attribute_layout'],
                'instance_upload':result['native_graphics_execution']['fixture_link'],
            },
            'draw_uniforms':self.uniform_values,
            'native_blend_descriptor':self.blend_descriptor,
            'native_draw_blend_calls':getattr(self,'native_draw_blend_calls',[]),
            'native_blend_mapping_probe_calls':self.blend_gl_calls,
            'output':{'dimensions':[321,228],'format':'RGBA8','texture_id':self.output_texture,
                      'framebuffer_id':self.output_fbo,'complete':True,
                      'framebuffer_status_gl_enum':hex(self.output_fbo_status),
                      'input_texture_ids':self.texture_ids,'feedback_alias':False,
                      'rgba_sha256':hashlib.sha256(self.readback).hexdigest(),
                      'rgba_base64':base64.b64encode(self.readback).decode('ascii'),
                      'bytes':len(self.readback),'pixel_count':321*228,
                      'nonzero_alpha_pixels':sum(a!=0 for a in alpha),
                      'alpha_range':[min(alpha),max(alpha)]},
            'gl_call_count':len(self.gl_calls_real),'gl_errors':self.gl_errors,
            'native_texture_factory_calls':self.native_factory_calls,
            'provenance':{
                'geometry_fixture_sha256':hashlib.sha256(GEOMETRY.read_bytes()).hexdigest(),
                'texture_fixture_sha256':hashlib.sha256(TEXTURE_FIXTURE.read_bytes()).hexdigest(),
                'native_elf_sha256':result['library_sha256'],
                'host_gles_dsos':{
                    'EGL':loaded_host_dso('libEGL.so'),
                    'GLESv2':loaded_host_dso('libGLESv2.so'),
                },
                'native_shader_source_sha256':self.shader_hashes,
                'native_embedded_grain_source':texture_fixture['embedded_grain_source'],
                'texture_profile':texture_fixture['constructor_profile'],
                'selected_initialized_atlas_tile':{'tile_index':1,'grid_xy':[1,0],
                    'pixel_bounds_xyxy':[48,0,96,48],
                    'sha256':result['uploads'][0]['tile_sha256_3x3'][0][1]},
                'poison_input_variant':self.poison_variant,
                'mesa_host_boundaries':[
                    'host queue, bitmap methods, shader binding dispatch, EGL context and output FBO',
                    'native GLES calls are intercepted and executed against host Mesa llvmpipe'],
            },
            'limitations':['texture pattern atlas contains two diagnostic poison cells inherited from the native allocation boundary; selected instance uses tile 1, which is initialized','host Mesa is not the Samsung driver; outputs are host render evidence, not device parity'],
        }

def math_floor(value):
    import math
    return math.floor(value)

def loaded_host_dso(fragment):
    for line in Path('/proc/self/maps').read_text().splitlines():
        fields=line.split()
        if len(fields)>=6 and fragment in fields[-1] and fields[-1].startswith('/'):
            path=Path(os.path.realpath(fields[-1]))
            return {'path':str(path),'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}
    raise RuntimeError(f'cannot identify loaded host DSO containing {fragment}')

def create_output_fbo(mesa):
    texture=C.c_uint(); fbo=C.c_uint(); f=mesa.f
    f['glGenTextures'](1,C.byref(texture)); f['glBindTexture'](0x0DE1,texture.value)
    f['glTexParameteri'](0x0DE1,0x2801,0x2600); f['glTexParameteri'](0x0DE1,0x2800,0x2600)
    f['glTexParameteri'](0x0DE1,0x2802,0x812F); f['glTexParameteri'](0x0DE1,0x2803,0x812F)
    f['glTexImage2D'](0x0DE1,0,0x8058,321,228,0,0x1908,0x1401,None)
    f['glGenFramebuffers'](1,C.byref(fbo)); f['glBindFramebuffer'](0x8D40,fbo.value)
    f['glFramebufferTexture2D'](0x8D40,0x8CE0,0x0DE1,texture.value,0)
    status=f['glCheckFramebufferStatus'](0x8D40)
    if status!=0x8CD5: raise RuntimeError(f'RGBA8 FBO incomplete: {status:#x}')
    return texture.value,fbo.value

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    modes=parser.add_mutually_exclusive_group(required=True)
    modes.add_argument('--write-fixture',action='store_true')
    modes.add_argument('--check-fixture',action='store_true')
    args=parser.parse_args()
    baseline=PositiveProbe('native-diagnostic')
    report=baseline.run()
    alternate=PositiveProbe('alternate-diagnostic')
    alternate_report=alternate.run()
    if baseline.readback != alternate.readback:
        raise AssertionError('output changed when only unused diagnostic atlas cells changed')
    base_upload=[row for row in report['texture_uploads'] if row['kind']=='upload']
    alt_upload=[row for row in alternate_report['texture_uploads'] if row['kind']=='upload']
    if base_upload[0]['sha256']==alt_upload[0]['sha256']:
        raise AssertionError('poison sensitivity control did not alter atlas payload')
    if base_upload[1]['sha256']!=alt_upload[1]['sha256']:
        raise AssertionError('poison control changed the independent grain texture')
    if report['output']['rgba_sha256']!=alternate_report['output']['rgba_sha256']:
        raise AssertionError('poison control output hash mismatch')
    report['poison_sensitivity_control']={
        'changed_cells_grid_xy':[[1,2],[2,2]],
        'native_diagnostic_rgba_hex':'cdcdcdcd',
        'alternate_host_only_rgba_hex':'5aa53cff',
        'baseline_pattern_upload_sha256':base_upload[0]['sha256'],
        'alternate_pattern_upload_sha256':alt_upload[0]['sha256'],
        'selected_tile1_sha256_baseline':report['pattern_valid_initialized_tile_hashes'][0][1],
        'selected_tile1_sha256_alternate':alternate_report['pattern_valid_initialized_tile_hashes'][0][1],
        'grain_upload_sha256_unchanged':base_upload[1]['sha256'],
        'output_sha256_baseline':report['output']['rgba_sha256'],
        'output_sha256_alternate':alternate_report['output']['rgba_sha256'],
        'output_unchanged':True,
        'interpretation':'host diagnostic-allocation sensitivity check only; does not model or assert production heap contents',
    }
    out=RUN/'brushpen-paint-positive-native.json'
    encoded=json.dumps(report,indent=2,sort_keys=True)+'\n'
    if args.write_fixture:
        out.write_text(encoded)
        print(f'wrote {out}')
    else:
        if not out.exists(): raise SystemExit(f'missing fixture {out}; review output before --write-fixture')
        if json.dumps(json.loads(out.read_text()),sort_keys=True)!=json.dumps(report,sort_keys=True):
            raise AssertionError('native BrushPen positive paint differs from reviewed fixture')
        print('BrushPen positive paint oracle passed (native Draw on host Mesa; poison control stable)')

if __name__=='__main__': main()
