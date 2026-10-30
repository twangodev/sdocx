#!/usr/bin/env python3
"""Recreate a two-draw native RT experiment on one host framebuffer."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[2]
here = Path(__file__).resolve().parent
with tempfile.TemporaryDirectory(prefix='marker3-two-color-') as tmp:
    work = Path(tmp)
    subprocess.run([sys.executable, str(root/'tools/marker3-terminal-size85/capture.py'), str(work/'base')], cwd=root, check=True)
    source = (work/'base/phase2-tool1/join.py').read_text()
    call = "p.call('SPen::Marker3StrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[rt,bitmap,mat],limit=5_000_000)"
    assert source.count(call) == 1
    red = "p.put(rt+44,'4f',0.8,0.2,0.1019607843,0.5019607843)"
    green = "p.put(rt+44,'4f',0.2,0.8,0.33333334,0.5019608)"
    matrix = "p.put(mat,'16f',2,0,0,0,0,2,0,0,0,0,1,0,12,8,0,1)"
    env = dict(os.environ, PYTHONPATH=':'.join(str(root/x) for x in ('scratch/apk-analysis-runtime/python','tmp/conformance-cleanup/native-capture-a241793/conformance')))
    for name in ('forward','reverse'):
        case = work/name
        case.mkdir()
        before = red+'\n' if name == 'reverse' else ''
        second = green if name == 'reverse' else red
        text = source.replace(call, before+call+'\n'+second+'\n'+matrix+'\n'+call)
        script = case/'join.py'
        script.write_text(text)
        with (case/'run.log').open('w') as log:
            subprocess.run([sys.executable, str(script)], cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        data = (case/'drawing-paint-positive.rgba').read_bytes()
        expected = json.loads((here/f'{name}.json').read_text())
        report = json.loads((case/'drawing-paint-positive.json').read_text())
        assert hashlib.sha256(data).hexdigest() == expected['sha256']
        assert report['draws'] == expected['draws']
        assert [x for x in report['uniform_calls'] if x['call']=='glUniform4fv'] == expected['color_uniforms']
        assert not any(x['error'] for x in report['gl_error_checks'])
        print(name, expected['sha256'], flush=True)
