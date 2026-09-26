"""Execute the complete native FountainPen V14 saved redraw loop.

This invokes native redraw(ObjectStroke),
drawLine, endPen, drawPoint and SmPath. Only ObjectStroke channel access,
MotionEvent endpoint access and the final GPU submission are supplied by the
host. APK hashes, allocation and instruction limits use fountain_native.

Input is ink_geometry's JSON. Output includes native radii AND direction
vectors: V14 uses those directions for its opacity-gradient stamp shader.
"""
import argparse
import copy
import json
import shutil
from pathlib import Path

from fountain_native import NativeFountain, check

PREFIX = 'SPen::FountainPenStrokeDrawableGLV14::'
LINE = PREFIX + 'drawLine(float, float, float, float, long long, int, SPen::RectF*, bool)'
REDRAW = PREFIX + 'redraw(SPen::ObjectStroke const*, SPen::RectF*, bool)'


class NativeFountain14(NativeFountain):
    def __init__(self, library_dir, objdump, additional_libraries=()):
        super().__init__(library_dir, objdump, additional_libraries)
        self.recording = False
        self.callbacks['SPen::FountainPenStrokeDrawableRTV4::AddPoint(SPen::Vector3<float>, SPen::Vector2<float>, float)'] = self.stamp

    def hook(self, u, address, size, data):
        if self.recording and address == self.syms[LINE]:
            if self.sample:
                self.ends[self.sample] = len(self.dots)
            self.sample += 1
        super().hook(u, address, size, data)

    def stamp(self):
        self.dots.append([self.f(i) for i in range(5)])

    def render(self, stroke):
        rendering = stroke['rendering']
        tool = rendering['tool_type_raw']
        if tool not in (1, 2, 3):
            raise ValueError('V14 oracle requires saved tool type 1, 2 or 3')
        points = stroke['points']
        count = len(points)
        if not count or any(len(stroke[k]) != count for k in ('pressures', 'timestamps')):
            raise ValueError('incomplete input channels')
        if any(stroke[k] and len(stroke[k]) != count for k in ('tilts', 'orientations')):
            raise ValueError('incomplete optional channels')
        self.heap = 0x10000000
        obj, data, settings = self.alloc(512), self.alloc(512), self.alloc(128)
        rect, source, rt = self.alloc(32), self.alloc(16), self.alloc(512)
        self.put(obj + 16, 'Q', rt)
        self.put(obj + 44, 'f', rendering['style']['initial_tolerance'] or 0.0)
        self.put(obj + 72, 'Q', data)
        self.put(data, 'Q', settings)
        self.put(data + 56, 'BB', 1, int(rendering['properties']['fixed_width']))
        fixed_width = rendering['style']['fixed_width']
        if rendering['properties']['fixed_width'] and fixed_width is None:
            raise ValueError('fixed-width redraw requires the saved width')
        self.put(data + 60, 'f', fixed_width if fixed_width is not None else stroke['pen_width'])
        self.put(settings, 'f', stroke['pen_width'])
        self.put(obj + 96, 'f', 0.82)
        self.call('SPen::SmPath::SmPath()', [obj + 136])
        self.channel('GetPoint', 'f', [v for p in points for v in (p['x'], p['y'])])
        self.channel('GetPressure', 'f', stroke['pressures'])
        self.channel('GetTimeStamp', 'i', stroke['timestamps'])
        self.channel('GetTilt', 'f', stroke['tilts'])
        self.channel('GetOrientation', 'f', stroke['orientations'])
        self.callbacks['SPen::ObjectStroke::GetPointCount() const'] = lambda: self.x(0, count)
        self.callbacks['SPen::ObjectStroke::GetToolType() const'] = lambda: self.x(0, tool)
        self.callbacks['SPen::ObjectStroke::IsShape() const'] = lambda: self.x(0, 0)
        self.end = {**points[-1], 'pressure': stroke['pressures'][-1],
                    'tilt': stroke['tilts'][-1] if stroke['tilts'] else 0.,
                    'time': stroke['timestamps'][-1], 'tool': tool, 'source': 0}
        self.dots, self.ends, self.sample = [], [0] * count, 0
        self.recording = True
        try:
            self.call(REDRAW, [obj, source, rect, 1])
        finally:
            self.recording = False
        if self.x(0) != 1 or self.sample != count - 1:
            raise AssertionError('native redraw did not complete all samples')
        self.ends[-1] = len(self.dots)
        return dict(dots=self.dots, sample_ends=self.ends)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prepared', type=Path, help='JSON from ink_geometry; compare reconstructed V14 fountain strokes')
    parser.add_argument('--reference', type=Path, default=Path('conformance/fountain-v14.json'))
    parser.add_argument('--output', type=Path)
    parser.add_argument('--library-dir', type=Path, default=Path('scratch/apk-analysis-native/arm64-v8a'))
    parser.add_argument('--objdump', type=Path, default=Path(shutil.which('aarch64-linux-gnu-objdump') or 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'))
    parser.add_argument('--limit', type=int)
    args = parser.parse_args()
    native = NativeFountain14(args.library_dir, args.objdump)
    if args.limit is not None and args.limit <= 0:
        raise ValueError('limit must be positive')
    selected = []
    if args.prepared:
        for i, row in enumerate(json.loads(args.prepared.read_text())):
            r = row['stroke']['rendering']
            g = row['geometry']
            if (g['support'] == 'reconstructed' and r and r['advanced_settings'] == '14;'
                    and r['pen_name'] == 'com.samsung.android.sdk.pen.pen.preload.FountainPen'):
                if len(g['points']) != len(g['dot_radii']) or len(g['points']) != len(g['dot_directions']):
                    raise ValueError(f'{i}: incomplete prepared stamp attributes')
                dots = [[p['x'], p['y'], radius, direction['x'], direction['y']]
                        for p, radius, direction in zip(g['points'], g['dot_radii'], g['dot_directions'])]
                selected.append((str(i), row['stroke'], dict(dots=dots, sample_ends=g['sample_ends'])))
    else:
        reference = json.loads(args.reference.read_text())
        for case in reference['cases']:
            stroke = copy.deepcopy(reference['stroke'])
            stroke['pen_width'] = case['size']
            stroke['rendering']['style']['initial_tolerance'] = case['tolerance']
            if 'tool_type_raw' in case:
                stroke['rendering']['tool_type_raw'] = case['tool_type_raw']
            if 'fixed_width' in case:
                stroke['rendering']['properties']['fixed_width'] = True
                stroke['rendering']['style']['fixed_width'] = case['fixed_width']
            stroke['points'] = [dict(x=s[0], y=s[1]) for s in case['samples']]
            stroke['pressures'] = [s[2] for s in case['samples']]
            stroke['tilts'] = [s[3] for s in case['samples']]
            stroke['timestamps'] = [s[4] for s in case['samples']]
            selected.append((case['name'], stroke, case))
    if args.limit is not None:
        selected = selected[:args.limit]
    if not selected:
        raise ValueError('no FountainPen 14 strokes')
    output, maximum = [], 0.0
    for name, stroke, expected in selected:
        actual = native.render(stroke)
        maximum = max(maximum, check(actual, expected, name))
        output.append(dict(name=name, **actual))
    if args.output:
        args.output.write_text(json.dumps(output) + '\n')
    print(json.dumps(dict(strokes=len(output), stamps=sum(len(s['dots']) for s in output),
                         reference_checked=True, comparison='prepared' if args.prepared else 'fixture', maximum_error=maximum,
                         scope='native saved V14 redraw; canonical inverse scale 1')))



if __name__ == '__main__':
    main()
