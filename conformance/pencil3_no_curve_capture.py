#!/usr/bin/env python3
"""Capture Pencil3 saved-redraw controls with curve disabled.

Requires the retained native harness in tmp/conformance-cleanup/native-capture-a241793
and the project-local Unicorn/AArch64 tools. Run from the repository root with
PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/apk-analysis-runtime/binutils-aarch64/usr/bin:tmp/conformance-cleanup/native-capture-a241793/conformance
and LD_LIBRARY_PATH=scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu.
Writes a compact receipt to stdout; it does not update fixtures.
"""
import hashlib
import json
from pathlib import Path

import pencil3_sampler_inputs_native as capture

root = Path(__file__).resolve().parents[1]
fixture = json.loads((root / 'conformance/pencil3-sampler-inputs-native.json').read_text())
cases = {}
for name in ('baseline_tool2_curve', 'tool3_curve_4pt'):
    case = dict(capture.CASES[name])
    case['curve_enabled'] = False
    native, _, _, source_hash = capture.run_native(case, 'empty')
    snapshots = native['native_calls']['ObjectStroke_getter_snapshots']
    if not snapshots or any(row['curve_enabled'] for row in snapshots):
        raise RuntimeError(f'{name}: native Model curve getter did not remain false')
    words = native['streams']['draw_object_stroke']['values_f32_bits']
    expected = fixture['cases'][name]['routes']['v3']['phases']['draw_object_stroke']['native_packed_stream_f32_bits_le']
    if words != expected or not native['streams']['saved_redraw']['same_bytes_as_drawing_phase']:
        raise RuntimeError(f'{name}: curve-disabled stream differs')
    raw = b''.join(bytes.fromhex(word) for word in words)
    cases[name] = {'curve_enabled': False, 'native_model_curve_getter_false': True, 'packed_word_count': len(words),
                   'stream_sha256': hashlib.sha256(raw).hexdigest(),
                   'same_as_curve_enabled_fixture': True,
                   'selected_drawable_class': native['route']['selected_drawable_class'],
                   'drawing_and_saved_redraw_match': True}
print(json.dumps({'schema': 'pencil3-no-curve-native-v1',
                  'boundary': 'Synthetic native Model and Drawing execution under Unicorn with host adapters; no device framebuffer or serialized-note claim.',
                  'saved_caller_source_sha256': source_hash,
                  'pencil3_sha256': capture.PENCIL3_SHA256,
                  'cases': cases}, indent=2, sort_keys=True))
