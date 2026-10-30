"""Capture Pencil2 endpoint pressure controls at zero tolerance."""
import hashlib
import json
import os
import shutil
from pathlib import Path
import subprocess
import tempfile

root = Path('/home/jding/RustroverProjects/sdocx')
here = root / 'tools/pencil2-serialized-bridge'
source = json.loads((root / 'conformance/fountain-v14-saved-scene-inputs.json').read_text())
# Exercise short, boundary-length, and reversed endpoint segments.
cases = {'terminal-short': [80, 90, 100, 110, 120, 130, 140, 140.1], 'terminal-long': [80, 90, 100, 110, 120, 130, 140, 145.0], 'terminal-reverse': [140, 130, 120, 110, 100, 90, 80, 79.7]}
sha = lambda b: hashlib.sha256(b).hexdigest()
with tempfile.TemporaryDirectory(prefix='sdocx-pencil2-boundary-') as d:
    tmp = Path(d)
    rows = []
    for name, values in cases.items():
        data = json.loads(json.dumps(source))
        stroke = data['strokes'][0]['stroke']
        # Use matching source channels and spread later points to paint geometry.
        xs = values
        data['strokes'][0]['point_count'] = len(xs)
        for field in ('pressures', 'timestamps', 'tilts', 'orientations'):
            stroke[field] = stroke[field][:len(xs)]
        stroke['points'] = [{'x': x, 'y': 100.0} for x in xs]
        stroke['pressures'] = [0.8] * (len(xs)-1) + [0.0]
        path = tmp / f'{name}.json'
        path.write_text(json.dumps(data))
        fresh = (here / 'fresh-object-control.py').read_text()
        fresh = fresh.replace("inputs = root / 'conformance/fountain-v14-saved-scene-inputs.json'", f'inputs = Path({str(path)!r})')
        fresh = fresh.replace("assert compact['geometry_sha256'] == 'de3cf9e4c6a832385e68daf4e99bd0b2c9174a2e576f9567e6c5f5f6d2d286d0'", '')
        fresh = fresh.replace("assert compact['geometry_bytes'] == 11648", '')
        fresh = fresh.replace("first = zlib.decompress", f"Path({str(tmp / (name + '.vbo'))!r}).write_bytes((scratch / 'second.vbo').read_bytes())\nfirst = zlib.decompress")
        with tempfile.NamedTemporaryFile(mode='w', suffix='.py', dir=here) as script:
            script.write(fresh); script.flush()
            env = os.environ.copy(); env['SDOCX_PENCIL2_FRESH_FRAME_DIR'] = str(tmp)
            proc = subprocess.run(['python3', script.name], cwd=root, env=env, text=True, capture_output=True)
            if proc.returncode:
                raise RuntimeError(f'{name}: {proc.stderr[-5000:]} {proc.stdout[-2000:]}')
        geometry = (tmp / f'{name}.vbo').read_bytes()
        rows.append({'case':name, 'initial_x':values, 'input_sha256':sha(path.read_bytes()), 'geometry_bytes':len(geometry), 'geometry_sha256':sha(geometry), 'native_frame_sha256':sha((tmp / 'second.rgba').read_bytes())})
    import struct
    for row in rows:
        b = (tmp / (row['case'] + '.vbo')).read_bytes()
        (root / 'crates/sdocx/src/ink/pencil2/assets' / (row['case'] + '.vbo')).write_bytes(b)
        print(row['case'], [struct.unpack_from('<f', b, 56*i+16)[0] for i in range(len(b)//56-4, len(b)//56)])
    dependencies = [Path(__file__), here / 'fresh-object-control.py', here / 'capture.py', root / 'conformance/fountain-v14-saved-scene-inputs.json']
    result = {'scope': 'Fresh setter-built Pencil2 V14, zero tolerance, deferred Drawing, ARM64 emulation and host Mesa; a separate deserialized object is discarded. Descriptor captured before Update; GPU submission not independently verified. No device or saved-note parity.', 'cases': rows, 'dependencies_sha256': {str(p.relative_to(root)): sha(p.read_bytes()) for p in dependencies}}
    (here / 'terminal-variants-receipt.json').write_text(json.dumps(result, indent=2) + '\n')
