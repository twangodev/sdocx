#!/usr/bin/env python3
"""Compare isolated experimental four/five-point Scene exports with host frames."""
import base64
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import zlib

import pymupdf
from PIL import Image

ROOT = Path(__file__).resolve().parents[3]
REVISION = 'afc6f8e32b4d24bc4238900e15cdaac26987a1b9'
sys.path.insert(0, str(ROOT / 'tools/montblanc'))
from public_fallback_compare import composite_native_frame, metrics

receipt = json.loads((ROOT / 'conformance/montblanc-extended-paint-native.json').read_text())
with tempfile.TemporaryDirectory(prefix='montblanc-extended-scene-') as directory:
    tmp = Path(directory)
    archive = tmp / 'source.tar'
    subprocess.run(['git', 'archive', '--format=tar', '--output', str(archive), REVISION], cwd=ROOT, check=True)
    with tarfile.open(archive) as source:
        source.extractall(tmp, filter='data')
    archive.unlink()
    ink = tmp / 'crates/sdocx/src/ink/montblanc.rs'
    s = ink.read_text()
    marker = '    pub(crate) fn captured_control_buffers('
    helper = '''    pub(crate) fn extended_buffers(case: usize) -> [Vec<[f32; 5]>; 3] {
        let (points, pressure): (&[[f32; 2]], &[f32]) = match case {
            0 => (&[[1.,3.],[10.,10.],[20.,2.],[30.,0.]], &[0.3,0.6,1.,0.7]),
            _ => (&[[0.,0.],[5.,8.],[11.,-1.],[15.,6.],[22.,-3.]], &[0.3,0.6,1.,0.7,0.5]),
        };
        let input = super::Input { points, pressure, pen_size: 2. };
        let records = if case == 0 { super::four_point_tests::four_point(input) } else { super::five_point_tests::five_point(input) }.unwrap();
        let mut buffers: [Vec<[f32; 5]>; 3] = std::array::from_fn(|_| Vec::new());
        for record in records {
            let (buffer, vertices) = match record.call {
                RtCall::Start => (1, cap_vertices(record.lanes)),
                RtCall::Point => (0, point_vertices(record.lanes)),
                RtCall::End => (2, cap_vertices(record.lanes)),
            };
            buffers[buffer].extend(vertices);
        }
        let output = std::path::PathBuf::from(std::env::var_os("SDOCX_MONTBLANC_CAPTURE_DIR").unwrap());
        for (index, buffer) in buffers.iter().enumerate() {
            let bytes: Vec<u8> = buffer.iter().flatten().flat_map(|v| v.to_le_bytes()).collect();
            std::fs::write(output.join(format!("{case}-{index}.bin")), bytes).unwrap();
        }
        buffers
    }

'''
    assert marker in s
    ink.write_text(s.replace(marker, helper + marker))
    for name in ('four', 'five'):
        path = tmp / f'crates/sdocx/src/ink/montblanc_{name}_test.rs'
        path.write_text(path.read_text().replace(f'fn {name}_point(input:', f'pub(super) fn {name}_point(input:', 1))
    scene = tmp / 'crates/sdocx/src/render/montblanc_candidate.rs'
    s = scene.read_text()
    a = s.index('    for (case, (name, end, pressure, width)) in [')
    b = s.index('        assert!(\n            triangles', a)
    s = s[:a] + '''    for (case, name) in ["four", "five"].into_iter().enumerate() {
        let triangles = pieces(&crate::ink::montblanc::tests::extended_buffers(case), 8);
        let count = triangles.len();
''' + s[b:]
    a = s.index('        assert_eq!(\n            svg.matches("<path").count(),')
    b = s.index('        let document =', a)
    s = s[:a] + '        assert_eq!(svg.matches("<path").count(), count);\n' + s[b:]
    scene.write_text(s)
    output = tmp / 'output'
    output.mkdir()
    env = os.environ | {'CARGO_TARGET_DIR':'/tmp/sdocx-agent-target', 'CARGO_INCREMENTAL':'0',
                        'CARGO_PROFILE_DEV_DEBUG':'0', 'CARGO_PROFILE_TEST_DEBUG':'0',
                        'SDOCX_MONTBLANC_CAPTURE_DIR':str(output)}
    subprocess.run(['cargo','test','--offline','-p','sdocx','--lib','--features','pdf',
                    'captured_controls_shared_scene_exports'], cwd=tmp, env=env, check=True)
    fallback = tmp / 'fallback'
    (fallback / 'src').mkdir(parents=True)
    (fallback / 'Cargo.toml').write_text('[package]\nname="montblanc-extended-fallback"\nversion="0.1.0"\nedition="2024"\n[workspace]\n[dependencies]\nsdocx={path="../crates/sdocx",features=["pdf"]}\n')
    source = (tmp / 'tools/montblanc/public-fallback/src/main.rs').read_text()
    fixture = ROOT / 'crates/sdocx/tests/assets/legacy-marker/synthetic-serialized.sdocx'
    assert hashlib.sha256(fixture.read_bytes()).hexdigest() == 'ec4476cba55a9e92e4d5c54eb5789e0842a223e120825009fffd004a952d7de5'
    source = source.replace('"tmp/legacy-marker-source-controls/serialized-marker.sdocx"', json.dumps(str(fixture)))
    a = source.index('    for (name, end, pressure, width) in [')
    b = source.index('        let mut stroke =', a)
    source = source[:a] + '''    for (name, points, pressures) in [
        ("four", vec![[1.,3.],[10.,10.],[20.,2.],[30.,0.]], vec![0.3,0.6,1.,0.7]),
        ("five", vec![[0.,0.],[5.,8.],[11.,-1.],[15.,6.],[22.,-3.]], vec![0.3,0.6,1.,0.7,0.5]),
    ] {
        let width = 2.;
''' + source[b:]
    a = source.index('        stroke.points = vec![')
    b = source.index('        stroke.pen_width = width;', a)
    source = source[:a] + '''        stroke.points = points.into_iter().map(|p| Point { x: 128. + p[0], y: 128. - p[1] }).collect();
        stroke.pressures = pressures;
''' + source[b:]
    (fallback / 'src/main.rs').write_text(source)
    fallback_output = tmp / 'fallback-output'
    fallback_output.mkdir()
    subprocess.run(['cargo','run','--offline','--quiet','--manifest-path',str(fallback / 'Cargo.toml'),'--',str(fallback_output)],cwd=ROOT,env=env,check=True)
    result = {'schema':'montblanc-extended-scene-host-comparison-v1',
              'boundary':'Synthetic controls, test-only sampler and approximate Scene paint on #fcfcfc; host Mesa is not device or saved-note parity.',
              'capture_receipt_sha256':hashlib.sha256((ROOT / 'conformance/montblanc-extended-paint-native.json').read_bytes()).hexdigest(),
              'source_revision':REVISION,'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              'compositor_sha256':hashlib.sha256((ROOT / 'tools/montblanc/public_fallback_compare.py').read_bytes()).hexdigest(),
              'method':'native support, export support, intersection, union, RGB absolute error on union; support differs from 252 by more than 2. Native RGB is premultiplied; composition uses nearest-integer source-over.',
              'renderers':{'pymupdf':pymupdf.VersionBind,'resvg':subprocess.check_output(['resvg','--version'],text=True).strip(),'pillow':Image.__version__}, 'cases':{}}
    for name in ('four','five'):
        raw = gzip.decompress((ROOT / f'tools/montblanc/extended-paint/{name}.json.gz').read_bytes())
        assert hashlib.sha256(raw).hexdigest() == receipt['cases'][name]['capture_sha256']
        capture = json.loads(raw)
        for index, expected in enumerate(receipt['cases'][name]['native_add_vectors']):
            assert hashlib.sha256((output / f'{(0 if name == "four" else 1)}-{index}.bin').read_bytes()).hexdigest() == expected['sha256'], (name,index)
        frame = zlib.decompress(base64.b64decode(capture['post_readback']['rgba8_zlib_base64']))
        assert hashlib.sha256(frame).hexdigest() == receipt['cases'][name]['frame_sha256']
        reference = composite_native_frame(frame, (256, 256)).tobytes()
        png = tmp / 'page.png'
        subprocess.run(['resvg',str(output / f'{name}.svg'),str(png)],check=True)
        svg = metrics(reference,Image.open(png).convert('RGB').tobytes())
        with pymupdf.open(output / f'{name}.pdf') as doc:
            pix = doc[0].get_pixmap(matrix=pymupdf.Matrix(4/3,4/3),alpha=False)
            assert (pix.width,pix.height)==(256,256)
            pdf = metrics(reference,pix.samples)
        subprocess.run(['resvg',str(fallback_output / f'{name}.svg'),str(png)],check=True)
        fallback_svg = metrics(reference,Image.open(png).convert('RGB').tobytes())
        with pymupdf.open(fallback_output / f'{name}.pdf') as doc:
            pix = doc[0].get_pixmap(matrix=pymupdf.Matrix(4/3,4/3),alpha=False)
            assert (pix.width,pix.height)==(256,256)
            fallback_pdf = metrics(reference,pix.samples)
        result['cases'][name]={'svg':svg,'pdf':pdf,'fallback_svg':fallback_svg,'fallback_pdf':fallback_pdf,'vertex_buffers_match_native':True}
        print(name, svg, pdf, fallback_svg, fallback_pdf, flush=True)
    (ROOT / 'conformance/montblanc-extended-scene-host-comparison.json').write_text(json.dumps(result,indent=2)+'\n')
