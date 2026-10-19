#!/usr/bin/env python3
"""Execute AirBrush V2 quad/instance uploads, instanced Draw, and blend state."""
import argparse
import hashlib
import json
import struct
from pathlib import Path

import airbrush_texture_native as texture
from unicorn import arm64_const as R

ROOT = Path(__file__).resolve().parents[1]
LIB = ROOT / 'scratch/apk-analysis-native/arm64-v8a'
SAVED = ROOT / 'conformance/airbrush-saved-native.json'
TEXTURE = ROOT / 'conformance/airbrush-texture-native.json'
OUT = ROOT / 'conformance/airbrush-draw-native.json'
DRAW = 'SPen::AirBrushPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)'
CTOR = 'SPen::AirBrushPenStrokeDrawableGLV2::AirBrushPenStrokeDrawableGLV2(SPen::AirBrushPenData*, SPen::PenGLDataManager*)'
UPDATE = 'SPen::GLES::GraphicsObject::UpdateBuffer(unsigned int, void const*, unsigned int)'
DRAW_INSTANCED = 'SPen::GLES::GraphicsObject::DrawInstanced(unsigned int, int, int) const'
BLEND_CTOR = 'SPen::GLES::BlendStateObject::BlendStateObject(SPen::BlendStateDescriptor const&)'
BLEND_ACTIVATE = 'SPen::GLES::BlendStateObject::Activate()'
ATTRIBUTE = 'SPen::VertexDescriptor::addAttribute(SPen::VertexDescriptor::AttributeType, int, int, int)'
CREATE_GRAPHICS = ('SPen::RendererFactory::CreateGraphicsObject(unsigned int, SPen::PrimitiveType, '
                   'SPen::VertexDescriptor const*, unsigned int, unsigned int)')
CREATE_BLEND = 'SPen::RendererFactory::CreateBlendStateObject(SPen::BlendStateDescriptor const&)'
SYNTHETIC_COLOR = [0.25, 0.5, 0.75, 0.8]
SYNTHETIC_COLOR_F32 = list(struct.unpack('<4f', struct.pack('<4f', *SYNTHETIC_COLOR)))


