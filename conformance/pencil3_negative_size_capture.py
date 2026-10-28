#!/usr/bin/env python3
"""Compare negative Pencil3 sizes through native Drawing and saved redraw.

Run with the environment documented in pencil3_no_curve_capture.py. This
overrides only the harness assertion that assumes the setter preserves size;
the replacement checks the native clamp instead. No native fields are changed.
"""
import hashlib
import json
import struct
import pencil3_sampler_inputs_native as c
old = c.replace_once
def replace(source, before, after, purpose):
    needle="if setting_resolution != [args.width,args.height] or setting_size_bits != source_size_bits: raise RuntimeError(f'native Pen settings mismatch: size={setting_size_bits} screen={setting_resolution}')"
    replacement="if setting_resolution != [args.width,args.height] or setting_size_bits != struct.pack('<f',max(0.4,min(input_case['size'],800.0))).hex(): raise RuntimeError(f'native Pen settings mismatch: size={setting_size_bits} screen={setting_resolution}')"
    if needle in source:
        source=source.replace(needle,replacement,1)
    return old(source,before,after,purpose)
c.replace_once=replace
expected = 'e597079876c3b10ab3def4cf395d8db944ba565d744c33cf5f682f09f04108a4'
cases = []
for size in (-10.0, -0.1, 0.4):
    case = dict(c.CASES['baseline_tool2_curve'])
    case['size'] = size
    native, _, _, source_hash = c.run_native(case, 'empty')
    words = native['streams']['draw_object_stroke']['values_f32_bits']
    digest = hashlib.sha256(b''.join(bytes.fromhex(word) for word in words)).hexdigest()
    same = native['streams']['saved_redraw']['same_bytes_as_drawing_phase']
    if len(words) != 5_240 or digest != expected or not same:
        raise RuntimeError(f'unexpected native stream for size {size}: {len(words)}, {digest}, {same}')
    cases.append({'input_size': size,
                  'input_size_f32_bits_le': struct.pack('<f', size).hex(),
                  'model_getter_size_f32_bits_le': native['input']['pen_size_f32_bits'],
                  'native_pen_size_f32_bits_le': native['pen_setting_data']['pen_size_f32_bits_offset_0'],
                  'selected_drawable_class': native['route']['selected_drawable_class'],
                  'record_count': len(words) // 5, 'drawing_sha256': digest,
                  'redraw_sha256': native['streams']['saved_redraw']['sha256'],
                  'drawing_and_redraw_match': same})
print(json.dumps({'schema': 'pencil3-negative-size-native-v1',
                  'boundary': 'Synthetic native Model setters and Drawing under Unicorn with host adapters; no serialized parser or device output.',
                  'saved_caller_source_sha256': source_hash,
                  'pencil3_sha256': c.PENCIL3_SHA256, 'cases': cases}, indent=2))
