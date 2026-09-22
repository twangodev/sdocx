"""Execute the complete native FountainPen V14 saved redraw loop.

Unlike the V16 primitive oracle, this invokes native redraw(ObjectStroke),
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
        # redraw constructs the endpoint MotionEvent from these same channels;
        # NativeFountain's getters return self.end, including its signed time.
        self.callbacks['SPen::MotionEvent::MotionEvent(long long, int, int, SPen::MotionEvent::PointerProperties*, SPen::MotionEvent::PointerCoords*, int, int, int)'] = lambda: None
        self.callbacks['SPen::MotionEvent::~MotionEvent()'] = lambda: None

    def hook(self, u, address, size, data):
        if self.recording and address == self.syms[LINE]:
            if self.sample:
                self.ends[self.sample] = len(self.dots)
            self.sample += 1
        super().hook(u, address, size, data)

    def stamp(self):
        self.dots.append([self.f(i) for i in range(5)])

    def channel(self, name, fmt, values):
        if values:
            pointer = self.alloc(8 * len(values))
            self.put(pointer, fmt * len(values), *values)
        else:
            pointer = 0
        self.callbacks['SPen::ObjectStroke::' + name + '() const'] = lambda: self.x(0, pointer)

    def render(self, stroke):
        rendering = stroke['rendering']
        if rendering['tool_type_raw'] != 2:
            raise ValueError('V14 oracle currently requires stylus channels')
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
        self.put(data + 60, 'f', rendering['style']['fixed_width'] or stroke['pen_width'])
        self.put(settings, 'f', stroke['pen_width'])
        self.put(obj + 96, 'f', 0.82)
        self.call('SPen::SmPath::SmPath()', [obj + 136])
        self.channel('GetPoint', 'f', [v for p in points for v in (p['x'], p['y'])])
        self.channel('GetPressure', 'f', stroke['pressures'])
        self.channel('GetTimeStamp', 'i', stroke['timestamps'])
        self.channel('GetTilt', 'f', stroke['tilts'])
        self.channel('GetOrientation', 'f', stroke['orientations'])
        self.callbacks['SPen::ObjectStroke::GetPointCount() const'] = lambda: self.x(0, count)
        self.callbacks['SPen::ObjectStroke::GetToolType() const'] = lambda: self.x(0, 2)
        self.callbacks['SPen::ObjectStroke::IsShape() const'] = lambda: self.x(0, 0)
        self.end = {**points[-1], 'pressure': stroke['pressures'][-1],
                    'tilt': stroke['tilts'][-1] if stroke['tilts'] else 0.,
                    'time': stroke['timestamps'][-1], 'tool': 2, 'source': 0}
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
    parser.add_argument('--prepared', type=Path)
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
            if r and r['advanced_settings'] == '14;' and r['pen_name'] == 'com.samsung.android.sdk.pen.pen.preload.FountainPen':
                selected.append((str(i), row['stroke'], None))
    else:
        reference = json.loads(args.reference.read_text())
        for case in reference['cases']:
            stroke = copy.deepcopy(reference['stroke'])
            stroke['pen_width'] = case['size']
            stroke['rendering']['style']['initial_tolerance'] = case['tolerance']
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
        if expected is not None:
            maximum = max(maximum, check(actual, expected, name))
        output.append(dict(name=name, **actual))
    if args.output:
        args.output.write_text(json.dumps(output) + '\n')
    print(json.dumps(dict(strokes=len(output), stamps=sum(len(s['dots']) for s in output),
                         reference_checked=not bool(args.prepared), maximum_error=maximum,
                         scope='native saved V14 redraw; canonical inverse scale 1')))



if __name__ == '__main__':
    main()
