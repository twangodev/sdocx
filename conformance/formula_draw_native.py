#!/usr/bin/env python3
"""Bounded native Formula image selection and stroke-precedence oracle.

Drawing::drawFormula, Model::ObjectFormulaImpl::GetLatexImagePath and
ImageCommon::GetImagePath execute from hash-pinned DSOs. Path String emptiness,
cache/decode results, CreateBitmap, final bitmap drawing, and ObjectList/DrawStroke
are explicit host boundaries. The probe verifies dispatch and retrieval results,
not complete Formula object construction or final pixels/transforms.

Run from the repository root:
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/formula_draw_native.py
"""
import hashlib
import json
import os
import re
import struct
import subprocess
from pathlib import Path

from unicorn import Uc, UC_ARCH_ARM64, UC_MODE_ARM, UC_HOOK_CODE
from unicorn import arm64_const as R

ROOT = Path(__file__).resolve().parents[1]
LIBS = ROOT / 'scratch/apk-analysis-native/arm64-v8a'
OBJDUMP = ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'
HASHES = {
    'PenCommon': 'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d',
    'Base': 'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb',
    'Drawing': '788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd',
    'Model': '4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a',
}


class Probe:
    def __init__(self):
        self.u = Uc(UC_ARCH_ARM64, UC_MODE_ARM)
        self.heap = 0x10000000
        self.u.mem_map(self.heap, 0x2000000)
        self.u.mem_map(0x20000000, 0x100000)
        self.u.mem_map(0x7000000, 0x1000)
        self.u.reg_write(R.UC_ARM64_REG_TPIDR_EL0, 0x20000000)
        self.syms, self.plt, self.callbacks = {}, {}, {}
        self.virtual_return = 0
        env = dict(os.environ)
        runtime = ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu'
        env['LD_LIBRARY_PATH'] = str(runtime)
        for index, name in enumerate(('PenCommon', 'Base', 'Drawing', 'Model')):
            path = LIBS / f'libSPen{name}.so'
            raw = path.read_bytes()
            if hashlib.sha256(raw).hexdigest() != HASHES[name]:
                raise ValueError(f'unverified {path}')
            base = index * 0x1000000
            off = struct.unpack_from('<Q', raw, 32)[0]
            stride, n = struct.unpack_from('<HH', raw, 54)
            self.u.mem_map(base, 0x600000)
            for i in range(n):
                kind, _, fo, va, _, fs, _, _ = struct.unpack_from('<II6Q', raw, off + i * stride)
                if kind == 1:
                    self.u.mem_write(base + va, raw[fo:fo + fs])
            for line in subprocess.check_output(['nm', '-D', '-C', path], text=True).splitlines():
                parts = line.strip().split(' ', 2)
                if len(parts) == 3 and parts[1] in ('T', 'W'):
                    self.syms[parts[2]] = base + int(parts[0], 16)
            dis = subprocess.check_output([str(OBJDUMP), '-d', '-C', '-j', '.plt', path], env=env, text=True)
            for address, symbol in re.findall(r'^([0-9a-f]+) <(.+)@plt>:', dis, re.M):
                self.plt[base + int(address, 16)] = symbol
        self.u.hook_add(UC_HOOK_CODE, self.hook)

    def x(self, i, value=None):
        reg = R.UC_ARM64_REG_X0 + i
        if value is None:
            return self.u.reg_read(reg)
        self.u.reg_write(reg, value & ((1 << 64) - 1))

    def alloc(self, size):
        address = self.heap
        self.heap += (size + 15) & ~15
        self.u.mem_write(address, bytes(size))
        return address

    def put(self, addr, fmt, *values):
        self.u.mem_write(addr, struct.pack('<' + fmt, *values))

    def get(self, addr, fmt):
        return struct.unpack('<' + fmt, self.u.mem_read(addr, struct.calcsize('<' + fmt)))

    def hook(self, uc, address, _size, _data):
        if address == 0x7000000:
            self.x(0, self.virtual_return)
            uc.reg_write(R.UC_ARM64_REG_PC, uc.reg_read(R.UC_ARM64_REG_LR))
            return
        if address not in self.plt:
            return
        name = self.plt[address]
        if name in self.callbacks:
            self.callbacks[name]()
            uc.reg_write(R.UC_ARM64_REG_PC, uc.reg_read(R.UC_ARM64_REG_LR))
        elif name in self.syms:
            uc.reg_write(R.UC_ARM64_REG_PC, self.syms[name])
        else:
            raise RuntimeError('unhandled native import: ' + name)

    def call(self, name, args=(), instruction_limit=2_000_000):
        for i, value in enumerate(args):
            self.x(i, value)
        self.u.reg_write(R.UC_ARM64_REG_SP, 0x200f0000)
        self.u.reg_write(R.UC_ARM64_REG_LR, 0x200f1000)
        self.u.emu_start(self.syms[name], 0x200f1000, count=instruction_limit)
        if self.u.reg_read(R.UC_ARM64_REG_PC) != 0x200f1000:
            raise RuntimeError('native instruction limit')