class DrawOracle(texture.TextureOracle):
    def __init__(self):
        super().__init__()
        self.actual_entries = []
        self.vertex_attributes = []
        self.graphics_factory = []
        self.blend_descriptors = []
        self.draw_gl = []
        self.draw_blend_gl = []
        self.synthetic = {}
        self.synthetic_calls = []
        self.shader_values = []
        self.pending_returns_draw = []
        self.renderer_base = self.extra_bases['Renderer']
        self.gl_id = 41
        self.queue_bitmap = None
        self.drawable_obj = None
        self.rt = None
        self.graphics_obj = None
        self.draw_return_tramp = self.alloc(8)
        self.u.mem_write(self.draw_return_tramp, struct.pack('<II', 0xd503201f, 0xd65f03c0))
        self.callbacks[CREATE_GRAPHICS] = self.create_graphics
        self.callbacks[CREATE_BLEND] = self.create_blend
        self.callbacks.update({
            'glBlendEquationSeparate': lambda: self.draw_blend_gl.append(
                {'name': 'glBlendEquationSeparate', 'args': [self.x(0), self.x(1)]}),
            'glBlendFuncSeparate': lambda: self.draw_blend_gl.append(
                {'name': 'glBlendFuncSeparate', 'args': [self.x(i) for i in range(4)]}),
            'glEnable': lambda: self.draw_blend_gl.append({'name': 'glEnable', 'args': [self.x(0)]}),
            'glDisable': lambda: self.draw_blend_gl.append({'name': 'glDisable', 'args': [self.x(0)]}),
            'glColorMask': lambda: self.draw_blend_gl.append(
                {'name': 'glColorMask', 'args': [self.x(i) for i in range(4)]}),
            'glDrawArraysInstanced': lambda: self.draw_gl.append(
                {'name': 'glDrawArraysInstanced', 'mode': self.x(0), 'first': self.x(1),
                 'count': self.x(2), 'instances': self.x(3)}),
            'glBindBuffer': lambda: self.draw_gl.append(
                {'name': 'glBindBuffer', 'target': self.x(0), 'id': self.x(1)}),
            'glBufferData': self.buffer_data,
            'glDisableVertexAttribArray': lambda: self.draw_gl.append(
                {'name': 'glDisableVertexAttribArray', 'index': self.x(0)}),
            'glEnableVertexAttribArray': lambda: self.draw_gl.append(
                {'name': 'glEnableVertexAttribArray', 'index': self.x(0)}),
            'glVertexAttribPointer': self.vertex_attrib_pointer,
            'glVertexAttribDivisor': lambda: self.draw_gl.append(
                {'name': 'glVertexAttribDivisor', 'index': self.x(0), 'divisor': self.x(1)}),
            'glBindVertexArray': lambda: self.draw_gl.append(
                {'name': 'glBindVertexArray', 'id': self.x(0)}),
        })

    def call(self, name, x=(), f=(), instruction_limit=2_000_000):
        try:
            super().call(name, x, f, instruction_limit)
        except Exception as error:
            print('AirBrush Draw call fault', name, repr(error),
                  'pc', hex(self.u.reg_read(R.UC_ARM64_REG_PC)),
                  'lr', hex(self.u.reg_read(R.UC_ARM64_REG_LR)),
                  'x0-x7', [hex(self.x(i)) for i in range(8)],
                  'rt', hex(self.rt or 0),
                  file=__import__('sys').stderr)
            raise
        if name == CTOR:
            self.drawable_obj = x[0]
            self.rt = self.get(self.drawable_obj + 16, 'Q')[0]

    def hook(self, u, address, size, data):
        if address in self.plt and self.plt[address] == CREATE_BLEND:
            self.create_blend()
            return
        for symbol in (CTOR, 'SPen::AirBrushPenStrokeDrawableRTV1::setRectData(float)',
                       DRAW, UPDATE, DRAW_INSTANCED, BLEND_CTOR, BLEND_ACTIVATE):
            if address == self.syms.get(symbol):
                self.actual_entries.append({'name': symbol, 'args': [self.x(i) for i in range(4)]})
        if address == self.syms.get(ATTRIBUTE):
            self.vertex_attributes.append({'descriptor': self.x(0), 'attribute_type': self.x(1),
                                           'components': self.x(2), 'stride': self.x(3),
                                           'divisor': self.x(4)})
        if address in self.synthetic:
            name = self.synthetic[address]
            record = {'name': name, 'args': [self.x(i) for i in range(6)]}
            self.synthetic_calls.append(record)
            if name == 'bitmap.get_projection':
                self.u.mem_write(self.x(8), struct.pack('<16f', 1,0,0,0, 0,1,0,0,
                                                        0,0,1,0, 0,0,0,1))
            elif name == 'bitmap.activate':
                pass
            elif name == 'bitmap.get_readable':
                self.x(0, self.readable)
            elif name == 'shader.ProjectionMatrix':
                self.shader_values.append({'name': name, 'matrix': list(
                    struct.unpack('<16f', bytes(self.u.mem_read(self.x(1), 64))))})
            elif name == 'shader.inputColor':
                self.shader_values.append({'name': name, 'color': list(
                    struct.unpack('<4f', bytes(self.u.mem_read(self.x(1), 16))))})
            elif name == 'shader.sampler0':
                self.shader_values.append({'name': name, 'unit': self.get(self.x(1), 'I')[0]})
            self.u.reg_write(R.UC_ARM64_REG_PC, self.u.reg_read(R.UC_ARM64_REG_LR))
            return
        if address == self.draw_return_tramp:
            if not self.pending_returns_draw:
                raise RuntimeError('unexpected draw return trampoline')
            self.u.reg_write(R.UC_ARM64_REG_PC, self.pending_returns_draw.pop())
            return
        super().hook(u, address, size, data)

    def create_graphics(self):
        args = [self.x(i) for i in range(5)]
        desc = args[2]
        raw = bytes(self.u.mem_read(desc, 0x130))
        obj = self.alloc(0xa40)
        self.put(obj, 'Q', self.renderer_base + 0x6a220)
        self.u.mem_write(obj + 0x68, raw)
        self.put(obj + 36, 'II', 41, 42)
        self.put(obj + 2540, 'I', 2)
        self.put(obj + 2612, 'I', 5)
        self.graphics_obj = obj
        self.graphics_factory.append({'descriptor_bytes': len(raw),
                                      'descriptor_sha256': hashlib.sha256(raw).hexdigest(),
                                      'descriptor_hex': raw.hex(),
                                      'attributes': list(self.vertex_attributes),
                                      'return_object_synthetic': True,
                                      'object_size': 0xa40})
        self.x(0, obj)

    def create_blend(self):
        desc = self.x(0)
        raw = bytes(self.u.mem_read(desc, 32))
        self.blend_descriptors.append(raw.hex())
        obj = self.alloc(48)
        self.pending_returns_draw.append(self.u.reg_read(R.UC_ARM64_REG_LR))
        self.u.reg_write(R.UC_ARM64_REG_LR, self.draw_return_tramp)
        self.x(0, obj)
        self.x(1, desc)
        self.u.reg_write(R.UC_ARM64_REG_PC, self.syms[BLEND_CTOR])

    def buffer_data(self):
        target, size, pointer, usage = [self.x(i) for i in range(4)]
        if size > 1_000_000:
            raise RuntimeError(f'buffer allocation exceeds witness bound: {size}')
        raw = bytes(self.u.mem_read(pointer, size)) if pointer and size else b''
        self.draw_gl.append({'name': 'glBufferData', 'target': target, 'bytes': size,
                             'data_sha256': hashlib.sha256(raw).hexdigest() if raw else None,
                             'data_hex': raw.hex() if size <= 64 else None, 'usage': usage})

    def vertex_attrib_pointer(self):
        self.draw_gl.append({'name': 'glVertexAttribPointer', 'index': self.x(0),
                             'size': self.x(1), 'type': self.x(2), 'normalized': self.x(3),
                             'stride': self.x(4), 'pointer': self.x(5)})

    def synthetic_method(self, name):
        ptr = self.alloc(16)
        self.synthetic[ptr] = name
        return ptr

    def draw_saved_stream(self, case):
        trampoline = struct.pack('<II', 0xd503201f, 0xd65f03c0)
        if bytes(self.u.mem_read(self.draw_return_tramp, 8)) != trampoline:
            raise RuntimeError('draw trampoline changed before Draw execution')
        if self.rt is None or self.graphics_obj is None:
            raise RuntimeError('Init did not produce the expected native RT/graphics object')
        self.call('SPen::AirBrushPenStrokeDrawableRTV1::setRectData(float)', [self.rt], [10.0])
        rows = case['native']['stamps']
        stream = b''.join(struct.pack('<I', int(word, 16)) for row in rows for word in row)
        stream_hash = hashlib.sha256(stream).hexdigest()
        if stream_hash != case['native']['stamp_stream_sha256']:
            raise RuntimeError('fixture transform bytes do not match the saved native stream digest')
        ptr = self.alloc(len(stream))
        self.u.mem_write(ptr, stream)
        self.call(UPDATE, [self.graphics_obj, 1, ptr, len(rows)], instruction_limit=2_000_000)
        self.put(self.rt + 72, 'I', len(rows))
        vector = self.alloc(24)
        self.put(vector, 'QQQ', ptr, ptr + len(stream), ptr + len(stream))
        self.put(self.rt + 88, 'Q', vector)

        bitmap, bvt = self.alloc(224), self.alloc(256)
        self.put(bitmap, 'Q', bvt)
        self.put(bvt + 16, 'Q', self.synthetic_method('bitmap.get_projection'))
        self.put(bvt + 40, 'Q', self.synthetic_method('bitmap.activate'))
        self.put(bvt + 48, 'Q', self.synthetic_method('bitmap.deactivate'))
        pattern, pvt = self.alloc(224), self.alloc(256)
        self.put(pattern, 'Q', pvt)
        readable, rv = self.alloc(16), self.alloc(64)
        self.readable = readable
        self.put(readable, 'Q', rv)
        self.put(rv + 16, 'Q', self.synthetic_method('pattern.activate'))
        self.put(pvt + 144, 'Q', self.synthetic_method('bitmap.get_readable'))
        self.put(self.rt + 224, 'Q', pattern)
        shader_wrapper, shader, sv = self.alloc(64), self.alloc(32), self.alloc(128)
        self.put(shader, 'Q', sv)
        self.put(sv + 16, 'Q', self.synthetic_method('shader.activate'))
        self.put(shader_wrapper + 24, 'Q', shader)
        self.put(self.rt + 208, 'Q', shader_wrapper)
        for offset, label in ((0, 'shader.ProjectionMatrix'), (8, 'shader.sampler0'),
                              (16, 'shader.inputColor')):
            obj, vt = self.alloc(16), self.alloc(32)
            self.put(obj, 'Q', vt)
            self.put(vt, 'Q', self.synthetic_method(label))
            self.put(shader_wrapper + offset, 'Q', obj)
        self.put(self.rt + 44, '4f', *SYNTHETIC_COLOR)
        caller = self.alloc(64)
        self.put(caller, '16f', 1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1)
        self.call(DRAW, [self.rt, bitmap, caller], instruction_limit=2_000_000)
        if bytes(self.u.mem_read(self.draw_return_tramp, 8)) != trampoline:
            raise RuntimeError('draw trampoline changed during native Draw')
        if self.pending_returns_draw:
            raise RuntimeError('native Draw return stack not empty')
        return stream, ptr

    def draw_result(self, init_result, saved_case, saved_v2, stream, quad):
        pointers = [x for x in self.draw_gl if x['name'] == 'glVertexAttribPointer']
        divisors = [x for x in self.draw_gl if x['name'] == 'glVertexAttribDivisor']
        draws = [x for x in self.draw_gl if x['name'] == 'glDrawArraysInstanced']
        updates = [x for x in self.actual_entries if x['name'] == UPDATE]
        quad_upload = next(x for x in self.draw_gl if x['name'] == 'glBufferData' and x['bytes'] == 64)
        instance_upload = next(x for x in self.draw_gl if x['name'] == 'glBufferData' and x['bytes'] == len(stream))
        expected_quad = struct.pack('<16f', -5,-5,0,1, -5,5,0,0, 5,-5,1,1, 5,5,1,0)
        if bytes.fromhex(quad_upload['data_hex']) != expected_quad:
            raise RuntimeError('native setRectData(10) quad upload differs from pinned XYUV geometry')
        if instance_upload['data_sha256'] != hashlib.sha256(stream).hexdigest():
            raise RuntimeError('native Renderer instance upload digest differs from the saved stream')
        if [(x['args'][1], x['args'][3]) for x in updates] != [(0, 4), (1, saved_case['native']['stamp_count'])]:
            raise RuntimeError('Renderer UpdateBuffer index/count mapping changed')
        if [(x['index'], x['size'], x['type'], x['normalized'], x['stride'], x['pointer'])
                for x in pointers] != [(0,4,0x1406,0,16,0),(1,4,0x1406,0,16,0)]:
            raise RuntimeError(f'Renderer attribute pointers changed: {pointers}')
        if [(x['index'],x['divisor']) for x in divisors[:2]] != [(0,0),(1,1)]:
            raise RuntimeError(f'instance divisors changed: {divisors}')
        if len(draws) != 1 or [draws[0][k] for k in ('mode','first','count','instances')] != [5,0,4,saved_case['native']['stamp_count']]:
            raise RuntimeError(f'instanced draw changed: {draws}')
        expected_blend = [
            {'name':'glBlendEquationSeparate','args':[0x8006,0x8006]},
            {'name':'glBlendFuncSeparate','args':[0x0302,0x0303,1,0x0303]},
            {'name':'glEnable','args':[0x0be2]},
            {'name':'glColorMask','args':[1,1,1,1]},
        ]
        if self.draw_blend_gl != expected_blend:
            raise RuntimeError(f'native blend state changed: {self.draw_blend_gl}')
        sampler = [{'pname': x['pname'], 'value': x['value']}
                   for x in init_result['texture']['sampler_calls']]
        native_entries = [x['name'] for x in self.actual_entries]
        counts = {n: native_entries.count(n) for n in (CTOR,
                  'SPen::AirBrushPenStrokeDrawableRTV1::setRectData(float)', DRAW,
                  UPDATE, DRAW_INSTANCED, BLEND_CTOR, BLEND_ACTIVATE)}
        expected_counts = {CTOR:1, 'SPen::AirBrushPenStrokeDrawableRTV1::setRectData(float)':1,
                           DRAW:1, UPDATE:2, DRAW_INSTANCED:1, BLEND_CTOR:1, BLEND_ACTIVATE:1}
        if counts != expected_counts:
            raise RuntimeError(f'native Draw method counts differ: {counts}')
        if self.shader_values != [
                {'name':'shader.ProjectionMatrix','matrix':[1.0,0.0,0.0,0.0,0.0,1.0,0.0,0.0,
                                                            0.0,0.0,1.0,0.0,0.0,0.0,0.0,1.0]},
                {'name':'shader.inputColor','color':SYNTHETIC_COLOR_F32},
                {'name':'shader.sampler0','unit':0}]:
            raise RuntimeError(f'native Draw uniform calls changed: {self.shader_values}')
        dso_hashes = texture.EXPECTED
        return {
            'schema':'sdocx.airbrush-draw-native.v1',
            'source':{'application':'Samsung Notes 4.4.45.37','native_libraries_sha256':dso_hashes,
                      'shader_sha256':init_result['texture']['shader_sha256'],
                      'cpu_pattern_sha256':init_result['texture']['cpu_pattern_sha256']},
            'runtime':{'profile':'default AirBrush V2', 'constructor':CTOR,
                       'native_method_counts':counts,
                       'synthetic_draw_context':{'input_color':SYNTHETIC_COLOR,
                                                 'projection':'identity matrix',
                                                 'sampler_unit':0,
                                                 'input_bitmap':'synthetic SPSubBitmap projection/activation vtable'},
                       'saved_native_redraw':{'case_id':saved_v2['input_id'],
                                              'profile_version':saved_v2['runtime_context']['drawable_profile_version'],
                                              'stamp_count':saved_v2['native']['stamp_count'],
                                              'stamp_stream_sha256':saved_v2['native']['stamp_stream_sha256'],
                                              'fixture_storage':'V2 records were deduplicated against V1; the full captured V1 array is used below after exact hash equality.'},
                       'draw_input_boundary':'The full V1 record array is injected into actual GraphicsObject::UpdateBuffer(index 1) after its SHA-256 is checked against the separately captured default V2 saved-redraw stream. This Draw witness does not itself replay MotionEvent history or invoke saved RedrawPen.',
                       'host_gles_callbacks':'all captured GL calls are controlled host callbacks; no native GLES driver or framebuffer.'},
            'vertex_data':{'quad':{'buffer_index':0,'vertex_count':4,'bytes':64,
                                  'sha256':quad['sha256'],'vertices_xyuv':quad['vertices']},
                           'instances':{'buffer_index':1,'instance_count':saved_case['native']['stamp_count'],
                                        'bytes':len(stream),'sha256':hashlib.sha256(stream).hexdigest(),
                                        'equals_saved_native_stream':True}},
            'vertex_input':{'attributes':[
                {'index':x['index'],'components':x['size'],'type':x['type'],
                 'normalized':x['normalized'],'stride_bytes':x['stride'],'byte_offset':x['pointer'],
                 'divisor':divisors[x['index']]['divisor'],
                 'data':'quad buffer index 0' if x['index']==0 else 'stamp buffer index 1'}
                for x in pointers],
                'draw':{'primitive':'TRIANGLE_STRIP','gl_enum':draws[0]['mode'],
                        'first':draws[0]['first'],'vertex_count':draws[0]['count'],
                        'instance_count':draws[0]['instances']}},
            'fragment':{'expression':'src.rgb = inputColor.rgb; src.a = inputColor.a * texture(sampler0, vUV).r',
                        'input_color':SYNTHETIC_COLOR,'sampler_unit':0,
                        'sampler_contract':init_result['texture']['final_sampler_contract'],
                        'pattern_sha256':init_result['texture']['cpu_pattern_sha256'],
                        'uniform_calls':self.shader_values},
            'blend':{'descriptor_hex':self.blend_descriptors[-1],
                     'gl_calls':self.draw_blend_gl,
                     'decoded':{'equation':'FUNC_ADD','rgb_source':'SRC_ALPHA',
                                'rgb_destination':'ONE_MINUS_SRC_ALPHA','alpha_source':'ONE',
                                'alpha_destination':'ONE_MINUS_SRC_ALPHA','enabled':True,
                                'color_write':[True,True,True,True]},
                     'equation':'RGBout=RGBsrc*As+RGBdst*(1-As); Aout=As+Ad*(1-As)'},
            'execution_boundary':{'actual':'Pinned AirBrush V2 constructor, RTV1 setRectData/Draw, Renderer GraphicsObject UpdateBuffer/DrawInstanced and BlendStateObject constructor/Activate execute natively under Unicorn.',
                                  'synthetic':'GraphicsObject memory/vtable, VBO identifiers, bitmap, projection/color shader sinks, and every GLES call are host supplied. Texture creation/upload/sampler are independently covered by airbrush-texture-native.json.',
                                  'excluded':'No native GLES device execution or device pixel output.'},
        }


