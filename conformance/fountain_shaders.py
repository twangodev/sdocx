"""Extract the hash-pinned APK's original RTV4/RTV5 shaders for pixel comparison.

python3 conformance/fountain_shaders.py --output /tmp/fountain-shaders.json
No vendor shader source is checked into the repository.
"""
import argparse
import hashlib
import json
import re
import struct
import subprocess
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--library', type=Path, default=Path(
        'scratch/apk-analysis-native/arm64-v8a/libSPenFountainPen.so'))
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    binary = args.library.read_bytes()
    expected = '1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec'
    if hashlib.sha256(binary).hexdigest() != expected:
        raise ValueError('unverified FountainPen library')
    offset = struct.unpack_from('<Q', binary, 32)[0]
    stride, count = struct.unpack_from('<HH', binary, 54)
    segments = [struct.unpack_from('<II6Q', binary, offset + i * stride)
                for i in range(count)]
    symbols = subprocess.check_output(['nm', '-D', '-C', args.library], text=True)
    shaders = {}
    for address, kind, version, stage in re.findall(
        r'^([0-9a-f]+) R SPen::FountainPenStroke(Alpha|Composite)?ShaderV([45])::sz(Vertex|Fragment)Shader$',
        symbols, re.M,
    ):
        address = int(address, 16)
        segment = next(s for s in segments
                       if s[0] == 1 and s[3] <= address < s[3] + s[5])
        offset = segment[2] + address - segment[3]
        key = (kind.lower() + '_' if kind else '') + stage.lower()
        shaders.setdefault(version, {})[key] = binary[offset:binary.index(0, offset)].decode()
    for version in ('4', '5'):
        assert set(shaders[version]) == {
            'vertex', 'fragment', 'alpha_vertex', 'alpha_fragment',
            'composite_vertex', 'composite_fragment',
        }
    args.output.write_text(json.dumps(shaders) + '\n')


if __name__ == '__main__':
    main()