def one_case(path_present, cache_present, decode_success, paint_success):
    p = Probe()
    formula_drawing, data, formula = p.alloc(16), p.alloc(0x100), p.alloc(40)
    canvas, source_bitmap, output_bitmap = p.alloc(16), p.alloc(16), p.alloc(16)
    context, vtable, queue = p.alloc(16), p.alloc(32), p.alloc(16)
    impl, image_state = p.alloc(0x180), p.alloc(64)
    image_common = impl + 232
    image_entry, path_holder = p.alloc(64), p.alloc(32)
    p.put(formula_drawing + 8, 'Q', data)
    p.put(data, 'Q', context)
    p.put(data + 8, 'Q', formula)
    p.put(formula + 32, 'Q', impl)
    p.put(impl + 208, 'i', 0)
    p.put(image_common + 8, 'Q', image_state)
    if path_present:
        # A minimal ImageCommon entry for local media index 0. The native
        # ImageCommon::GetImagePath traversal reads next at +0, key at +32,
        # payload holder at +40, and returns its String member at +16.
        p.put(image_state + 24, 'Q', image_entry)
        p.put(image_entry, 'Q', 0)
        p.put(image_entry + 32, 'i', 0)
        p.put(image_entry + 40, 'Q', path_holder)
    p.put(context, 'Q', vtable)
    p.put(vtable + 24, 'Q', 0x7000000)
    p.virtual_return = queue
    events = []

    def get_cache_image():
        events.append('cache-query')
        p.x(0, source_bitmap if cache_present else 0)

    p.callbacks.update({
        'SPen::String::IsEmpty() const': lambda: p.x(0, 0),
        'SPen::ObjectFormula::GetCacheImage()': get_cache_image,
        'SPen::ObjectFormula::GetLatexImage() const': lambda: (events.append('decode'), p.x(0, source_bitmap if decode_success else 0)),
        'SPen::ObjectFormulaDrawing::setCacheImageIfNotExist(SPen::Bitmap const*, bool)':
            lambda: events.append(('cache-store', p.x(1), p.x(2))),
        'SPen::ObjectFormulaDrawing::drawFormulaBitmap(SPen::ISPCanvas&, SPen::ISPBitmap*)':
            lambda: (events.append(('draw-bitmap', p.x(1), p.x(2), int(paint_success))), p.x(0, int(paint_success))),
        'SPen::ObjectFormulaDrawing::releaseFormulaImage(bool)': lambda: events.append(('release', p.x(1))),
        '__android_log_print': lambda: p.x(0, 0),
        'SPen::Error::SetError(unsigned long)': lambda: None,
        'SPen::Error::SetError(unsigned long, int, char const*)': lambda: None,
        'SPen::ObjectFormulaDrawing::getResizedFormulaBitmap(SPen::Bitmap const*)':
            lambda: (events.append(('resize-source', p.x(1))), p.x(0, output_bitmap)),
        'SPen::Bitmap::GetWidth() const': lambda: p.x(0, 40),
        'SPen::Bitmap::GetHeight() const': lambda: p.x(0, 20),
        'SPen::Bitmap::GetBuffer() const': lambda: p.x(0, p.alloc(40 * 20 * 4)),
        'SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)':
            lambda: (events.append(('CreateBitmap', p.x(0), p.x(1), p.x(2), p.x(4))), p.x(0, output_bitmap)),
    })
    if path_present:
        p.call('SPen::ImageCommon::GetImagePath(int) const', [image_common, 0])
        if p.x(0) != path_holder + 16:
            raise AssertionError(f'native ImageCommon path lookup returned {p.x(0):#x}, expected {path_holder + 16:#x}')
        p.call('SPen::ObjectFormulaImpl::GetLatexImagePath() const', [impl])
        if p.x(0) != path_holder + 16:
            raise AssertionError(f'native Model image-path lookup returned {p.x(0):#x}, expected {path_holder + 16:#x}')
    p.call('SPen::ObjectFormulaDrawing::drawFormula(SPen::ISPCanvas&)', [formula_drawing, canvas])
    return {'result': p.x(0), 'events': events}


