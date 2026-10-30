import base64, hashlib, json, os, struct, subprocess, sys, zlib
from pathlib import Path

output = Path(sys.argv[1])
if output.exists():
    raise FileExistsError(output)
root = Path(__file__).resolve().parents[2]
cases = []
for color, width, tool, curve, scale, shape in [(0xff336699, 6, 1, 1, 5, 'bent'), (0x80d04721, 12, 2, 1, 8, 'phase'), (0x20a030e0, 3, 0, 0, 6, 'bent'), (0x0019aa44, 6, 1, 1, 7, 'straight')]:
    env = os.environ.copy()
    env.update(PYTHONPATH='scratch/apk-analysis-runtime/python:scratch/straightinkpen2-alias', INKPEN_COLOR=hex(color), FIXED_WIDTH=str(width), TOOL=str(tool), CURVE=str(curve), INKPEN_CAPTURE_SCALE=str(scale), INKPEN_CAPTURE_BENT=str(int(shape == 'bent')), INKPEN_CAPTURE_PHASE=str(int(shape == 'phase')))
    text = subprocess.check_output(['python3', 'tools/inkpen-v1/fixed_width_color_capture.py', '--print-capture'], cwd=root, env=env, text=True)
    capture = json.loads(text[text.index('\n{') + 1:])
    caller, readback = capture['source_caller'], capture['target']['readback']
    rgba = zlib.decompress(base64.b64decode(readback['fbo4_rgba8_zlib_base64']))
    assert len(rgba) == 128 * 64 * scale * scale * 4
    assert hashlib.sha256(rgba).hexdigest() == readback['fbo4_rgba8_sha256']
    getters = caller['native_calls']['ObjectStroke_getter_snapshots']
    assert getters, 'native getter snapshots must not be empty'
    for getter in getters:
        assert getter['color_argb'] == color and getter['tool_type'] == tool and getter['curve_enabled'] == curve
        assert getter['fixed_width_enabled'] == 1
        assert getter['fixed_width_bits'] == struct.pack('<f', width).hex()
    cases.append({'color':color, 'width':width, 'tool':tool, 'curve':bool(curve), 'scale':scale, 'shape':shape, 'input':caller['input'], 'streams':caller['streams'], 'rgba_sha256':hashlib.sha256(rgba).hexdigest(), 'rgba_zlib_base64':readback['fbo4_rgba8_zlib_base64']})
    print(hex(color), scale, shape, flush=True)
with output.open('x') as f:
    json.dump({'schema':'sdocx-inkpen-v1-fixed-width-high-scale-color-native-v1', 'scope':'Synthetic ObjectStroke native Drawing and host Mesa; not device or saved-note parity', 'cases':cases, 'libraries':caller['libraries'], 'renderer_library_sha256':capture['renderer_library_sha256'], 'helper_hashes':capture['helper_hashes']}, f, indent=2)
    f.write('\n')
