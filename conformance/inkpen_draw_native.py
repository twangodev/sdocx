"""Call InkPen RTV1::Draw with recording virtual interfaces under Unicorn.

This is a native Draw control-flow/argument probe, not a GPU renderer. The
draw method and its internal matrix transform execute from the pinned ELF;
host vtables record boundary arguments and provide renderer returns.
"""
import argparse
import json
from pathlib import Path

import fountain_native
from fountain_native import NativeFountain

DRAW = 'SPen::InkPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)'
LIBS = Path('scratch/apk-analysis-native/arm64-v8a')
OBJDUMP = Path('scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump')


class Probe(NativeFountain):
    def __init__(self, library_dir, objdump):
        fountain_native.HASHES['InkPen'] = '939a7c719838306d693f96701994486b03f5c7eea35f16f25c6a89d84daba401'
        super().__init__(library_dir, objdump, ('InkPen',))
        self.stubs = {}
        self.calls = []
        self.stopped = False
        self.rect_empty = False
        self.target_calls = []
        self.alpha_matrix = None
        self.callbacks['SPen::PenDrawableRT::DisableDepthStencil()'] = lambda: None
        self.callbacks['SPen::RectF::IsEmpty() const'] = lambda: self.x(0, int(self.rect_empty))
        self.callbacks['SPen::RectF::ExtendRect()'] = self.extend_rect
        self.callbacks['SPen::RectF::Width() const'] = lambda: self.f(0, 100.0)
        self.callbacks['SPen::RectF::Height() const'] = lambda: self.f(0, 100.0)
        self.callbacks['SPen::PenUtil::TransformPenRectToCanvasCoordinates(SPen::RectF const&, SPen::Matrix4<float> const&)'] = self.transform_rect
        self.callbacks['SPen::PenUtil::ConvertPenRectToNDC(SPen::RectF const&, SPen::Matrix4<float> const&, int, int, SPen::Vector4<float>*, SPen::Vector4<float>*)'] = self.convert_rect
        self.callbacks['SPen::InkPenStrokeDrawableRTV1::updateCopyGeometryData(SPen::Vector4<float> const&)'] = lambda: None
        self.callbacks['SPen::Matrix4<float>::Matrix4(float const&)'] = lambda: None

    def stub(self, name, return_value=0, matrix_out=False):
        address = 0x20001000 + len(self.stubs) * 4
        self.stubs[address] = (name, return_value, matrix_out)
        return address

    def table(self, slots):
        vtable = self.alloc((max(slots) + 1) * 8)
        for index, spec in slots.items():
            name, result, matrix_out = spec
            self.put(vtable + index * 8, 'Q', self.stub(name, result, matrix_out))
        return vtable

    def object(self, slots):
        obj = self.alloc(32)
        self.put(obj, 'Q', self.table(slots))
        return obj

    def extend_rect(self):
        self.put(self.x(0), '4f', 0, 0, 100, 100)

    def transform_rect(self):
        for i, v in enumerate((0.0, 0.0, 100.0, 100.0)):
            self.f(i, v)

    def convert_rect(self):
        self.target_calls.append({'width': self.x(2), 'height': self.x(3)})
        self.put(self.x(4), '4f', -1, -1, 1, 1)
        self.put(self.x(5), '4f', -1, -1, 1, 1)

    def hook(self, u, address, size, data):
        draw = self.syms.get(DRAW)
        if draw is not None and address == draw + 0x4fc:
            matrix = self.x(1)
            self.alpha_matrix = list(self.get(matrix, '16f'))
        if address in self.stubs:
            name, result, matrix_out = self.stubs[address]
            args = [self.x(i) for i in range(6)]
            self.calls.append({'name': name, 'x': args,
                               's': [self.f(i) for i in range(4)]})
            if matrix_out:
                # Identity canvas transform returned through the native x8 out pointer.
                self.put(self.x(8), '16f', 1, 0, 0, 0, 0, 1, 0, 0,
                         0, 0, 1, 0, 0, 0, 0, 1)
            self.x(0, result)
            from unicorn import arm64_const as R
            u.reg_write(R.UC_ARM64_REG_PC, u.reg_read(R.UC_ARM64_REG_LR))
            if name == 'canvas_set_texture':
                self.stopped = True
                u.emu_stop()
            return
        super().hook(u, address, size, data)


