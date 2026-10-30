"""Verify retained saved stroke and native redraw stamps against the source note."""
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from native_geometry import NativeMarker4

root = Path('tools/marker4_v7/saved-v197')
source = json.loads((root / 'source.json').read_bytes())['cases'][0]
note = Path('tmp/stroke-conformance/quiz.sdocx')
assert hashlib.sha256(note.read_bytes()).hexdigest() == '38fd0ef0729d3a113e1c14bcc10557dcc263e5a3582fd80a3cf99c8c2c4ad40a'
with tempfile.TemporaryDirectory() as tmp:
    output = Path(tmp) / 'geometry.json'
    with output.open('wb') as f:
        subprocess.run([sys.argv[1], str(note)], stdout=f, check=True)
    stroke = json.loads(output.read_bytes())[69]['stroke']
assert stroke == source['stroke']
assert stroke['rendering']['tool_type_raw'] == 2
assert stroke['rendering']['advanced_settings'] == '7;'
assert stroke['pen_width'] == source['size']
assert stroke['rendering']['style']['color_argb'] == 1932760831
objdump = shutil.which('aarch64-linux-gnu-objdump') or 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'
native = NativeMarker4(Path('scratch/apk-analysis-native/arm64-v8a'), Path(objdump))
actual = native.render(stroke)
assert actual['dots'] == source['dots']
print(json.dumps({'stroke_index': 69, 'samples': len(stroke['points']), 'native_stamps': len(actual['dots']), 'native_sample_ends': len(actual['sample_ends']), 'source_sha256': hashlib.sha256((root / 'source.json').read_bytes()).hexdigest()}))
