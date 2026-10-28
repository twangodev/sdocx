"""Replay the retained Marker4 RTV4 host harness at thin and positive-control sizes."""
import hashlib
import subprocess
import sys
import tempfile
from pathlib import Path

source = Path('scratch/defaultpen-native/marker4_native_capture_full_tool2.py')
raw = source.read_bytes()
assert hashlib.sha256(raw).hexdigest() == '187ccdb582218ea898d1fbe7b536e25bd8241bc2518ed7efc6110ba84cb9851f'
original = raw.decode()
old_matrix = "p.put(mat,'16f',1,0,0,0,0,-1,0,0,0,0,1,0,w/2-cx,h/2+cy,0,1)"
assert original.count(old_matrix) == 1
assert original.count('[36.21])') == 1
for size, scale in [(0.4, 1), (0.4, 2), (0.4, 4), (2.0, 1)]:
    with tempfile.TemporaryDirectory(prefix='marker4-thin-') as directory:
        root = Path(directory)
        matrix = f"p.put(mat,'16f',{scale},0,0,0,0,-{scale},0,0,0,0,1,0,w/2-{scale}*cx,h/2+{scale}*cy,0,1)"
        script = original.replace(old_matrix, matrix).replace('[36.21])', f'[{size}])')
        script = script.replace("'set_pen_size_f32':36.21", f"'set_pen_size_f32':{size}")
        script = script.replace('/home/jding/RustroverProjects/sdocx/scratch/defaultpen-native/fresh-marker4-', str(root / 'capture-'))
        path = root / 'runner.py'
        path.write_text(script)
        subprocess.run([sys.executable, str(path), '--capture-case', 'full-real', '--capture-output', str(root / 'output.json')], check=True, stdout=subprocess.DEVNULL)
        pixels = (root / 'capture-stroke35.rgba').read_bytes()
        alpha = pixels[3::4]
        print(size, scale, sum(x != 0 for x in alpha), max(alpha), hashlib.sha256(pixels).hexdigest())
