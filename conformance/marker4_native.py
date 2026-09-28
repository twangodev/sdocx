"""Compare saved Marker4 V7 stylus geometry with the hash-pinned APK.

Executes native redraw, drawLine, endPen, drawPoint and SmPath. Object setup
at unit inverse scale, saved channels and endpoint MotionEvent access are host
supplied. The final RTV3 AddPoint call is collected instead of submitting to GL.
This checks geometry, not the native texture's filtered pixels.
"""
import argparse
import copy
import json
import math
import shutil
from pathlib import Path

from fountain_native import NativeFountain

PREFIX = 'SPen::Marker4StrokeDrawableGLV7::'
LINE = PREFIX + 'drawLine(float, float, float, bool, bool)'
END = PREFIX + 'endPen(SPen::MotionEvent const*, SPen::RectF*, bool)'
REDRAW = PREFIX + 'redraw(SPen::ObjectStroke const*, SPen::RectF*, bool)'
PEN = 'com.samsung.android.sdk.pen.pen.preload.Marker4'


class NativeMarker4(NativeFountain):
    def __init__(self, library_dir, objdump):
        self.recording = False
        super().__init__(library_dir, objdump, ('Marker4',))
        self.callbacks['SPen::Marker4StrokeDrawableRTV3::AddPoint(SPen::Vector2<float>, float, float)'] = self.stamp
        self.callbacks['SPen::MotionEvent::GetOrientation(int) const'] = lambda: self.f(0, 0.)

    def hook(self, u, address, size, data):
        if self.recording and address in (self.syms[LINE], self.syms[END]):
            self.ends[self.sample] = len(self.dots)
            self.sample += 1
        super().hook(u, address, size, data)

    def stamp(self):
        self.dots.append([self.f(i) for i in range(4)])

    def render(self, stroke):
        rendering = stroke['rendering']
        points = stroke['points']
        count = len(points)
        if rendering['tool_type_raw'] != 2 or rendering['advanced_settings'] != '7;':
            raise ValueError('oracle requires V7 stylus input')
        if not count or any(len(stroke[k]) != count for k in ('pressures', 'timestamps')):
            raise ValueError('incomplete required channels')
        if any(stroke[k] and len(stroke[k]) != count for k in ('tilts', 'orientations')):
            raise ValueError('incomplete optional channels')
        self.heap = 0x10000000
        obj, data, settings = self.alloc(512), self.alloc(128), self.alloc(128)
        self.put(obj + 16, 'Q', self.alloc(512))
        self.put(obj + 44, 'f', 2.)
        self.put(obj + 72, 'Q', data)
        self.put(data, 'Q', settings)
        self.put(settings, 'f', stroke['pen_width'])
        self.put(obj + 88, 'f', 1.)
        self.put(obj + 96, 'f', 20.)
        self.call('SPen::SmPath::SmPath()', [obj + 136])
        self.channel('GetPoint', 'f', [v for p in points for v in (p['x'], p['y'])])
        self.channel('GetPressure', 'f', stroke['pressures'])
        self.channel('GetTimeStamp', 'i', stroke['timestamps'])
        self.channel('GetTilt', 'f', stroke['tilts'])
        self.channel('GetOrientation', 'f', stroke['orientations'])
        self.callbacks['SPen::ObjectStroke::GetPointCount() const'] = lambda: self.x(0, count)
        self.callbacks['SPen::ObjectStroke::GetToolType() const'] = lambda: self.x(0, 2)
        self.callbacks['SPen::ObjectStroke::IsShape() const'] = lambda: self.x(0, 0)
        self.end = {**points[-1], 'tool': 2, 'source': 0}
        self.dots, self.ends, self.sample = [], [0] * count, 0
        self.recording = True
        try:
            self.call(REDRAW, [obj, self.alloc(16), self.alloc(32), 1], instruction_limit=20000000)
        finally:
            self.recording = False
        if self.x(0) != 1 or self.sample != max(1, count - 1):
            raise AssertionError('native redraw did not consume the expected samples')
        self.ends[-1] = len(self.dots)
        tip = self.alloc(32)
        self.call('SPen::Marker4StrokeDrawableRTV3::MakePointRect(SPen::Vector2<float>, float, SPen::RectF&)',
                  [0, tip], [0., 0., self.dots[0][2]])
        return {'dots': self.dots, 'sample_ends': self.ends,
                'tip_bounds': list(self.get(tip, '4f'))}