def capture():
    fixture = json.loads(SAVED.read_text())
    texture_fixture = json.loads(TEXTURE.read_text())
    case = fixture['cases'][0]
    v2 = fixture['default_v2_saved_redraw']['cases'][0]
    v1_bytes = b''.join(struct.pack('<I', int(word, 16))
                        for row in case['native']['stamps'] for word in row)
    if (case['native']['stamp_count'] != 40 or v2['native']['stamp_count'] != 40 or
            hashlib.sha256(v1_bytes).hexdigest() != v2['native']['stamp_stream_sha256'] or
            v2['runtime_context']['drawable_profile_version'] != 2):
        raise RuntimeError('default V2 forty-stamp stream is not bitwise equal to captured V1 records')
    draw_case = {'native':{**case['native'],
                           'stamp_stream_sha256':v2['native']['stamp_stream_sha256']}}
    oracle = DrawOracle()
    init_result = oracle.run()
    stream, _ = oracle.draw_saved_stream(draw_case)
    raw = next(x for x in oracle.draw_gl if x['name']=='glBufferData' and x['bytes']==64)
    quad_floats = struct.unpack('<16f', bytes.fromhex(raw['data_hex']))
    quad = {'sha256': raw['data_sha256'],
            'vertices': [list(quad_floats[i:i+4]) for i in range(0,16,4)]}
    result = oracle.draw_result(init_result, draw_case, v2, stream, quad)
    if result['source']['cpu_pattern_sha256'] != texture_fixture['texture']['cpu_pattern_sha256']:
        raise RuntimeError('draw and texture fixtures disagree on AirBrush pattern source')
    return result


def main():
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--write-fixture', action='store_true')
    mode.add_argument('--check-fixture', action='store_true')
    args = parser.parse_args()
    actual = capture()
    encoded = json.dumps(actual, indent=2, sort_keys=True) + '\n'
    if args.check_fixture:
        if not OUT.exists() or OUT.read_text() != encoded:
            raise SystemExit('AirBrush Draw fixture mismatch: ' + str(OUT))
        print('AirBrush Draw fixture matches ' + str(OUT))
    elif args.write_fixture:
        OUT.write_text(encoded)
        print('wrote ' + str(OUT))


if __name__ == '__main__':
    main()
