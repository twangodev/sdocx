"""Join same-object native BrushPen ObjectStroke output to host-Mesa paint."""
import argparse
import base64
import hashlib
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GEOMETRY = ROOT / 'conformance/brushpen-saved-native.json'
TEXTURE = ROOT / 'conformance/brushpen-texture-native.json'
POSITIVE_SOURCE = ROOT / 'conformance/brushpen_paint_positive_native.py'
TEXTURE_SOURCE = ROOT / 'conformance/brushpen_texture_native.py'
GEOMETRY_SOURCE = ROOT / 'conformance/brushpen_saved_native.py'
OUT = ROOT / 'conformance/brushpen-same-run-native.json'

import sys
sys.path.insert(0, str(ROOT / 'conformance'))
import brushpen_paint_positive_native as positive
import brushpen_saved_native as geometry

OBJECT_CASE = geometry.OBJECT_CASE


def sha256(raw):
    return hashlib.sha256(raw).hexdigest()


def assert_same_bytes(expected, actual, context):
    if expected != actual:
        raise AssertionError(f'{context}: expected {len(expected)} bytes/{sha256(expected)}, '
                             f'got {len(actual)} bytes/{sha256(actual)}')


class SameRunProbe(positive.PositiveProbe):
    def __init__(self, poison_variant='native-diagnostic'):
        super().__init__(poison_variant)
        self.stop_before_end_pen = False
        self.paint_state = None
        self.source_result = None
        self.source_vector_bytes = None
        self.draw_args = None
        self.full_vector_alias = False
        self.vector_alias_capture = None
        self.buffer_transfers = []
        self.timeline = []
        self.native_this_records = []
        original_setup = self.setup_native_pen

        def setup_once():
            if self.paint_state is None:
                self.paint_state = original_setup()
            return self.paint_state
        self.setup_native_pen = setup_once

    def call(self, name, x=(), f=(), instruction_limit=2_000_000):
        if name in ('SPen::BrushPenStrokeDrawableRTV1::Init()',
                    'SPen::BrushPenStrokeDrawableRTV1::Update(bool)',
                    'SPen::BrushPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',
                    'SPen::BrushPenStrokeDrawableRTV1::SetStartPoint(float, float)'):
            rt = self.paint_state['rt'] if self.paint_state else 0
            self.native_this_records.append({'method': name, 'this': hex(x[0]),
                                             'matches_initialized_rt': bool(rt and x[0] == rt)})
        if name == 'SPen::BrushPenStrokeDrawableRTV1::Init()':
            self.timeline.append({'event': 'RTV1::Init', 'phase': 'before-native-call'})
        if name == 'SPen::BrushPenStrokeDrawableRTV1::SetStartPoint(float, float)':
            self.timeline.append({'event': 'RTV1::SetStartPoint', 'phase': 'before-native-call',
                                  'synthetic_values': list(self.synthetic_start)})
        if name == 'SPen::BrushPenStrokeDrawableRTV1::Update(bool)' and self.full_vector_alias:
            rt = x[0]
            if rt != self.paint_state['rt']:
                raise RuntimeError('full-vector Update target is not the initialized RTV1')
            before = bytes(self.u.mem_read(rt + 80, 16))
            producer_vector = self.get(rt + 80, 'Q')[0]
            old_draw_vector = self.get(rt + 88, 'Q')[0]
            if producer_vector == old_draw_vector:
                raise RuntimeError('negative control: producer and prior draw vectors unexpectedly alias')
            self.put(rt + 88, 'Q', producer_vector)
            after = bytes(self.u.mem_read(rt + 80, 16))
            new_draw_vector = self.get(rt + 88, 'Q')[0]
            if new_draw_vector != producer_vector:
                raise RuntimeError('failed to alias the native producer vector at rt+88')
            self.vector_alias_capture = {
                'rt_base': hex(rt),
                'pointer_fields': {'producer_vector_offset_dec': 80,
                                   'draw_input_vector_offset_dec': 88},
                'raw_before_hex_16_bytes_from_rt_plus_80': before.hex(),
                'raw_after_hex_16_bytes_from_rt_plus_80': after.hex(),
                'producer_pointer_before': hex(producer_vector),
                'draw_pointer_before': hex(old_draw_vector),
                'draw_pointer_after': hex(new_draw_vector),
                'same_pointer_before': producer_vector == old_draw_vector,
                'same_pointer_after': producer_vector == new_draw_vector,
                'mutated_qword_offset': 88,
                'mutated_qword_raw_before_hex': struct.pack('<Q', old_draw_vector).hex(),
                'mutated_qword_raw_after_hex': struct.pack('<Q', new_draw_vector).hex(),
            }
            self.timeline.append({'event': 'host pointer alias', 'phase': 'before-native-Update',
                                  'rt_plus_80_equals_rt_plus_88': True})
        if name == 'SPen::BrushPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)':
            self.draw_args = list(x)
        result = super().call(name, x, f, instruction_limit)
        if name in ('SPen::BrushPenStrokeDrawableRTV1::Init()',
                    'SPen::BrushPenStrokeDrawableRTV1::Update(bool)',
                    'SPen::BrushPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)'):
            self.timeline.append({'event': name, 'phase': 'returned-from-native-call'})
        return result

    def gl_buffer_data_real(self):
        target, size, pointer, usage = [self.x(i) for i in range(4)]
        raw = bytes(self.u.mem_read(pointer, size)) if pointer and size else b''
        self.buffer_transfers.append({'api': 'glBufferData', 'target': target, 'size': size,
                                      'usage': usage, 'data_hex': raw.hex(),
                                      'sha256': sha256(raw)})
        return super().gl_buffer_data_real()

    def gl_buffer_sub_data_real(self):
        target, offset, size, pointer = [self.x(i) for i in range(4)]
        raw = bytes(self.u.mem_read(pointer, size)) if pointer and size else b''
        self.buffer_transfers.append({'api': 'glBufferSubData', 'target': target,
                                      'offset': offset, 'size': size,
                                      'data_hex': raw.hex(), 'sha256': sha256(raw)})
        return super().gl_buffer_sub_data_real()


