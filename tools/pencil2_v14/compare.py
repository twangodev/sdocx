"""Measure Rust-emitted SVG and scene PDF controls with local readers.

Install PyMuPDF and pypdfium2 in the Python environment before running. This
script replaces the paired receipt; run it in an isolated checkout to retain
the historical measurements.
"""
import hashlib
import json
from pathlib import Path
import pymupdf
import pypdfium2

root = Path(__file__).resolve().parents[2]
directory = root / 'scratch/pencil2-mixed-rgb/exports'
receipt = json.loads((root / 'conformance/pencil2-v14-mixed-rgb-native.json').read_text())

def native(name):
    frame = receipt['results'][name]
    image = bytearray(frame['background']*(192*192))
    for x,y,*rgba in frame['pixels']:
        offset = ((191-y)*192+x)*4
        image[offset:offset+4] = bytes(rgba)
    return bytes(image)

def metrics(image, target):
    assert len(image) == len(target) == 192*192*4
    errors = [abs(a-b) for a,b in zip(image,target)]
    return {'different_channel_bytes':sum(e!=0 for e in errors),'absolute_sum':sum(errors),'maximum':max(errors)}

def pdf_pixels(path, reader):
    if reader == 'mupdf':
        with pymupdf.open(path) as document:
            pixmap = document[0].get_pixmap(alpha=True,colorspace=pymupdf.csRGB)
            assert (pixmap.width,pixmap.height,pixmap.n) == (192,192,4)
            return pixmap.samples
    with pypdfium2.PdfDocument(str(path)) as document:
        bitmap = document[0].render(scale=1,rev_byteorder=True,fill_color=(0,0,0,0))
        image = bitmap.to_pil().convert('RGBA')
        assert image.size == (192,192)
        raw = image.tobytes()
        return bytes((v*raw[i-i%4+3]+127)//255 if i%4!=3 else v for i,v in enumerate(raw))

results = {}
for path in sorted(directory.glob('*.resvg.rgba')):
    name = path.name.removesuffix('.resvg.rgba')
    reference = native('colored_white' if name=='generic_colored_white' else name.removesuffix('_seeded'))
    results[name] = {'resvg':metrics(path.read_bytes(),reference)}
    for kind in ['direct','svg']:
        pdf = directory / f'{name}.{kind}.pdf'
        if not pdf.exists(): continue
        for reader in ['mupdf','pdfium']:
            results[name][kind+'_'+reader] = metrics(pdf_pixels(pdf,reader),reference)
expected = {'generic_colored_white'} | {name+suffix for name in ['colored','mixed','mixed_reversed'] for suffix in ['','_white','_paper','_white_seeded','_paper_seeded']}
if set(results) != expected:
    raise SystemExit('Incomplete Rust export set; regenerate render::pencil2_tests artifacts before writing the paired receipt.')
for name, values in results.items():
    required = {'resvg','direct_mupdf','direct_pdfium'}
    if name != 'generic_colored_white':
        required |= {'svg_mupdf','svg_pdfium'}
    if set(values) != required:
        raise SystemExit('Incomplete PDF measurements for '+name)
report = {'schema':'sdocx-pencil2-mixed-rgb-paired-v1','frame':[192,192],'dpi':72,
          'native_sha256':hashlib.sha256((root / 'conformance/pencil2-v14-mixed-rgb-native.json').read_bytes()).hexdigest(),
          'readers':{'resvg':'0.47.0','mupdf':pymupdf.VersionBind,'pdfium':str(pypdfium2.PDFIUM_INFO)},
          'results':results,
          'reproduction':['CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 SDOCX_PENCIL2_ARTIFACTS="$PWD/scratch/pencil2-mixed-rgb/exports" cargo test -p sdocx --lib --all-features render::pencil2_tests -- --nocapture','.venv/bin/python tools/pencil2_v14/compare.py'],
          'scope':'Controlled host records and test-only bounded RGBA hypothesis; paired scene SVG/direct PDF and SVG-to-PDF exports. Only generic_colored_white uses public Stroke production dispatch. Suffix _seeded paints each stamp onto the paper backdrop before encoding the full frame. All raster carriers are destination-resolution images, not scalable vector paint.',
          'limitations':['PDF image transport shares the existing image importer; independent readers do not independently validate the shader.','Unseeded transparent stroke precomposition changes per-stamp rounding over paper. Seeded carriers include the backdrop and cannot be inserted over arbitrary live content. Saved-note context, arbitrary transforms, device output and other resolutions remain unverified.']}
(root / 'conformance/pencil2-v14-mixed-rgb-paired.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(results,indent=2))
