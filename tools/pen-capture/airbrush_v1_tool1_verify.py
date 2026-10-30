#!/usr/bin/env python3
"""Verify fresh tool-1 captures or compare the pinned direct PDFs."""
import base64
import hashlib
import json
from pathlib import Path
import sys
import zlib

root = Path(__file__).resolve().parents[2]
receipt = json.loads((root / 'tools/pen-capture/airbrush_v1_tool1_receipt.json').read_text())
mode, directory = sys.argv[1], Path(sys.argv[2])
if mode == 'capture':
    prerequisites = receipt['capture_prerequisites']
    for key in ('manifest', 'runner'):
        assert hashlib.sha256((root / prerequisites[key]).read_bytes()).hexdigest() == prerequisites[key + '_sha256'], key
    assert hashlib.sha256((root / 'tools/pen-capture/airbrush_v1_tool1_capture.py').read_bytes()).hexdigest() == prerequisites['capture_script_replay_sha256']
    for name, expected in receipt['cases'].items():
        raw = (directory / f'{name}.json').read_bytes()
        assert hashlib.sha256(raw).hexdigest() == receipt['capture_sha256'][name], name
        lines = raw.decode().splitlines()
        frame = json.loads(next(x.removeprefix('FRAME_STATE ') for x in lines if x.startswith('FRAME_STATE ')))['post_frame']
        result = json.loads(lines[-1])
        assert result['selected_profile'] == 1 and result['model_state'] == expected['model_state']
        assert result['source_arrays'] == expected['source_arrays']
        assert result['producer_events'][-1]['after_bytes'] == expected['vector_hex']
        assert zlib.decompress(base64.b64decode(frame['data_base64'])) == zlib.decompress(base64.b64decode(expected['frame']['data_base64']))
    print('Both raw captures match the compact receipt.')
elif mode in ('mupdf', 'pdfium'):
    if mode == 'mupdf':
        import pymupdf
        assert pymupdf.VersionBind == '1.28.2'
    else:
        import pypdfium2
        assert str(pypdfium2.PDFIUM_INFO) == '150.0.7869.0'
    for name, case in receipt['cases'].items():
        raw = zlib.decompress(base64.b64decode(case['frame']['data_base64']))
        native = b''.join(reversed([raw[i:i + 1024] for i in range(0, len(raw), 1024)]))
        for variant in ('candidate', 'generic'):
            filename = f'{name}-{variant}.pdf'
            path = directory / filename
            assert hashlib.sha256(path.read_bytes()).hexdigest() == receipt['pdf_sha256'][filename], filename
            if mode == 'mupdf':
                doc = pymupdf.open(path)
                pix = doc[0].get_pixmap(matrix=pymupdf.Matrix(96 / 72, 96 / 72), alpha=True)
                assert (pix.width, pix.height, pix.n) == (256, 256, 4)
                actual = sum(abs(a-b) for a,b in zip(pix.samples, native))
                expected = receipt['comparison']['pymupdf_1.28.2_rgba_abs_error'][name][variant]
            else:
                with pypdfium2.PdfDocument(str(path)) as doc:
                    page = doc[0]
                    alpha = page.render(scale=4/3, rev_byteorder=True, fill_color=(0,0,0,0)).to_pil().convert('RGBA')
                    white = page.render(scale=4/3, rev_byteorder=True, fill_color=(255,255,255,255)).to_pil().convert('RGB')
                    assert alpha.size == white.size == (256, 256)
                    native_white = bytes(min(255, native[i+c] + 255-native[i+3]) for i in range(0, len(native), 4) for c in range(3))
                    actual = [sum(abs(a-b) for a,b in zip(alpha.tobytes()[3::4], native[3::4])), sum(abs(a-b) for a,b in zip(white.tobytes(), native_white))]
                expected = receipt['comparison']['pdfium_150_alpha_white_rgb_abs_error'][name][variant]
            assert actual == expected, (filename, actual, expected)
            print(f'{filename}: {actual}')
else:
    raise SystemExit('mode must be capture, mupdf or pdfium')
