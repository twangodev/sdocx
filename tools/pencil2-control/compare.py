"""Replay retained native frames through the isolated public canvas and readers."""
import json, os, struct, subprocess, tempfile, zlib
from pathlib import Path
import pymupdf, pypdfium2
from PIL import Image
root = Path(__file__).resolve().parents[2]
here = Path(__file__).resolve().parent
native = json.loads((here / 'native-receipt.json').read_text())
offset = [struct.unpack('<I', struct.pack('<f', x))[0] for x in [c for c in native['uniforms'] if c[:3] == ['glUniform2fv', 4, 1]][-1][3]]
with tempfile.TemporaryDirectory(prefix='pencil2-control-') as temp:
    d = Path(temp)
    for name in ('first', 'second'):
        (d / (name + '.rgba')).write_bytes(zlib.decompress((here / (name + '.rgba.zlib')).read_bytes()))
    env = {**os.environ, 'CARGO_TARGET_DIR': '/tmp/sdocx-agent-target', 'CARGO_INCREMENTAL': '0', 'CARGO_PROFILE_DEV_DEBUG': '0', 'CARGO_PROFILE_TEST_DEBUG': '0', 'CARGO_BUILD_JOBS': '2', 'SDOCX_PENCIL2_PRIOR': str(d / 'first.rgba'), 'SDOCX_PENCIL2_OUTPUT': str(d / 'canvas.rgba'), 'SDOCX_PENCIL2_EXPORT_DIR': str(d)}
    run = subprocess.run(['cargo', 'run', '--offline', '--quiet', '--manifest-path', str(root / 'tools/pencil2-public-replay/Cargo.toml'), '--features', 'export-pdf', '--bin', 'changed_input_compare'], cwd=root, env=env, check=True, capture_output=True, text=True)
    line = next(x for x in run.stdout.splitlines() if x.startswith('computed offset bits: '))
    assert [int(x,16) for x in line.removeprefix('computed offset bits: ').split()] == offset
    subprocess.run(['resvg', str(d/'canvas.svg'), str(d/'svg.png')], check=True)
    raw=(d/'second.rgba').read_bytes(); expected=b''.join(raw[i:i+768] for i in range(len(raw)-768,-1,-768))
    frames={'canvas':(d/'canvas.rgba').read_bytes(), 'png':Image.open(d/'canvas.png').convert('RGBA').tobytes(), 'svg_resvg':Image.open(d/'svg.png').convert('RGBA').tobytes()}
    with pymupdf.open(d/'canvas.pdf') as pdf: frames['pdf_mupdf']=pdf[0].get_pixmap(alpha=True,colorspace=pymupdf.csRGB).samples
    with pypdfium2.PdfDocument(str(d/'canvas.pdf')) as pdf: frames['pdf_pdfium']=pdf[0].render(scale=1,rev_byteorder=True,fill_color=(0,0,0,0)).to_pil().convert('RGBA').tobytes()
    for name, actual in frames.items():
        assert len(actual)==len(expected)
        differences=sum(a != b for a,b in zip(expected,actual))
        print(f'{name}: {differences} differing channels')
        assert differences == 0