def capture(library_dir, objdump, redraw_state):
    from unicorn import arm64_const as R
    probe = Probe(library_dir, objdump)
    d = probe.alloc(512)
    sub = probe.object({9: ('subbitmap_get_texture', 7, False),
                        5: ('subbitmap_prepare_composite', 0, False)})
    renderer = probe.object({24: ('renderer_get_context', 0, False),
                             4: ('renderer_prepare_rect', 0, False),
                             6: ('renderer_set_viewport', 0, False),
                             8: ('renderer_set_target', 0, False),
                             2: ('renderer_finish_geometry', 0, False),
                             7: ('renderer_draw_geometry', 0, False)})
    context = probe.object({21: ('context_get_canvas', 0, False)})
    canvas = probe.object({8: ('canvas_bind_texture', 0, False),
                           5: ('canvas_prepare', 0, False),
                           2: ('canvas_export_matrix', 0, True),
                           6: ('canvas_finish', 0, False)})
    texture_api = probe.object({2: ('canvas_set_texture', 0, False)})
    probe.put(canvas + 8, 'Q', probe.get(texture_api, 'Q')[0])
    # Set explicit object returns for the context chain and texture result.
    probe.stubs[next(a for a, s in probe.stubs.items() if s[0] == 'canvas_bind_texture')] = ('canvas_bind_texture', canvas, False)
    probe.stubs[next(a for a, s in probe.stubs.items() if s[0] == 'canvas_prepare')] = ('canvas_prepare', canvas, False)
    probe.stubs[next(a for a, s in probe.stubs.items() if s[0] == 'renderer_get_context')] = ('renderer_get_context', context, False)
    probe.stubs[next(a for a, s in probe.stubs.items() if s[0] == 'context_get_canvas')] = ('context_get_canvas', canvas, False)
    blend = {}
    for off, label in ((224, 'blend_source_over'), (232, 'blend_replace'), (240, 'blend_max')):
        obj = probe.object({3: (label, 0, False)})
        probe.put(d + off, 'Q', obj)
        blend[label] = obj
    alpha_program = probe.object({0: ('alpha_uniform_lookup', 1, False),
                                 2: ('activate_alpha_program', 0, False)})
    alpha = probe.alloc(32)
    probe.put(alpha, 'QQ', alpha_program, alpha_program)
    probe.put(d + 248, 'Q', alpha)
    composite_program = probe.object({0: ('get_composite_uniform', 1, False),
                                      2: ('activate_composite_program', 0, False)})
    composite = probe.alloc(32)
    probe.put(composite, 'QQQ', composite_program, 0, composite_program)
    probe.put(d + 256, 'Q', composite)
    draw_object = probe.object({7: ('draw_instanced', 0, False)})
    probe.put(d + 208, 'Q', draw_object)
    copy_object = probe.object({6: ('draw_copy_geometry', 0, False)})
    probe.put(d + 216, 'Q', copy_object)
    probe.put(d + 200, 'Q', renderer)
    probe.put(d + 72, 'I', 3)
    probe.put(d + 25, 'B', int(redraw_state))
    probe.rect_empty = False
    probe.put(d + 28, '4f', 0, 0, 100, 100)
    probe.put(d + 193, 'B', 0)
    probe.put(d + 272, 'ii', 256, 128)
    vec, points = probe.alloc(24), probe.alloc(36)
    probe.put(points, '9f', 10, 20, 2, 30, 20, 2, 40, 25, 3)
    probe.put(vec, 'QQQ', points, points + 36, points + 36)
    probe.put(d + 88, 'Q', vec)
    matrix = probe.alloc(64)
    probe.put(matrix, '16f', 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1)
    probe.u.reg_write(R.UC_ARM64_REG_SP, 0x200f0000)
    probe.u.reg_write(R.UC_ARM64_REG_LR, 0x200f1000)
    probe.x(0, d); probe.x(1, sub); probe.x(2, matrix)
    probe.u.emu_start(probe.syms[DRAW], 0x200f1000, count=200000)
    if not probe.stopped and probe.u.reg_read(R.UC_ARM64_REG_PC) != 0x200f1000:
        raise RuntimeError('native Draw did not return')
    draw = next(call for call in probe.calls if call['name'] == 'draw_instanced')
    return {'native_library_sha256': '939a7c719838306d693f96701994486b03f5c7eea35f16f25c6a89d84daba401',
            'draw_state': 'redraw' if redraw_state else 'ordinary',
            'supplied': {'instance_count': 3, 'target_size': [256, 128], 'input_matrix': 'identity',
                         'synthetic_native_point_vector': [[10, 20, 2], [30, 20, 2], [40, 25, 3]]},
            'draw_instanced_x1_x2_x3': draw['x'][1:4],
            'recorded_calls': probe.calls,
            'native_alpha_shader_matrix_argument': probe.alpha_matrix,
            'native_target_size_call': probe.target_calls,
            'boundary': 'Actual ELF Draw and helper matrix transform execute through the instanced draw, native PenUtil target-size conversion when the redraw branch reaches it, and composite texture binding; a host hook stops before composite uniform setup. All renderer, sub-bitmap, shader-program, blend and GL-object virtual methods are recording host stubs. No Init, GL, texture or GPU execution.'}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--library-dir', type=Path, default=LIBS)
    p.add_argument('--objdump', type=Path, default=OBJDUMP)
    p.add_argument('--output', type=Path, default=Path('tmp/vector-parity-plan/inkpen-draw-probe.json'))
    p.add_argument('--redraw-state', action='store_true')
    args = p.parse_args()
    args.output.write_text(json.dumps(capture(args.library_dir, args.objdump, args.redraw_state), indent=2) + '\n')
    print(args.output)


if __name__ == '__main__':
    main()
