#!/usr/bin/env python3
"""Capture short Pencil3 saved strokes; environment as pencil3_no_curve_capture.py."""
import hashlib
import json
import pencil3_sampler_inputs_native as capture

cases = {}
for count in (1, 2, 3):
    case = dict(capture.CASES['baseline_tool2_curve'])
    for channel in ('points', 'pressure', 'tilt', 'orientation', 'timestamps'):
        case[channel] = case[channel][:count]
    try:
        native, _, _, source_hash = capture.run_native(case, 'empty')
    except RuntimeError as error:
        if count != 1 or str(error) != 'Drawing failed to select Pencil3: result=0, loader=[]':
            raise
        cases[str(count)] = {'input_profile': case, 'drawing_result': 0, 'drawable_selected': False}
        continue
    words = native['streams']['draw_object_stroke']['values_f32_bits']
    if not native['streams']['saved_redraw']['same_bytes_as_drawing_phase']:
        raise RuntimeError(f'{count}: Drawing and saved redraw differ')
    raw = b''.join(bytes.fromhex(word) for word in words)
    cases[str(count)] = {'input_profile': case, 'packed_words_le': words,
                         'sha256': hashlib.sha256(raw).hexdigest(),
                         'drawing_and_saved_redraw_match': True}
print(json.dumps({'schema': 'pencil3-short-stroke-native-v1',
                  'boundary': 'Synthetic native Model and Drawing execution under Unicorn with host adapters; no device framebuffer or serialized-note claim.',
                  'saved_caller_source_sha256': source_hash,
                  'pencil3_sha256': capture.PENCIL3_SHA256, 'cases': cases}, indent=2, sort_keys=True))
