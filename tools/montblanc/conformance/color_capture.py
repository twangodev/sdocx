#!/usr/bin/env python3
"""Reproduce the pinned host-Mesa native SetPenData color controls."""
import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import zlib

ROOT = Path(__file__).resolve().parents[3]
HARNESS = ROOT / 'tmp/vector-parity-plan/montblanc-rtv1-mesa-source-v12/capture_candidate.py'
RECEIPT = ROOT / 'conformance/montblanc-native-color.json'
COLORS = ('ff336699', '80336699', '00336699', 'ffff0000', 'ff00ff00', 'ff0000ff')


def digest(data):
    return hashlib.sha256(data).hexdigest()


def summarize(capture):
    pre = zlib.decompress(base64.b64decode(capture['pre_readback']['rgba8_zlib_base64']))
    post = zlib.decompress(base64.b64decode(capture['post_readback']['rgba8_zlib_base64']))
    assert len(pre) == len(post) == 256 * 256 * 4
    assert pre == bytes(len(pre))
    assert digest(pre) == capture['pre_readback']['sha256']
    assert digest(post) == capture['post_readback']['sha256']
    assert capture['target']['completeness'] == 36053
    assert capture['target']['framebuffer'] == capture['target']['native_offscreen_framebuffer_id']
    assert capture['target']['texture'] == capture['target']['read_attachment']
    assert all(d['state']['framebuffer'] == capture['target']['framebuffer'] for d in capture['draw_calls'])
    assert all(not e['before'] and not e['after'] for e in capture['gl_error_checks'])
    assert all(not capture[k][f'gl_error_{when}'] for k in ('pre_readback', 'post_readback') for when in ('before', 'after'))
    shaders = [s for d in capture['shader_source_diagnostics'] for s in d['sources'] if 'gl_FragColor' in s['text']]
    assert len(shaders) == 3
    assert all('gl_FragColor = vec4(inputColor.xyz, 1.0) * alpha;' in s['text'] for s in shaders)
    return {'setter_calls': capture['style']['observed_calls'],
            'color_uniforms': [d['values'] for d in capture['uniform_diagnostics'] if d['call'] == 'glUniform4fv'],
            'fragment_shader_sha256': [s['sha256'] for s in shaders],
            'draw_counts': [d['values'] for d in capture['draw_calls']],
            'pre_rgba8_sha256': digest(pre), 'post_rgba8_sha256': digest(post),
            'pixel_statistics': capture['pixel_statistics']}


def main():
    receipt = json.loads(RECEIPT.read_text())
    assert digest(HARNESS.read_bytes()) == receipt['harness_sha256']
    env = os.environ.copy()
    env['PYTHONPATH'] = os.pathsep.join((str(ROOT / 'scratch/apk-analysis-runtime/python'), str(ROOT / 'tmp/vector-parity-plan/montblanc-drawing-capture-root-v2/conformance')))
    with tempfile.TemporaryDirectory(prefix='montblanc-color-') as directory:
        for color in COLORS:
            output = Path(directory) / f'{color}.json'
            subprocess.run([sys.executable, str(HARNESS), '--case', 'baseline', '--argb', '0x' + color, '--output', str(output)], cwd=ROOT, env=env, check=True)
            assert summarize(json.loads(output.read_text())) == receipt['cases'][color], color
    print('Verified native host-Mesa color and opacity controls')


if __name__ == '__main__':
    main()
