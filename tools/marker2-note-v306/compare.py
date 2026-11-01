"""Compare single-stroke public SVG and direct PDF with retained native paint."""
import json, os, pathlib, struct, subprocess, sys, tempfile
import numpy as np
from PIL import Image
import pymupdf, pypdfium2

here = pathlib.Path(__file__).resolve().parent
receipt = json.loads((here / 'receipt.json').read_text())
native = np.asarray(Image.open(here / 'native.png').convert('RGBA'), dtype=np.int32)
def decode(x): return struct.unpack('<f', bytes.fromhex(x))[0]
def encode(x): return struct.pack('<f', x).hex()
inputs = receipt['input'].copy()
dx, dy = receipt['native_paint']['translation_f32']
inputs['points_f32_bits'] = [[encode(decode(x) + dx/2), encode(128 - dy/2 - decode(y))] for x,y in inputs['points_f32_bits']]
with tempfile.TemporaryDirectory(prefix='marker2-note-exports-') as tmp:
    p = pathlib.Path(tmp)
    (p/'input.json').write_text(json.dumps({'cases': {'test': {'input': inputs}}}))
    for mode in ('baseline', 'profile'):
        env = dict(os.environ, MARKER2_CANDIDATE='2' if mode == 'profile' else '')
        subprocess.run([sys.argv[1], str(p)], env=env, check=True, stdout=subprocess.DEVNULL)
        svg, pdf, png = p/'case-1.svg', p/'case-1-direct.pdf', p/'case-1.png'
        subprocess.run(['resvg', '--zoom', '2', svg, png], check=True)
        for reader in ('resvg', 'mupdf', 'pdfium'):
            if reader == 'resvg': pixels = np.asarray(Image.open(png).convert('RGB'), dtype=np.int32)[:256,:256]
            elif reader == 'mupdf':
                with pymupdf.open(pdf) as document:
                    pix = document[0].get_pixmap(matrix=pymupdf.Matrix(8/3,8/3), colorspace=pymupdf.csRGB, alpha=False)
                    pixels = np.frombuffer(pix.samples, dtype=np.uint8).reshape(pix.height,pix.width,3).astype(np.int32)[:256,:256]
            else:
                with pypdfium2.PdfDocument(pdf) as document:
                    pixels = np.asarray(document[0].render(scale=8/3).to_pil().convert('RGB'), dtype=np.int32)[:256,:256]
            expected = native[:,:,:3] + np.rint(pixels[0,0] * (255-native[:,:,3,None])/255).astype(np.int32)
            error = int(np.abs(pixels - expected).sum())
            assert error == receipt['rgb_l1'][mode][reader], (mode, reader, error)
            print(mode, reader, error)