def fallback_getter_order():
    p = Probe()
    parent, context, canvas, canvas_vtable, formula = (p.alloc(32) for _ in range(5))
    drawing_data = p.alloc(0x100)
    source_list, answer_list = p.alloc(32), p.alloc(32)
    source_strokes = [p.alloc(16), p.alloc(16)]
    answer_strokes = [p.alloc(16)]
    p.put(parent + 8, 'Q', context)
    p.put(canvas, 'Q', canvas_vtable)
    matrix = p.alloc(64)
    p.put(canvas_vtable + 80, 'Q', 0x7000000)
    p.virtual_return = matrix
    observed = []
    drawn_strokes = []
    cursors = {}

    def begin_list():
        cursors[p.x(0)] = 0
        p.x(0, 0)

    def list_data():
        values = source_strokes if p.x(0) == source_list else answer_strokes
        drawn_strokes.append(('get-data', p.x(0), cursors[p.x(0)]))
        p.x(0, values[cursors[p.x(0)]])

    def next_list():
        cursors[p.x(0)] += 1
        p.x(0, cursors[p.x(0)])

    def construct():
        # Outer ObjectDrawing owns this local Drawing facade. Install only the
        # bounded data pointer used by its actual fallback control flow.
        p.put(p.x(0) + 8, 'Q', drawing_data)

    p.callbacks.update({
        'SPen::ObjectFormulaDrawing::Construct(SPen::IContext const*)': construct,
        'SPen::ObjectFormulaDrawing::~ObjectFormulaDrawing()': lambda: None,
        'SPen::ObjectFormulaDrawing::DrawObject(SPen::ISPCanvas&, SPen::ObjectFormula const*)':
            lambda: p.x(0, 0),
        'SPen::ObjectFormula::GetStrokeList() const':
            lambda: (observed.append('source'), p.x(0, source_list)),
        'SPen::ObjectFormula::GetAnswerStrokeList() const':
            lambda: (observed.append('answer'), p.x(0, answer_list)),
        'SPen::ObjectList::GetCount() const':
            lambda: p.x(0, 2 if p.x(0) == source_list else 1),
        'SPen::ObjectList::BeginTraversal() const': begin_list,
        'SPen::ObjectList::GetData(long) const': list_data,
        'SPen::ObjectList::NextData(long) const': next_list,
        'SPen::ObjectList::EndTraversal(long) const': lambda: None,
        'SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)':
            lambda: drawn_strokes.append(('draw-stroke', p.x(3), p.x(4))),
        'SPen::Matrix3<float>::Matrix3(SPen::Matrix3<float> const&)': lambda: None,
    })
    p.call('SPen::ObjectDrawing::drawObjectFormula(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectFormula const*)',
           [parent, canvas, 0, formula])
    return {'list_getters': observed, 'list_data': [entry for entry in drawn_strokes if entry[0] == 'get-data'],
            'draw_stroke_calls': [entry for entry in drawn_strokes if entry[0] == 'draw-stroke'],
            'expected_stroke_order': source_strokes + answer_strokes}


if __name__ == '__main__':
    rows = [
        ('missing-path', False, False, False, False),
        ('missing-path-cache-present', False, True, False, False),
        ('cache-hit', True, True, False, True),
        ('decode-hit', True, False, True, True),
        ('decode-failure', True, False, False, False),
        ('paint-failure', True, True, False, False),
    ]
    results = {}
    for row in rows:
        name, *args = row
        result = one_case(*args)
        results[name] = result
        print(json.dumps({'name': name, **result}, sort_keys=True))
    expected = {
        'missing-path': 0,
        'missing-path-cache-present': 0,
        'cache-hit': 1,
        'decode-hit': 1,
        'decode-failure': 0,
        'paint-failure': 1,
    }
    for name, result in results.items():
        if result['result'] != expected[name]:
            raise AssertionError(f'{name}: drawFormula returned {result["result"]}, expected {expected[name]}')
    if not any(event[0] == 'CreateBitmap' for event in results['cache-hit']['events']):
        raise AssertionError('cache-hit did not reach native getFormulaBitmap/CreateBitmap')
    if results['missing-path-cache-present']['events']:
        raise AssertionError('path gate unexpectedly queried the cache without a path')
    if not results['decode-hit']['events'][:2] == ['cache-query', 'decode']:
        raise AssertionError('cache miss did not fall through to native image-path/decode selection')
    if results['paint-failure']['result'] != 1:
        raise AssertionError('paint-failure incorrectly changed the bitmap-retrieval result')
    paint_event = next(event for event in results['paint-failure']['events'] if isinstance(event, tuple) and event[0] == 'draw-bitmap')
    if paint_event[-1] != 0:
        raise AssertionError('paint-failure control did not return false from the draw callback')
    fallback = fallback_getter_order()
    if fallback['list_getters'] != ['source', 'answer']:
        raise AssertionError(f'unexpected formula stroke-list lookup order: {fallback}')
    actual_strokes = [stroke for _, stroke, _ in fallback['draw_stroke_calls']]
    if actual_strokes != fallback['expected_stroke_order']:
        raise AssertionError(f'unexpected source/answer draw order: {fallback}')
    print(json.dumps({'fallback': fallback}, sort_keys=True))
