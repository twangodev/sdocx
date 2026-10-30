import argparse, base64, hashlib, json, re, subprocess, tempfile, zlib
from pathlib import Path
from importlib.metadata import version
import pymupdf, pypdfium2
from PIL import Image

root = Path(__file__).resolve().parents[2]
fixture_path = root / 'conformance/inkpen-v1-fixed-width-color-native.json'
fixture = json.loads(fixture_path.read_text())
parser = argparse.ArgumentParser()
parser.add_argument('exports', type=Path)
parser.add_argument('output', type=Path)
args = parser.parse_args()
exports = args.exports
results = []
# A 16x16 bottom-right patch is clear in each native control and both exports.
patch = [(x, y) for y in range(48, 64) for x in range(112, 128)]
with tempfile.TemporaryDirectory(prefix='sdocx-v1-color-compare-') as directory:
    tmp = Path(directory)
    for index, case in enumerate(fixture['cases']):
        native = zlib.decompress(base64.b64decode(case['rgba_zlib_base64']))
        assert len(native) == 128 * 64 * 4
        assert hashlib.sha256(native).hexdigest() == case['rgba_sha256']
        target = bytes(min(255, native[i+c] + round(252 * (255-native[i+3])/255)) for i in range(0,len(native),4) for c in range(3))
        assert all(native[(y * 128 + x) * 4:(y * 128 + x + 1) * 4] == b'\0\0\0\0' for x, y in patch), 'native background patch must be clear'
        row = {'color':case['color'], 'width':case['width'], 'native_sha256':case['rgba_sha256']}
        for label in ('candidate','generic'):
            svg_path = exports / f'{index}-{label}.svg'
            svg_bytes = svg_path.read_bytes()
            svg = svg_bytes.decode('utf-8')
            svg, count = re.subn(r'<rect fill="#fcfcfc"[^>]*/>', '', svg)
            assert count == 1
            (tmp/'probe.svg').write_text(svg)
            subprocess.run(['resvg', str(tmp/'probe.svg'), str(tmp/'probe.png')],check=True)
            image = Image.open(tmp/'probe.png').convert('RGBA')
            assert image.size == (128, 64)
            page = Image.new('RGBA', image.size, (252,252,252,255)); page.alpha_composite(image)
            def error(data):
                assert len(data) == len(target) == 128 * 64 * 3
                return sum(abs(a-b) for a,b in zip(target,data, strict=True))
            pdf = exports/f'{index}-{label}.pdf'
            pix = pymupdf.open(pdf)[0].get_pixmap(matrix=pymupdf.Matrix(4/3,4/3), alpha=False, colorspace=pymupdf.csRGB)
            bitmap = pypdfium2.PdfDocument(pdf)[0].render(scale=4/3,rev_byteorder=True)
            assert (pix.width, pix.height, pix.n, pix.stride) == (128, 64, 3, 128 * 3)
            assert (bitmap.width, bitmap.height, bitmap.n_channels, bitmap.stride) == (128, 64, 3, 128 * 3)
            def reader_relative(data):
                assert len(data) == 128 * 64 * 3
                samples = [data[(y * 128 + x) * 3:(y * 128 + x + 1) * 3] for x, y in patch]
                background = samples[0]
                assert all(pixel == background for pixel in samples), 'PDF background patch must be uniform'
                adjusted = bytes(min(255, native[i+c] + round(background[c] * (255-native[i+3])/255)) for i in range(0,len(native),4) for c in range(3))
                return {'background_rgb': list(background), 'rgb_error': sum(abs(a-b) for a,b in zip(adjusted,data,strict=True))}
            row[label]={'svg_sha256':hashlib.sha256(svg_bytes).hexdigest(), 'pdf_sha256':hashlib.sha256(pdf.read_bytes()).hexdigest(), 'svg_rgb_error':error(page.convert('RGB').tobytes()),'mupdf_rgb_error':error(pix.samples),'pdfium_rgb_error':error(bytes(bitmap.buffer)), 'mupdf_reader_relative':reader_relative(pix.samples), 'pdfium_reader_relative':reader_relative(bytes(bitmap.buffer))}
        for reader in ('mupdf_reader_relative', 'pdfium_reader_relative'):
            assert row['candidate'][reader]['background_rgb'] == row['generic'][reader]['background_rgb']
        for metric in ('svg_rgb_error', 'mupdf_rgb_error', 'pdfium_rgb_error'):
            assert row['candidate'][metric] < row['generic'][metric], (index, metric, row)
        results.append(row)
        print(row)
output=args.output
with output.open('x') as f:
    json.dump({'schema':'sdocx-inkpen-v1-fixed-width-color-export-comparison-v1','scope':'Synthetic host Mesa and public SVG/direct PDF over #fcfcfc; reader-relative composites use each PDF reader rendered background and do not correct stroke color conversion; device and saved-note parity unverified', 'comparator_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'fixture_sha256':hashlib.sha256(fixture_path.read_bytes()).hexdigest(), 'background_patch':{'x':112, 'y':48, 'width':16, 'height':16},'readers':{'resvg':subprocess.check_output(['resvg', '--version'], text=True).strip(), 'pillow':version('pillow'), 'pymupdf':version('pymupdf'), 'pypdfium2':str(pypdfium2.PYPDFIUM_INFO), 'pdfium':str(pypdfium2.PDFIUM_INFO)},'cases':results},f,indent=2); f.write('\n')
