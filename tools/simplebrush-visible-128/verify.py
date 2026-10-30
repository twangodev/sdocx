#!/usr/bin/env python3
"""Replay the recorded control from clean source and retained native inputs."""
import ast
import hashlib
import json
import os
from pathlib import Path
import re
import struct
import subprocess
import tarfile
import tempfile
import zlib

root = Path(__file__).resolve().parents[2]
receipt = json.loads((root / 'conformance/simplebrush-visible-128/tool0-receipt.json').read_text())
sha = lambda b: hashlib.sha256(b).hexdigest()
for path, digest in receipt['comparison_script_sha256'].items():
    assert sha((root / path).read_bytes()) == digest
assert sha((root / 'tools/simplebrush-visible-128/capture.py').read_bytes()) == receipt['capture_sha256']
with tempfile.TemporaryDirectory(prefix='simplebrush-visible-replay-') as tmp:
    tmp = Path(tmp)
    archive = tmp / 'source.tar'
    with archive.open('wb') as f:
        subprocess.run(['git', 'archive', receipt['source_revision']], cwd=root, stdout=f, check=True)
    source = tmp / 'source'; source.mkdir()
    with tarfile.open(archive) as tar:
        tar.extractall(source, filter='data')
    test = source / 'crates/sdocx/src/render/simplebrush_tests.rs'
    text = test.read_text()
    old = 'if count > 128 {'
    assert text.count(old) == 3
    text = text.replace(old, 'if count >= 128 {', 1)
    old = '''        assert_eq!(
            hex,
            native["drawing_vector"]["vector_bytes"].as_str().unwrap()
        );'''
    assert text.count(old) >= 1
    text = text.replace(old, '''        std::fs::write(std::env::var("SDOCX_GEOMETRY_OUTPUT").unwrap(), hex).unwrap();
        let _ = native;''', 1)
    test.write_text(text)
    output = tmp / 'output'; output.mkdir(); (output / 'captures').mkdir()
    frame = zlib.decompress((root / 'conformance/simplebrush-visible-128/tool0-frame.rgba.zlib').read_bytes())
    geometry = (root / 'conformance/simplebrush-visible-128/tool0-geometry.bin').read_bytes()
    assert sha(frame) == receipt['native_frame_sha256'] and sha(geometry) == receipt['geometry_sha256']
    edge = sum(frame[(y*256+x)*4+3] != 0 for y in range(256) for x in range(256) if x in (0,255) or y in (0,255))
    assert edge == receipt['edge_alpha_pixels'] == 0
    (output / 'native.json').write_text(json.dumps({'drawing_vector': {'vector_bytes': geometry.hex()}, 'native_draw_target_readback': {'sha256': sha(frame)}}))
    (output / 'captures' / f'simplebrush-native-draw-rgba-{sha(frame)}.bin').write_bytes(frame)
    env = os.environ | {'CARGO_TARGET_DIR':'/tmp/sdocx-agent-target', 'CARGO_INCREMENTAL':'0', 'CARGO_PROFILE_DEV_DEBUG':'0', 'CARGO_PROFILE_TEST_DEBUG':'0', 'CARGO_BUILD_JOBS':'2', 'SDOCX_POINTS_COUNT':'128', 'PROBE_SIZE':'3', 'VARIANT':'reverse', 'SDOCX_TOOL':'0', 'SDOCX_SIMPLEBRUSH_TWO_EXPORT':str(output), 'SDOCX_V62_NATIVE':str(output/'native.json'), 'SDOCX_GEOMETRY_OUTPUT':str(output/'rust-geometry.hex')}
    subprocess.run(['cargo','test','-p','sdocx','--lib','--features','pdf','two_pixel_native_export_fixture','--locked','--offline','--','--nocapture'], cwd=source, env=env, check=True)
    rust = bytes.fromhex((output/'rust-geometry.hex').read_text())
    assert len(rust) == len(geometry) == 1425*4
    a = struct.unpack('<1425f', rust); b = struct.unpack('<1425f', geometry)
    differences = [(i,x,y) for i,(x,y) in enumerate(zip(a,b)) if x != y]
    assert differences == [(711,56.49998474121094,56.499977111816406)]
    compare = subprocess.check_output([os.sys.executable, str(root/'tools/simplebrush-long-expanded/compare.py'), str(output)], text=True)
    values = {line.split(' ',1)[0]: ast.literal_eval(line.split(' ',1)[1]) for line in compare.splitlines()}
    for mode, name in [('generic','generic'),('two','two_pixel')]:
        assert values[mode]['svg_alpha_error'] == receipt['resvg_alpha_error'][name]
        assert values[mode]['matched'] == receipt['mupdf_paper_matched_rgb_error'][name]
    assert values['two']['svg_support'] == receipt['resvg_alpha_support_intersection_union']['two_pixel']
    pdf = subprocess.check_output([os.sys.executable, str(root/'tools/simplebrush-v209/pdfium.py'), str(output)], text=True)
    numbers = dict(line.split() for line in pdf.splitlines())
    assert int(numbers['generic']) == receipt['pdfium_rgb_error']['generic']
    assert int(numbers['two']) == receipt['pdfium_rgb_error']['two_pixel']
    print('Verified visible frame, single geometry lane difference, SVG, MuPDF and PDFium comparisons.')
