"""Hash-pinned Model empty-flex reader and Pen::SetSize defaults oracle.

Model defaults are loaded by the real ObjectStrokeBinaryHandler into an
ObjectStrokeImpl created by its native constructors. Pen size forwarding is
checked through the pinned PenCommon implementation with a real Pen ctor.
Allocator and ELF/PLT execution are provided by the existing harness.
"""
import argparse
import hashlib
import json
import os
import re
import struct
import subprocess
from pathlib import Path

import fountain_native
from fountain_native import NativeFountain

ROOT = Path(__file__).resolve().parents[1]
LIB = ROOT / 'scratch/apk-output/samsung-notes-4.4.45.37/lib/arm64-v8a'
OBJDUMP = ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'
MODEL_SHA = '4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a'
BASE_SHA = 'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb'
PENCOMMON_SHA = fountain_native.HASHES['PenCommon']
CALLIGRAPHY_SHA = '75aa5312fba02e21ff01d713187d2fd8d015ac4781e2d58da85ae700114e6f42'


def model_reader():
    # This existing wrapper pins and maps Base, Model, and Calligraphy; no
    # wrapper redraw runs on import. Its host callbacks cover only ELF support.
    import calligraphy_saved_wrapper_native as wrapper
    n = wrapper.n
    if hashlib.sha256((LIB / 'libSPenModel.so').read_bytes()).hexdigest() != MODEL_SHA:
        raise RuntimeError('Model SHA mismatch')
    if hashlib.sha256((LIB / 'libSPenBase.so').read_bytes()).hexdigest() != BASE_SHA:
        raise RuntimeError('Base SHA mismatch')
    obj, impl, handler = n.alloc(256), n.alloc(512), n.alloc(256)
    pointer, empty = n.alloc(8), n.alloc(4)
    n.call('SPen::ObjectStroke::ObjectStroke()', [obj])
    n.call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)', [impl, obj])
    n.put(obj + 32, 'Q', impl)
    n.call('SPen::ObjectStrokeBinaryHandler::ObjectStrokeBinaryHandler(SPen::ObjectStrokeImpl*)', [handler, impl])
    n.put(pointer, 'Q', empty)
    before = n.get(impl, 'I' * 128)
    ret = n.call('SPen::ObjectStrokeBinaryHandler::m_ApplyBinary_FlexibleData(unsigned char**, int, unsigned int, float) const',
                 [handler, pointer, 0, 0], [1.0])
    reader_return = n.x(0)
    if reader_return != 1 or n.get(pointer, 'Q')[0] != empty:
        raise RuntimeError('empty flexible reader did not succeed without advancing input')
    reader_after = n.get(impl, 'I' * 128)
    changed = [i * 4 for i, (a, b) in enumerate(zip(before, reader_after)) if a != b]
    # Getter return values are recorded from the ABI result registers, not
    # inferred from the implementation field names.
    gets = {
        'pen_size': ('SPen::ObjectStroke::GetPenSize() const', 'S0'),
        'color_argb': ('SPen::ObjectStroke::GetColor() const', 'X0'),
        'color_type': ('SPen::ObjectStroke::GetColorType() const', 'X0'),
        'stroke_type': ('SPen::ObjectStroke::GetStrokeType() const', 'X0'),
        'tool_type': ('SPen::ObjectStroke::GetToolType() const', 'X0'),
        'curve_enabled': ('SPen::ObjectStroke::IsCurveEnabled() const', 'X0'),
        'fixed_width_enabled': ('SPen::ObjectStroke::IsFixedWidthEnabled() const', 'X0'),
        'fixed_opacity_enabled': ('SPen::ObjectStroke::IsFixedOpacityEnabled() const', 'X0'),
    }
    from unicorn import arm64_const as R
    got = {}
    for key, (name, reg) in gets.items():
        if name not in n.syms:
            raise RuntimeError('missing native getter: ' + name)
        n.call(name, [obj])
        value = n.u.reg_read(R.UC_ARM64_REG_S0) if reg == 'S0' else n.x(0)
        got[key] = struct.pack('<I', value).hex() if reg == 'S0' else value
    # Getter methods for these pointers are called directly on ObjectStroke.
    null_getters = {}
    for key, name in [('pen_name', 'SPen::ObjectStroke::GetPenName() const'),
                      ('advanced_settings', 'SPen::ObjectStroke::GetAdvancedPenSetting() const')]:
        if name not in n.syms:
            raise RuntimeError('missing native getter: ' + name)
        n.call(name, [obj]); null_getters[key] = n.x(0) == 0
    field_offsets = {'name_id': 16, 'curve_enabled': 40, 'argb': 288,
                     'pen_size': 292, 'tool_type': 316, 'stroke_type': 464,
                     'color_type': 488}
    fields = {key: struct.pack('<I', reader_after[offset // 4]).hex()
              for key, offset in field_offsets.items()}
    # Nonzero ABI control: prove GetPenSize is read from S0 as raw f32 bits,
    # rather than packing a floating interpretation of the register integer.
    n.call('SPen::ObjectStroke::SetPenSize(float)', [obj], [6.0])
    n.call('SPen::ObjectStroke::GetPenSize() const', [obj])
    getter_control_bits = struct.pack('<I', n.u.reg_read(R.UC_ARM64_REG_S0)).hex()
    return {'case': 'empty-flex-defaults', 'reader_return': reader_return,
            'input': {'length': 0, 'flags': 0, 'scale_bits': struct.pack('<f', 1.0).hex()},
            'input_pointer_unchanged': True, 'buffer_unchanged': n.get(empty, 'I')[0] == 0,
            'native_getters': got, 'get_pen_size_nonzero_control_bits': getter_control_bits,
            'null_getters': null_getters,
            'implementation_fields_u32le': fields,
            'constructor_to_reader_changed_offsets': changed}


def pen_size_forwarding():
    # Fresh runtime, before importing the Calligraphy wrapper (which enlarges
    # Model's mapping). This is the actual generic PenCommon constructor and
    # setters; it does not claim Calligraphy ctor/profile min-size state.
    n = NativeFountain(LIB, OBJDUMP)
    n.callbacks.update({
        'operator new(unsigned long, std::nothrow_t const&)': lambda: n.x(0, n.alloc(n.x(0))),
        'operator new[](unsigned long, std::nothrow_t const&)': lambda: n.x(0, n.alloc(n.x(0))),
        '__cxa_atexit': lambda: n.x(0, 0),
        '__cxa_guard_acquire': lambda: n.x(0, 1),
        '__cxa_guard_release': lambda: None,
        '__cxa_guard_abort': lambda: None,
        'strlen': lambda: n.x(0, len(n.u.mem_read(n.x(0), 4096).split(b'\0', 1)[0])),
    })
    pen = n.alloc(256)
    n.call('SPen::Pen::Pen()', [pen])
    ctor_default = struct.pack('<f', n.get(pen + 24, 'f')[0]).hex()
    cases = []
    from unicorn import arm64_const as R
    for value in (0.0, 0.4, 6.0):
        n.call('SPen::Pen::SetSize(float)', [pen], [value])
        setter_return = n.x(0)
        n.call('SPen::Pen::GetSize()', [pen])
        bits = n.u.reg_read(R.UC_ARM64_REG_S0)
        cases.append({'input_bits': struct.pack('<f', value).hex(),
                      'setter_return': setter_return, 'getter_bits': struct.pack('<I', bits).hex()})
    return {'constructor_default_bits': ctor_default, 'cases': cases,
            'boundary': 'real PenCommon Pen ctor/SetSize/GetSize; Calligraphy-specific constructor min-size fields are outside this case'}


def calligraphy_profile_vtable_evidence():
    # The profile factory itself could not be run in the bounded Model harness;
    # verify the pinned DSO's factory/constructor callsite and its vtable slot
    # relocation instead of attributing the generic Pen probe to the profile.
    dso = LIB / 'libSPenMontblancCalligraphyPen.so'
    env = dict(os.environ)
    env['LD_LIBRARY_PATH'] = str((ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu').resolve())
    objdump = str(OBJDUMP)
    dis = subprocess.check_output([objdump, '-d', '-C', '--start-address=0x1c9b4',
                                   '--stop-address=0x1cb04', str(dso)], env=env, text=True)
    factory = subprocess.check_output([objdump, '-d', '-C', '--start-address=0x1cf5c',
                                       '--stop-address=0x1cf88', str(dso)], env=env, text=True)
    readelf = ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-readelf'
    relocs = subprocess.check_output([str(readelf), '-rW', str(dso)], env=env, text=True)
    if 'fmov\ts0, #4.000000000000000000e+00' not in dis or 'ldr\tx8, [x8, #16]' not in dis or 'blr\tx8' not in dis:
        raise RuntimeError('Calligraphy ctor size setter callsite changed')
    if not re.search(r'000000000002a2d8\s+\S+\s+R_AARCH64_ABS64\s+\S+\s+_ZN4SPen3Pen7SetSizeEf', relocs):
        raise RuntimeError('Calligraphy vtable SetSize relocation changed')
    if 'MontblancCalligraphyPen::MontblancCalligraphyPen()@plt' not in factory:
        raise RuntimeError('createPenInst no longer invokes the Calligraphy constructor')
    return {'evidence': 'pinned Calligraphy DSO disassembly and dynamic vtable relocation',
            'createPenInst': {'address': '0x1cf5c', 'alloc_bytes': 144,
                              'calls_ctor_at': '0x1cf74'},
            'constructor': {'address': '0x1c9b4', 'initializes_Pen': '0x1c9d8',
                            'vtable_address_point': 'vtable+0x10',
                            'SetSize_vtable_slot_offset': '0x10',
                            'SetSize_relocation_address': '0x2a2d8',
                            'constructor_argument_bits': '00008040',
                            'argument_load_pc': '0x1ca7c',
                            'indirect_call_pc': '0x1ca84'},
            'limit': 'proves profile construction dispatches Pen::SetSize(4.0f); does not execute createPenInst or prove later saved-size clamp under profile-specific minimums'}


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    mode = ap.add_mutually_exclusive_group(required=True)
    mode.add_argument('--write-fixture', action='store_true')
    mode.add_argument('--check-fixture', action='store_true')
    ap.add_argument('--fixture', type=Path, default=ROOT / 'conformance/object-stroke-defaults-native.json')
    args = ap.parse_args()
    source = {'model_sha256': MODEL_SHA, 'base_sha256': BASE_SHA,
              'pencommon_sha256': PENCOMMON_SHA,
              'calligraphy_sha256': CALLIGRAPHY_SHA,
              'loaded_library_paths': {
                  'Model': str((LIB / 'libSPenModel.so').relative_to(ROOT)),
                  'Base': str((LIB / 'libSPenBase.so').relative_to(ROOT)),
                  'PenCommon': str((LIB / 'libSPenPenCommon.so').relative_to(ROOT)),
                  'MontblancCalligraphyPen': str((LIB / 'libSPenMontblancCalligraphyPen.so').relative_to(ROOT)),
              }}
    pen_defaults = pen_size_forwarding()
    model_case = model_reader()
    actual = {'schema': 'sdocx-object-stroke-defaults-native-v1', 'source': source,
              'cases': [model_case], 'pen_size_forwarding': pen_defaults,
              'calligraphy_profile_size_vtable': calligraphy_profile_vtable_evidence()}
    if args.write_fixture:
        args.fixture.write_text(json.dumps(actual, indent=2) + '\n')
    else:
        expected = json.loads(args.fixture.read_text())
        if expected != actual:
            raise SystemExit('native defaults fixture mismatch')
    print(f"{len(actual['cases'])} Model default case and Pen size forwarding {'matched' if args.check_fixture else 'written'}")


if __name__ == '__main__':
    main()
