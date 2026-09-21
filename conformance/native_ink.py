"""Bounded native oracle for FountainPen V16's width limiter.

Run from the repository root with Unicorn installed, for example:
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/native_ink.py

This executes only the identified pure ARM64 helper. It does not validate the
complete pen renderer, width-history manager, sample selection or shader.
"""
import argparse
import hashlib
import random
import struct
from pathlib import Path

LIBRARY_SHA256 = "1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec"
START, END, RETURN = 0x79298, 0x792F4, 0x203000


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def reference(size, previous, previous_pressure, stylus, delta, step, target, pressure):
    """Float32 reconstruction of the helper, not a pressure-to-width law."""
    if delta > step:
        target = f32(previous + (-step if previous > target else step))
    width = max(f32(size / 3), max(target, f32(size * pressure)))
    return previous if not stylus and previous_pressure == pressure else width


def function_bytes(data):
    if data[:6] != b"\x7fELF\x02\x01":
        raise ValueError("expected little-endian ELF64")
    offset = struct.unpack_from("<Q", data, 32)[0]
    stride, count = struct.unpack_from("<HH", data, 54)
    for index in range(count):
        kind, _, file_offset, address, _, size, _, _ = struct.unpack_from(
            "<II6Q", data, offset + index * stride
        )
        if kind == 1 and address <= START and END <= address + size:
            begin = file_offset + START - address
            return data[begin:begin + END - START]
    raise ValueError("helper is outside the executable image")


def run(library):
    from unicorn import Uc, UC_ARCH_ARM64, UC_MODE_ARM, UC_HOOK_CODE
    from unicorn import arm64_const as reg

    data = library.read_bytes()
    if hashlib.sha256(data).hexdigest() != LIBRARY_SHA256:
        raise ValueError("library hash differs from the inspected APK; reverify addresses")
    machine = Uc(UC_ARCH_ARM64, UC_MODE_ARM)
    machine.mem_map(0x79000, 0x1000)
    machine.mem_write(START, function_bytes(data))
    machine.mem_map(0x200000, 0x4000)
    machine.mem_write(0x200000 + 72, struct.pack("<Q", 0x201000))
    machine.mem_write(0x201000, struct.pack("<Q", 0x202000))
    visited = set()

    def bounded(_, address, size, __):
        if not START <= address < END:
            raise AssertionError(f"unexpected instruction {address:#x}")
        visited.add(address)

    machine.hook_add(UC_HOOK_CODE, bounded)
    cases = []
    for stylus in (False, True):
        for pressure in (0.0, 0.25, 0.7, 1.0):
            for target in (0.0, 2.0, 3.0, 12.0):
                cases.append((9., 3., pressure, stylus, abs(3.-target), 2., target, pressure))
    rng = random.Random(0x79298)
    for _ in range(4096):
        size, previous, step, target = [rng.uniform(0.1, 30) for _ in range(4)]
        cases.append((size, previous, rng.random(), bool(rng.getrandbits(1)),
                      abs(previous-target), step, target, rng.random()))
    for raw in cases:
        case = tuple(value if isinstance(value, bool) else f32(value) for value in raw)
        size, previous, previous_pressure, stylus, delta, step, target, pressure = case
        machine.mem_write(0x202000, struct.pack("<f", size))
        machine.mem_write(0x200000 + 104, struct.pack("<ff", previous, previous_pressure))
        machine.mem_write(0x200000 + 272, bytes([stylus]))
        machine.reg_write(reg.UC_ARM64_REG_X0, 0x200000)
        machine.reg_write(reg.UC_ARM64_REG_LR, RETURN)
        for i, value in enumerate((delta, step, target, pressure, 0.0)):
            machine.reg_write(getattr(reg, f"UC_ARM64_REG_S{i}"), struct.unpack("<I", struct.pack("<f", value))[0])
        machine.emu_start(START, RETURN, count=64)
        if machine.reg_read(reg.UC_ARM64_REG_PC) != RETURN:
            raise AssertionError("helper failed to return within instruction limit")
        actual = machine.reg_read(reg.UC_ARM64_REG_S0)
        expected = struct.unpack("<I", struct.pack("<f", reference(*case)))[0]
        if actual != expected:
            raise AssertionError((case, hex(actual), hex(expected)))
    print(f"{len(cases)} bit-exact width-limiter cases; {len(visited)} native instructions exercised")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--library", type=Path, default=Path("scratch/apk-analysis-native/arm64-v8a/libSPenFountainPen.so"))
    run(parser.parse_args().library)
