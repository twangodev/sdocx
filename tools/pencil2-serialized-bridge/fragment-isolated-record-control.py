"""Isolate the second native draw on an empty transparent framebuffer."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

here = Path(__file__).resolve().parent
base_path = here / 'fragment-nan-control.py'
base = base_path.read_text()
# Substitute only the first uploaded instance with record 30; keep the remaining
# upload and size intact. The draw is truncated to one instance below.
needle_upload = "  host=C.create_string_buffer(raw) if raw is not None else None"
replacement_upload = "  if raw is not None and size == 207*56:\n   assert hashlib.sha256(raw).hexdigest() == 'cb697a69d16b2e8d070c359de7f48f747f8b33c12fa87e13252c3c2a65aa8c09'\n   Path(os.environ['RECORD']).write_bytes(raw[29*56:30*56])\n   raw=raw[29*56:30*56]+raw[56:]\n" + needle_upload
inject_upload = "assert code.count(" + repr(needle_upload) + ") == 1\n    code = code.replace(" + repr(needle_upload) + ", " + repr(replacement_upload) + ")\n    "
base = base.replace("    needle_shader = ", "    " + inject_upload + "needle_shader = ")
base = base.replace("and uploads[0][5]==row['geometry_sha256']", "")

anchor = 'original = source.read_text()'
needle = "   target['independent_second']['stage']='draw'\n"
replacement = "   clear_color=(C.c_float*4)(); ip.cfunc(p.mesa.gl,'glGetFloatv',None,[C.c_uint,C.POINTER(C.c_float)])(0x0c22,clear_color); f['glClearColor'](*[float(v) for v in os.environ['BG'].split(',')]); f['glClear'](0x4000); empty=C.create_string_buffer(192*192*4); f['glReadPixels'](0,0,192,192,0x1908,0x1401,empty); assert len(set(empty.raw[i:i+4] for i in range(0,len(empty.raw),4))) == 1; Path(os.environ['EMPTY']).write_bytes(empty.raw); f['glClearColor'](*clear_color)\n" + needle
injection = f'\nassert original.count({needle!r}) == 1\noriginal = original.replace({needle!r}, {replacement!r})'
marker = "value=float(os.environ.get('TOL', '0.2'))"
assert base.count(marker) == 1
base = base.replace(marker, f'code=code.replace(anchor, anchor + {injection!r})\n' + marker)
base = base.replace("assert d['first_sha256']=='0f0cbf122a9484d1760f1fae7f260f51552e4c6bee05a8016d032e833385e923'", '')
base = base.replace("'draw_calls':draws,", "'uniforms':[x for x in d['uniforms'] if x[0]=='glUniform2fv'], 'draw_calls':draws,")
base = base.replace('import shutil\nshutil.rmtree(scratch)', "Path(os.environ['FRAME']).write_bytes(raw)\nimport shutil\nshutil.rmtree(scratch)")
original = 'FragColor = isnan(pLen) ? vec4(1,0,0,1) : vec4(0,1,0,1); return;'
assert base.count(original) == 1
modes = {'native': None, 'red': original, 'one': 'FragColor = vec4(fragColor.xyz,1.0); return;', 'zero': 'FragColor = vec4(fragColor.xyz,0.0); return;'}
rows = {}
with tempfile.TemporaryDirectory(prefix='pencil-nan-empty-') as d:
    tmp = Path(d)
    for prefix in (1,):
        for mode, expression in modes.items():
            code = base
            if expression is None:
                code = code.replace('source=[x.replace(marker,marker+diag) for x in source]', 'source=source')
            else:
                code = code.replace(original, expression)
            script, frame = tmp / f'{prefix}-{mode}.py', tmp / f'{prefix}-{mode}.rgba'
            script.write_text(code)
            proc = subprocess.run(['python3', str(script)], env={**os.environ, 'PREFIX': str(prefix), 'FRAME': str(frame), 'EMPTY': str(tmp / f'{prefix}-{mode}-empty.rgba'), 'BG': os.environ.get('BG', '0,0,0,0'), 'RECORD': str(tmp / f'{prefix}-{mode}.bin')}, capture_output=True, text=True)
            if proc.returncode:
                raise RuntimeError((proc.stdout + proc.stderr)[-5000:])
            row = json.loads(proc.stdout)
            prior = (tmp / f'{prefix}-{mode}-empty.rgba').read_bytes()
            record = (tmp / f'{prefix}-{mode}.bin').read_bytes()
            assert len(prior) == 192 * 192 * 4 and len(record) == 56
            if mode != 'native':
                assert prior == (tmp / f'{prefix}-native-empty.rgba').read_bytes()
                assert record == (tmp / f'{prefix}-native.bin').read_bytes()
            raw = frame.read_bytes()
            assert len(raw) == 192 * 192 * 4
            rows[f'{prefix}-{mode}'] = (row, raw)
    cases = {}
    for prefix in (1,):
        prior_sha = hashlib.sha256((tmp / '1-native-empty.rgba').read_bytes()).hexdigest()
        record = (tmp / '1-native.bin').read_bytes()
        record_sha = hashlib.sha256(record).hexdigest()
        (here / 'fragment-isolated-record.bin').write_bytes(record)
        import zlib
        label = os.environ.get('LABEL', 'transparent')
        (here / ('fragment-isolated-record-' + label + '.rgba.zlib')).write_bytes(zlib.compress(rows['1-native'][1]))
        (here / ('fragment-isolated-record-' + label + '-prior.rgba.zlib')).write_bytes(zlib.compress((tmp / '1-native-empty.rgba').read_bytes()))
        native = rows[f'{prefix}-native'][1]
        cases[str(prefix)] = {}
        for mode in modes:
            row, raw = rows[f'{prefix}-{mode}']
            assert row['upload_sha256'] == rows[f'{prefix}-native'][0]['upload_sha256']
            changed = [[i // 4 % 192, i // 4 // 192, list(native[i:i+4]), list(raw[i:i+4])] for i in range(0, len(raw), 4) if native[i:i+4] != raw[i:i+4]]
            cases[str(prefix)][mode] = {'uniforms': row['uniforms'], 'pixel': row['rgba'], 'sha256': hashlib.sha256(raw).hexdigest(), 'different_pixels': len(changed), 'sample': changed[:8], 'gl_errors': row['gl_errors'], 'upload_sha256': row['upload_sha256']}
receipt = {'scope': 'Isolated 30th instance substituted at upload, synthetic remapped Pencil2 tolerance 0.2, ARM64 native Drawing on host Mesa; clear target to a uniform background after first stroke and immediately before second draw. Shader diagnostics restricted to target fragment and can alter compilation.', 'pixel_bottom_origin': [129, 150], 'background': os.environ.get('BG', '0,0,0,0'), 'prior_readback_sha256': prior_sha, 'record_sha256': record_sha, 'cases': cases, 'limitations': 'Draw retains reused renderer state; only one geometry, shader target and host Mesa context. Clearing establishes a uniform destination before the second draw, the second draw contains exactly one instance, substituted at upload. Instrumented recompilation may change arithmetic. No device or saved-note parity.', 'script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'base_sha256': hashlib.sha256(base_path.read_bytes()).hexdigest(), 'transitive_source_sha256': {str(p.relative_to(here.parents[1])): hashlib.sha256(p.read_bytes()).hexdigest() for p in [here / 'capture.py', here.parents[1] / 'tools/pencil2-independent-drawing/capture.py', here.parents[1] / 'tools/pencil2-mixed-drawing/capture.patch', here.parents[1] / 'tools/pencil2-public-replay/drawing_native_target_translated_probe.py']}}
(here / ('fragment-isolated-record-control-' + os.environ.get('LABEL', 'transparent') + '-receipt.json')).write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(receipt, indent=2))