def case_stroke(template, case):
    stroke = copy.deepcopy(template)
    stroke['pen_width'] = case['size']
    stroke['points'] = [{'x': p[0], 'y': p[1]} for p in case['samples']]
    stroke['pressures'] = [p[2] for p in case['samples']]
    stroke['timestamps'] = [i * 8 for i in range(len(case['samples']))]
    stroke['tilts'] = [p[3] for p in case['samples']]
    return stroke


def compare(actual, expected):
    if actual['sample_ends'] != expected['sample_ends']:
        raise AssertionError('sample boundaries differ')
    if len(actual['dots']) != len(expected['dots']):
        raise AssertionError('stamp counts differ')
    error = 0.
    for a, b in zip(actual['dots'], expected['dots']):
        if len(a) != len(b):
            raise AssertionError('stamp attributes differ')
        if not all(math.isfinite(v) for v in (*a, *b)):
            raise AssertionError('nonfinite stamp attributes')
        error = max(error, *(abs(x - y) for x, y in zip(a, b)))
    if 'tip_bounds' in expected:
        if actual['tip_bounds'] != expected['tip_bounds']:
            raise AssertionError('native tip bounds differ')
    if error > 0.0001:
        raise AssertionError(f'maximum stamp error {error}')
    return error


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference', type=Path, default=Path('conformance/marker4-v7.json'))
    parser.add_argument('--prepared', type=Path)
    parser.add_argument('--capture', type=Path, help='write new native captures separately from the reference')
    parser.add_argument('--library-dir', type=Path, default=Path('scratch/apk-analysis-native/arm64-v8a'))
    parser.add_argument('--objdump', type=Path, default=Path(shutil.which('aarch64-linux-gnu-objdump') or 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'))
    args = parser.parse_args()
    native = NativeMarker4(args.library_dir, args.objdump)
    count, stamps, error = 0, 0, 0.
    if args.prepared:
        if args.capture:
            parser.error('--prepared and --capture cannot be combined')
        for row in json.loads(args.prepared.read_text()):
            s, g = row['stroke'], row['geometry']
            r = s.get('rendering') or {}
            if (r.get('pen_name'), r.get('advanced_settings'), r.get('tool_type_raw')) != (PEN, '7;', 2):
                continue
            actual = native.render(s)
            tip = g.get('rect_stamp')
            if not tip or not g.get('sample_ends'):
                raise AssertionError('V7 stroke fell back to generic geometry')
            left, top, right, bottom = actual['tip_bounds']
            if not all(math.isfinite(v) for v in tip.values()):
                raise AssertionError('nonfinite tip geometry')
            if abs(tip['height'] - (bottom - top) * .99) > 0.0001 or abs(tip['width'] - (right - left) * .99) > 0.0001:
                raise AssertionError('incorrect V7 rectangular tip dimensions')
            radius = tip['height'] / 1.98 - .5
            expected = {'dots': [[p['x'], p['y'], radius, tip['angle']] for p in g['points']],
                        'sample_ends': g['sample_ends']}
            error = max(error, compare(actual, expected))
            count += 1
            stamps += len(actual['dots'])
    else:
        reference = json.loads(args.reference.read_text())
        for case in reference['cases']:
            actual = native.render(case_stroke(reference['stroke'], case))
            if args.capture:
                case.update(actual)
            else:
                error = max(error, compare(actual, case))
            count += 1
            stamps += len(actual['dots'])
        if args.capture:
            args.capture.write_text(json.dumps(reference, separators=(',', ':')) + '\n')
    if not count:
        raise ValueError('no V7 stylus strokes selected')
    print(json.dumps({'cases': count, 'stamps': stamps, 'maximum_error': error,
                      'reference_checked': not bool(args.capture)}))


if __name__ == '__main__':
    main()
