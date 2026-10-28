#!/usr/bin/env python3
"""Compare an isolated public-Stroke sampler bridge with four host captures."""
import base64
import gzip
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import zlib

import pymupdf
from PIL import Image

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'tools/montblanc'))
from triangle_svg_prototype import build
from public_fallback_compare import metrics, FIXTURE, FIXTURE_SHA256


def sha(data):
    return hashlib.sha256(data).hexdigest()


def run():
    if sha(FIXTURE.read_bytes()) != FIXTURE_SHA256:
        raise ValueError('source fixture hash mismatch')
    sampler = (ROOT / 'crates/sdocx/src/ink/montblanc.rs').read_text()
    bridge = sampler[sampler.index('use super::path'):sampler.index('#[cfg(test)]')]
    bridge = bridge.replace('use sha2::{Digest, Sha256};', '')
    bridge = bridge.replace('const NATIVE_STREAM: &[u8] = include_bytes!("../../../../conformance/montblanc-native-v1.bin");', '')
    bridge = bridge.replace('const NATIVE_STREAM_SHA256: &str =\n    "860aa1762e8a341e45a8cb9087a0c74960d9e34b304ccb19c2d09800347eaf3d";', '')
    for old, new in [('enum RtCall', 'pub enum RtCall'), ('struct RtRecord', 'pub struct RtRecord'),
                     ('    call: RtCall,', '    pub call: RtCall,'), ('    lanes: [f32; 6],', '    pub lanes: [f32; 6],'),
                     ('enum Error', 'pub enum Error'), ("struct Input<'a>", "pub struct Input<'a>"),
                     ("    points: &'a", "    pub points: &'a"), ("    pressure: &'a", "    pub pressure: &'a"),
                     ('    pen_size: f32,', '    pub pen_size: f32,'), ('fn sample(input:', 'pub fn sample(input:')]:
        bridge = bridge.replace(old, new)
    mesh = sampler[sampler.index('    type Vertex ='):sampler.index('    #[test]\n    fn saved_routes_match_native_expanded_triangle_meshes')]
    for old, new in [('    type Vertex', '    pub type Vertex'), ('    fn cap_vertices', '    pub fn cap_vertices'), ('    fn point_vertices', '    pub fn point_vertices')]:
        mesh = mesh.replace(old, new)
    path = (ROOT / 'crates/sdocx/src/ink/path.rs').read_text().replace('use crate::Point;', 'use sdocx::Point;')
    receipt = json.loads((ROOT / 'conformance/montblanc-public-candidate-host-comparison.json').read_text())
    for filename, expected in receipt['source_sha256'].items():
        if sha((ROOT / filename).read_bytes()) != expected:
            raise ValueError(f'source hash mismatch: {filename}')
    with tempfile.TemporaryDirectory(prefix='montblanc-candidate-') as directory:
        tmp = Path(directory); (tmp / 'src').mkdir()
        (tmp / 'src/sampler.rs').write_text(bridge + mesh)
        (tmp / 'src/path.rs').write_text(path)
        (tmp / 'src/main.rs').write_bytes((ROOT / 'tools/montblanc/public-candidate/main.rs').read_bytes())
        (tmp / 'Cargo.toml').write_text('[package]\nname="montblanc-candidate"\nversion="0.1.0"\nedition="2024"\n[workspace]\n[dependencies]\nsdocx={path="' + str(ROOT / 'crates/sdocx') + '"}\nserde_json="1"\n')
        env = os.environ | {'CARGO_TARGET_DIR': '/tmp/sdocx-agent-target', 'CARGO_INCREMENTAL': '0', 'CARGO_PROFILE_DEV_DEBUG': '0', 'CARGO_PROFILE_TEST_DEBUG': '0'}
        output = subprocess.check_output(['cargo', 'run', '--offline', '--quiet', '--manifest-path', str(tmp / 'Cargo.toml')], cwd=ROOT, env=env, text=True)
        for line in output.splitlines():
            case = json.loads(line); name = case['case']; expected = receipt['cases'][name]
            raw = gzip.decompress((ROOT / f'tools/montblanc/captures/{name}.json.gz').read_bytes())
            if sha(raw) != expected['capture_sha256']:
                raise ValueError(f'{name}: capture hash mismatch')
            capture = json.loads(raw)
            for idx, proof in enumerate(capture['draw_provenance']):
                buf = next(a['buffer'] for a in proof['attributes'] if a['index'] == 0)
                upload = next(x for x in capture['vbo_uploads'] if x.get('boundBuffer') == buf and x.get('dataHex'))
                data = b''.join(struct.pack('<5f', *vertex) for vertex in case['buffers'][idx])
                if data.hex() != upload['dataHex'].lower():
                    raise ValueError(f'{name}: mesh {idx} differs from native')
                upload.update(dataHex=data.hex(), size=len(data), dataSha256=sha(data))
            capture_path = tmp / f'{name}.json'; capture_path.write_text(json.dumps(capture))
            svg = tmp / f'{name}.svg'; build(capture_path, svg, 4)
            subprocess.run(['resvg', str(svg), str(tmp / 'page.png')], check=True)
            frame = zlib.decompress(base64.b64decode(capture['post_readback']['rgba8_zlib_base64']))
            if sha(frame) != capture['post_readback']['sha256']:
                raise ValueError(f'{name}: frame hash mismatch')
            native = Image.new('RGBA', (256, 256), (252, 252, 252, 255))
            native.alpha_composite(Image.frombytes('RGBA', (256, 256), frame).transpose(Image.Transpose.FLIP_TOP_BOTTOM))
            actual = Image.new('RGBA', (256, 256), (252, 252, 252, 255))
            actual.alpha_composite(Image.open(tmp / 'page.png').convert('RGBA'))
            svg_values = metrics(native.convert('RGB').tobytes(), actual.convert('RGB').tobytes())
            opaque_svg = svg.read_text().replace('<defs>', '<rect width="256" height="256" fill="#fcfcfc"/><defs>', 1)
            with pymupdf.open(stream=opaque_svg.encode(), filetype='svg') as document:
                pdf = document.convert_to_pdf()
            with pymupdf.open(stream=pdf, filetype='pdf') as document:
                pixmap = document[0].get_pixmap(alpha=False)
                if (pixmap.width, pixmap.height) != (256, 256):
                    raise ValueError(f'{name}: unexpected PDF size')
                pdf_values = metrics(native.convert('RGB').tobytes(), pixmap.samples)
            print(name, 'svg', svg_values, 'svg-to-pdf', pdf_values)
            if svg_values != expected['svg'] or pdf_values != expected['svg_to_pdf']:
                raise ValueError(f'{name}: metrics differ from receipt')


if __name__ == '__main__':
    run()
