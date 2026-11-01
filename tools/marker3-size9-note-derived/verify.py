import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[2]
case = sys.argv[3]
receipt = sys.argv[4] if len(sys.argv) > 4 else 'comparison.json'
expected = json.loads((root / 'tools/marker3-size9-note-derived/native-receipt.json').read_text())
assert hashlib.sha256(Path(sys.argv[1]).read_bytes()).hexdigest() == expected[case[-1]]['rgba_sha256']
actual = json.loads(subprocess.check_output([sys.executable, str(root / 'tools/marker3-size9-diverse/compare.py'), *sys.argv[1:4]], env=os.environ, text=True))
assert actual == json.loads((root / f'tools/marker3-size9-note-derived/{receipt}').read_text())[case]
print(case, 'verified')