def read_producer_vector(probe, rt, source_result):
    vector = probe.get(rt + 80, 'Q')[0]
    begin, end, capacity = probe.get(vector, '3Q')
    if not begin or end <= begin or end > capacity or (end - begin) % 44:
        raise RuntimeError(f'invalid native producer vector [{begin:#x}, {end:#x}, {capacity:#x}]')
    raw = bytes(probe.u.mem_read(begin, end - begin))
    records = source_result['buffer_records']
    if len(raw) != len(records) * 44:
        raise RuntimeError('live native vector size differs from captured AddPoint record count')
    if sha256(raw) != records[-1]['cumulative_sha256']:
        raise RuntimeError('live native vector hash differs from final AddPoint cumulative hash')
    return raw


def run_oracle(poison_variant='native-diagnostic'):
    probe = SameRunProbe(poison_variant)
    # This first positive paint run executes the real native Init/Draw path and
    # leaves its independent RGBA FBO, shaders, textures, and native RT alive.
    initial_paint = probe.run()
    init_count = probe.native_entries.get('SPen::BrushPenStrokeDrawableRTV1::Init()', 0)
    if init_count != 1:
        raise RuntimeError(f'expected one native Init before ObjectStroke, found {init_count}')
    probe.timeline.append({'event': 'ObjectStroke producer', 'phase': 'before-native-call',
                          'rt_matches_initialized_object': True})
    probe.source_result = probe.run_object_core(OBJECT_CASE)
    rt = probe.paint_state['rt']
    if probe.source_result['admission']['actual_getVersion'] != 10:
        raise RuntimeError('ObjectStroke producer did not select GLV10')
    callback_rt_by_addpoint = [record['rt_this']
                               for record in probe.source_result['callback_arguments']]
    callback_rts = sorted(set(callback_rt_by_addpoint))
    if callback_rts != [hex(rt)] or len(callback_rt_by_addpoint) != len(probe.source_result['buffer_records']):
        raise RuntimeError(f'AddPoint receiver identities do not all match initialized RTV1: '
                           f'{callback_rt_by_addpoint[:4]}... vs {hex(rt)}')
    probe.source_vector_bytes = read_producer_vector(probe, rt, probe.source_result)
    saved = json.loads(GEOMETRY.read_text())
    expected_case = next(case for case in saved['synthetic_cases']
                         if case['label'] == OBJECT_CASE['name'])
    assert_same_bytes(
        b''.join(bytes.fromhex(word)
                 for row in expected_case['buffer_records']
                 for word in row['instance_bits_le']),
        probe.source_vector_bytes,
        'native ObjectStroke producer vector vs saved geometry fixture')
    if probe.source_result['buffer_records'] != expected_case['buffer_records']:
        raise AssertionError('native ObjectStroke AddPoint boundaries differ from saved fixture')
    probe.timeline.append({'event': 'RTV1::AddPoint', 'phase': 'producer-complete',
                           'record_count': len(probe.source_result['buffer_records']),
                           'bytes': len(probe.source_vector_bytes),
                           'sha256': sha256(probe.source_vector_bytes)})

    records = [struct.unpack_from('<11f', probe.source_vector_bytes, offset)
               for offset in range(0, len(probe.source_vector_bytes), 44)]
    # The pinned vertex shader floors lane 3 to select a 3x3 atlas tile. Its
    # fractional component controls opacity, so retain both values and the
    # actual selected indices for the entire emitted stream.
    tile_ids = [int(row[3] // 1) for row in records]
    tile_counts = {str(tile): tile_ids.count(tile) for tile in sorted(set(tile_ids))}
    if any(tile < 0 or tile > 6 for tile in tile_ids):
        raise RuntimeError(f'full stream selects a diagnostic/uninitialized pattern tile: {tile_ids}')
    xs, ys = [row[0] for row in records], [row[1] for row in records]
    center = [(min(xs) + max(xs)) / 2, (min(ys) + max(ys)) / 2]
    scale = [16 / 321, 16 / 228]
    probe.synthetic_start = struct.unpack('<2f', struct.pack('<2f', *center))
    matrix = [scale[0], 0, 0, 0, 0, scale[1], 0, 0,
              0, 0, 1, 0, -scale[0] * center[0], -scale[1] * center[1], 0, 1]
    probe.synthetic_caller_matrix = struct.unpack('<16f', struct.pack('<16f', *matrix))

    # The native producer writes rt+80. The draw/update path reads rt+88. The
    # full replay below sets a pointer alias only; it does not copy instance data.
    framebuffer = C.c_int()
    probe.mesa.f['glGetIntegerv'](0x8CA6, C.byref(framebuffer))
    if framebuffer.value != initial_paint['output']['framebuffer_id']:
        raise RuntimeError('same-run full-vector replay would use a different framebuffer')
    probe._call_gl('glClearColor', probe.mesa.f['glClearColor'], 0.0, 0.0, 0.0, 0.0)
    probe._call_gl('glClear', probe.mesa.f['glClear'], 0x4000)
    probe.call('SPen::BrushPenStrokeDrawableRTV1::SetStartPoint(float, float)', [rt])
    probe.full_vector_alias = True
    probe.timeline.append({'event': 'RTV1::Update', 'phase': 'before-full-stream-native-call',
                           'instance_count': len(probe.source_vector_bytes) // 44})
    probe.call('SPen::BrushPenStrokeDrawableRTV1::Update(bool)', [rt, 0])
    if not probe.draw_args or probe.draw_args[0] != rt:
        raise RuntimeError('no captured Draw args for the initialized RTV1')
    probe.call('SPen::BrushPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',
               probe.draw_args)
    pixels = C.create_string_buffer(321 * 228 * 4)
    probe._call_gl('glPixelStorei', probe.mesa.f['glPixelStorei'], 0x0D05, 1)
    probe._call_gl('glReadPixels', probe.mesa.f['glReadPixels'], 0, 0, 321, 228,
                   0x1908, 0x1401, C.cast(pixels, C.c_void_p))
    readback = pixels.raw
    if probe.gl_errors:
        raise RuntimeError(f'GL errors during joined native paint: {probe.gl_errors[:8]}')
    if any(not row['matches_initialized_rt'] for row in probe.native_this_records):
        raise RuntimeError(f'native RTV1 method receiver changed during joined paint: {probe.native_this_records}')
    transfers = [row for row in probe.buffer_transfers
                 if row['size'] == len(probe.source_vector_bytes)]
    if len(transfers) != 1:
        raise RuntimeError(f'expected one full native vector GL buffer transfer, got {len(transfers)}')
    uploaded = bytes.fromhex(transfers[0]['data_hex'])
    assert_same_bytes(probe.source_vector_bytes, uploaded,
                      'native producer vector vs intercepted GLES upload')

    # Negative control uses the same byte-equality guard as the real join and
    # proves that a one-byte corruption is rejected.
    corrupt = bytearray(probe.source_vector_bytes)
    corrupt[0] ^= 1
    try:
        assert_same_bytes(probe.source_vector_bytes, bytes(corrupt), 'negative mismatch control')
    except AssertionError:
        mismatch_control_rejected = True
    else:
        raise AssertionError('negative control failed to reject a corrupted producer byte')

    alpha = readback[3::4]
    libraries = initial_paint['provenance']['native_elf_sha256']
    if len(libraries) != 7:
        raise RuntimeError(f'native DSO pin list is incomplete: {sorted(libraries)}')
    return {
        'schema': 'brushpen-same-run-objectstroke-paint-v1',
        'artifact': 'Samsung Notes 4.4.45.37 native BrushPen',
        'scope': 'Synthetic ObjectStroke getter inputs execute native GLV10 RedrawPen and RTV1::AddPoint on the same RTV1 that has already completed native Init. The 26-record native vector is aliased into the draw input pointer and consumed by native Update/DrawInstanced. Native shaders, textures, blend and graphics methods execute on host Mesa. Host adapters supply ObjectStroke getters, queue/bitmap/context. EndPen finalization, app callback/queue handoff, Samsung GLES and device pixels are not established.',
        'source': {
            'case': probe.source_result['label'],
            'path': probe.source_result['path'],
            'synthetic_input': probe.source_result['synthetic_input'],
            'admission': probe.source_result['admission'],
            'AddPoint_receiver_rt_this_by_callback': callback_rt_by_addpoint,
            'AddPoint_receiver_rt_this_unique': callback_rts,
            'AddPoint_record_count': len(probe.source_result['buffer_records']),
            'vector_bytes': len(probe.source_vector_bytes),
            'vector_sha256': sha256(probe.source_vector_bytes),
            'buffer_records': probe.source_result['buffer_records'],
            'matches_saved_fixture_exactly': True,
        },
        'join': {
            'initialized_RT_same_as_producer_and_painter': True,
            'producer_pointer_field_offset_bytes': 80,
            'draw_input_pointer_field_offset_bytes': 88,
            'pointer_alias_mutation': probe.vector_alias_capture,
            'source_bytes_hex': probe.source_vector_bytes.hex(),
            'uploaded_bytes_hex': transfers[0]['data_hex'],
            'source_sha256': sha256(probe.source_vector_bytes),
            'uploaded_sha256': transfers[0]['sha256'],
            'byte_for_byte_equal': True,
            'gl_buffer_transfer': transfers[0],
            'negative_one_byte_corruption_control_rejected': mismatch_control_rejected,
            'negative_control_scope': 'byte-equality guard rejects a one-byte mutation; this does not validate Android app queue handoff or alias policy',
        },
        'paint': {
            'prior_positive_capture': initial_paint,
            'full_stream_draw_call': probe.graphics_draw_calls[-1],
            'full_stream_draw_inputs': {'synthetic_SetStartPoint_f32': list(probe.synthetic_start),
                                        'synthetic_caller_matrix_f32': probe.synthetic_caller_matrix,
                                        'instance_xy_extent_f32': [[min(xs), max(xs)], [min(ys), max(ys)]],
                                        'texture_viewport_dimensions': [321, 228]},
            'full_stream_uniforms': probe.uniform_values,
            'full_stream_atlas_selection': {
                'shader_source_sha256': initial_paint['shader_hashes']['vertex'],
                'shader_mapping': 'floor(lane3) selects tile; mod(tile,3) is column and floor(tile/3) is row; fract(lane3)*1000/255 is opacity',
                'lane3_f32': [row[3] for row in records],
                'selected_tile_indices': tile_ids,
                'selected_tile_counts': tile_counts,
                'selected_tile_sha256_by_index': {
                    str(tile): initial_paint['pattern_valid_initialized_tile_hashes'][tile // 3][tile % 3]
                    for tile in sorted(set(tile_ids))
                },
                'diagnostic_poison_tile_indices': [7, 8],
                'diagnostic_tiles_selected': any(tile in (7, 8) for tile in tile_ids),
            },
            'output': {'dimensions': [321, 228], 'format': 'RGBA8',
                       'framebuffer_id': initial_paint['output']['framebuffer_id'],
                       'texture_id': initial_paint['output']['texture_id'],
                       'complete': initial_paint['output']['complete'],
                       'input_texture_ids': initial_paint['output']['input_texture_ids'],
                       'feedback_alias': initial_paint['output']['feedback_alias'],
                       'pixel_count': 321 * 228, 'bytes': len(readback),
                       'nonzero_alpha_pixels': sum(value != 0 for value in alpha),
                       'alpha_range': [min(alpha), max(alpha)],
                       'rgba_sha256': sha256(readback),
                       'rgba_base64': base64.b64encode(readback).decode('ascii')},
            'gl_calls': probe.gl_calls_real,
            'gl_errors': probe.gl_errors,
        },
        'execution_order': probe.timeline,
        'native_this_receivers': probe.native_this_records,
        'provenance': {
            'geometry_fixture_sha256': sha256(GEOMETRY.read_bytes()),
            'texture_fixture_sha256': sha256(TEXTURE.read_bytes()),
            'positive_paint_runner_sha256': sha256(POSITIVE_SOURCE.read_bytes()),
            'texture_runner_sha256': sha256(TEXTURE_SOURCE.read_bytes()),
            'geometry_runner_sha256': sha256(GEOMETRY_SOURCE.read_bytes()),
            'native_dsos': libraries,
            'host_gl_dsos': initial_paint['provenance']['host_gles_dsos'],
            'mesa_identity': initial_paint['mesa'],
            'shader_source_sha256': initial_paint['shader_hashes'],
            'texture_profile': initial_paint['provenance']['texture_profile'],
            'texture_uploads': initial_paint['texture_uploads'],
            'selected_initialized_pattern_tile': initial_paint['provenance']['selected_initialized_atlas_tile'],
            'diagnostic_poison_cells': initial_paint['pattern_poison_cells'],
            'pattern_poison_variant': poison_variant,
        },
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--write-fixture', action='store_true')
    mode.add_argument('--check-fixture', action='store_true')
    args = parser.parse_args()
    report = run_oracle('native-diagnostic')
    alternate = run_oracle('alternate-diagnostic')
    base_upload = [row for row in report['paint']['prior_positive_capture']['texture_uploads']
                   if row['kind'] == 'upload']
    alternate_upload = [row for row in alternate['paint']['prior_positive_capture']['texture_uploads']
                        if row['kind'] == 'upload']
    if report['paint']['output']['rgba_sha256'] != alternate['paint']['output']['rgba_sha256']:
        raise AssertionError('full 26-record paint changed when only diagnostic poison tiles 7/8 changed')
    if base_upload[0]['sha256'] == alternate_upload[0]['sha256']:
        raise AssertionError('full-stream poison control did not change diagnostic atlas upload')
    if base_upload[1]['sha256'] != alternate_upload[1]['sha256']:
        raise AssertionError('full-stream poison control changed the independent grain texture')
    if report['paint']['full_stream_atlas_selection']['diagnostic_tiles_selected']:
        raise AssertionError('full stream selected diagnostic poison tiles')
    if report['paint']['full_stream_atlas_selection']['selected_tile_sha256_by_index'] != \
            alternate['paint']['full_stream_atlas_selection']['selected_tile_sha256_by_index']:
        raise AssertionError('selected initialized tiles changed in full-stream poison control')
    report['paint']['full_stream_poison_sensitivity_control'] = {
        'changed_diagnostic_tile_indices': [7, 8],
        'native_diagnostic_pattern_upload_sha256': base_upload[0]['sha256'],
        'alternate_diagnostic_pattern_upload_sha256': alternate_upload[0]['sha256'],
        'grain_upload_sha256_unchanged': base_upload[1]['sha256'],
        'selected_initialized_tile_sha256_by_index_baseline': report['paint']['full_stream_atlas_selection']['selected_tile_sha256_by_index'],
        'selected_initialized_tile_sha256_by_index_alternate': alternate['paint']['full_stream_atlas_selection']['selected_tile_sha256_by_index'],
        'baseline_full_stream_rgba_sha256': report['paint']['output']['rgba_sha256'],
        'alternate_full_stream_rgba_sha256': alternate['paint']['output']['rgba_sha256'],
        'full_stream_output_unchanged': True,
        'interpretation': 'host-only diagnostic poison sensitivity check for this complete synthetic 26-record stream; does not establish production heap contents or device behavior',
    }
    encoded = json.dumps(report, indent=2, sort_keys=True) + '\n'
    if args.write_fixture:
        OUT.write_text(encoded)
        print(f'wrote {OUT}')
        return
    if not OUT.exists():
        raise SystemExit(f'missing fixture {OUT}; review output before --write-fixture')
    expected = json.loads(OUT.read_text())
    if json.dumps(expected, sort_keys=True) != json.dumps(report, sort_keys=True):
        raise AssertionError('same-run BrushPen producer-to-paint output differs from reviewed fixture')
    print('BrushPen same-run native ObjectStroke-to-paint oracle passed')


if __name__ == '__main__':
    C = positive.C
    main()
