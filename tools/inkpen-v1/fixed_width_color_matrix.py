import argparse, base64, hashlib, json, os, struct, subprocess, zlib
from pathlib import Path

root = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser()
parser.add_argument('output', type=Path)
args = parser.parse_args()
cases = []
for color, width in [(0xff336699, 6), (0x80d04721, 12), (0x20a030e0, 3)]:
    env = os.environ.copy()
    env.update(PYTHONPATH='scratch/apk-analysis-runtime/python:scratch/straightinkpen2-alias', INKPEN_COLOR=hex(color), FIXED_WIDTH=str(width), TOOL='1', CURVE='1', INKPEN_CAPTURE_SCALE='1', INKPEN_CAPTURE_BENT='0', INKPEN_CAPTURE_PHASE='0')
    output = subprocess.check_output(['python3', 'tools/inkpen-v1/fixed_width_color_capture.py', '--print-capture'], cwd=root, env=env, text=True)
    capture = json.loads(output[output.index('\n{') + 1:])
    caller, readback = capture['source_caller'], capture['target']['readback']
    rgba = zlib.decompress(base64.b64decode(readback['fbo4_rgba8_zlib_base64']))
    assert hashlib.sha256(rgba).hexdigest() == readback['fbo4_rgba8_sha256']
    assert capture['target']['viewport'] == [0, 0, 128, 64]
    assert readback['fbo4_read']['dimensions'] == [128, 64]
    assert len(rgba) == 128 * 64 * 4
    assert caller['input']['tool_type'] == 1 and caller['input']['curve_enabled'] is True
    for getter in caller['native_calls']['ObjectStroke_getter_snapshots']:
        assert getter['color_argb'] == color and getter['tool_type'] == 1 and getter['curve_enabled'] == 1
        assert getter['fixed_width_enabled'] == 1
        assert getter['fixed_width_bits'] == struct.pack('<f', width).hex()
    cases.append({'color':color, 'width':width, 'tool':1, 'curve':True, 'input':caller['input'], 'streams':caller['streams'], 'rgba_sha256':hashlib.sha256(rgba).hexdigest(), 'rgba_zlib_base64':readback['fbo4_rgba8_zlib_base64']})
    print(hex(color), flush=True)
output = args.output
with output.open('x') as f:
    json.dump({'schema':'sdocx-inkpen-v1-fixed-width-color-native-v1', 'scope':'Synthetic ObjectStroke native Drawing and host Mesa; not device or saved-note parity', 'cases':cases, 'libraries':caller['libraries'], 'renderer_library_sha256':capture['renderer_library_sha256'], 'helper_hashes':capture['helper_hashes']}, f, indent=2)
    f.write('\n')
