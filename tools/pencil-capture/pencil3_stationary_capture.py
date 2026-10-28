#!/usr/bin/env python3
"""Capture stationary Pencil3 V3 Drawing controls using the retained native harness.

Use the PYTHONPATH and LD_LIBRARY_PATH documented in pencil3_no_curve_capture.py.
Writes a compact receipt to stdout; no fixture is updated automatically.
"""
import hashlib
import json
import pencil3_sampler_inputs_native as capture

cases = {}
for pressure in (0.0, 0.5, 1.0):
    case = dict(capture.CASES['baseline_tool2_curve'])
    case['points'] = [[0.0, 0.0] for _ in range(4)]
    case['pressure'] = [pressure] * 4
    native, _, _, source_hash = capture.run_native(case, 'empty')
    words = native['streams']['draw_object_stroke']['values_f32_bits']
    if len(words) != 35 or not native['streams']['saved_redraw']['same_bytes_as_drawing_phase']:
        raise RuntimeError(f'unexpected native stream at pressure {pressure}')
    raw = b''.join(bytes.fromhex(word) for word in words)
    cases[str(pressure)] = {'input_profile': case, 'packed_words_le': words,
                            'sha256': hashlib.sha256(raw).hexdigest(),
                            'drawing_and_saved_redraw_match': True}
print(json.dumps({'schema': 'pencil3-stationary-native-v1',
                  'boundary': 'Synthetic native Model and Drawing execution under Unicorn with host adapters; no device framebuffer or serialized-note claim.',
                  'saved_caller_source_sha256': source_hash,
                  'pencil3_sha256': capture.PENCIL3_SHA256, 'cases': cases}, indent=2, sort_keys=True))
