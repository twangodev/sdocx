"""Verify saved Pencil3 MotionEvent construction with native Base code.

Reuses the saved-array inputs from pencil-v1-native.json, but executes the
hash-pinned SPen::MotionEvent array constructor and source/tool/history getters
instead of the host getter model used by the geometry oracle. Only bounded
operator allocation is stubbed; no JNI or MotionEvent semantic imports are.

Run from the repository root:
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/pencil_event_native.py
"""
import argparse
import json
import shutil
import struct
from pathlib import Path

from pencil_native import MOTION_EVENT_ARRAY, NativePencil3

SOURCE = 'SPen::MotionEvent::GetSource() const'
TOOL = 'SPen::MotionEvent::GetToolType(int) const'
HISTORY = 'SPen::MotionEvent::GetHistorySize() const'
ALLOC = 'operator new(unsigned long)'
DELETE = 'operator delete(void*)'


def array(native, fmt, values):
    address = native.alloc(struct.calcsize('<' + fmt))
    native.put(address, fmt, *values)
    return address


def verify_case(library_dir, objdump, case):
    native = NativePencil3(library_dir, objdump)

    # NativeFountain routes imported symbols through callbacks before native
    # bodies. Retain only bounded allocation/deallocation callbacks so all
    # MotionEvent semantics execute from the pinned Base ELF.
    native.callbacks = {
        ALLOC: lambda: native.x(0, native.alloc(native.x(0))),
        DELETE: lambda: None,
    }

    stroke = case['stroke']
    count = len(stroke['points'])
    event = native.alloc(8)
    points = array(native, f'{2 * count}f', [v for point in stroke['points'] for v in point])
    pressures = array(native, f'{count}f', stroke['pressures'])
    timestamps = array(native, f'{count}i', stroke['timestamps'])
    tilts = array(native, f'{count}f', stroke['tilts']) if stroke['tilts'] else 0
    orientations = array(native, f'{count}f', stroke['orientations']) if stroke['orientations'] else 0

    # AArch64 places argument 9 (orientation pointer) at the caller's SP.
    native.put(0x200f0000, 'Q', orientations)
    native.call(MOTION_EVENT_ARRAY, [
        event, stroke['tool_type'], count, points, pressures,
        stroke['timestamps'][0], timestamps, tilts,
    ], instruction_limit=10_000_000)

    inner = native.get(event, 'Q')[0]
    source_field = native.get(inner + 8, 'i')[0]
    down_time = native.get(inner + 16, 'q')[0]
    native.call(SOURCE, [event])
    source_getter = native.x(0)
    native.call(TOOL, [event, 0])
    tool_type = native.x(0)
    native.call(HISTORY, [event])
    history_size = native.x(0)

    if source_field != 0 or source_getter != 0:
        raise AssertionError(f"{case['name']}: source field/getter {source_field}/{source_getter}, expected 0/0")
    if tool_type != stroke['tool_type']:
        raise AssertionError(f"{case['name']}: native tool type {tool_type}, expected {stroke['tool_type']}")
    if history_size != count - 1:
        raise AssertionError(f"{case['name']}: native history size {history_size}, expected {count - 1}")
    if down_time != stroke['timestamps'][0]:
        raise AssertionError(f"{case['name']}: native down time {down_time}, expected {stroke['timestamps'][0]}")
    return tool_type


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--library-dir', type=Path, default=Path('scratch/apk-analysis-native/arm64-v8a'))
    parser.add_argument('--objdump', type=Path, default=Path(shutil.which('aarch64-linux-gnu-objdump') or
                                                               'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'))
    parser.add_argument('--fixture', type=Path, default=Path('conformance/pencil-v1-native.json'))
    args = parser.parse_args()

    fixture = json.loads(args.fixture.read_text())
    cases = fixture.get('saved_cases', [])
    if not cases:
        raise SystemExit('fixture contains no saved_cases')
    tools = [verify_case(args.library_dir, args.objdump, case) for case in cases]
    print(f'{len(cases)} saved MotionEvent cases match native construction; '
          f'{sum(tool == 2 for tool in tools)} tool-2 cases have source 0')


if __name__ == '__main__':
    main()
