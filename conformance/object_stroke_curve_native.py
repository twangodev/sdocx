"""Causal native mapping of serialized stroke property bit 0 to curve state.

This bounded helper-level probe executes the pinned SPen Model DSO's
ObjectStrokeBinaryHandler property reader/writer and ObjectStroke getter/setter.
The binary handler receiver and ObjectStrokeImpl storage use the recovered
native offsets; the test does not invoke full record or SDK deserialization.

Run from the repository root with the pinned research tools:
  PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/apk-analysis-runtime/binutils-aarch64/usr/bin \
    LD_LIBRARY_PATH=scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu \
    python3 conformance/object_stroke_curve_native.py --check-fixture
"""
import argparse
import hashlib
import json
import os
import re
import struct
import subprocess
from pathlib import Path

from unicorn import UC_ARCH_ARM64, UC_HOOK_CODE, UC_MODE_ARM, Uc
from unicorn import arm64_const as R

ROOT = Path(__file__).resolve().parents[1]
LIBRARY = ROOT / "scratch/apk-analysis-native/arm64-v8a/libSPenModel.so"
OBJDUMP = ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump"
FIXTURE = ROOT / "conformance/object-stroke-curve-native.json"
MODEL_SHA256 = "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a"
BASE = 0x7000000
STOP = 0x200F1000
STACK = 0x200EF000
MASKS = (0x00, 0x01, 0x80, 0x81)

METHODS = {
    "apply_property": "SPen::ObjectStrokeBinaryHandler::m_ApplyBinary_Property(int&, bool&)",
    "write_property": "SPen::ObjectStrokeBinaryHandler::m_GetBinary_Property(unsigned int&) const",
    "is_curve_enabled": "SPen::ObjectStroke::IsCurveEnabled() const",
    "set_curve_enabled": "SPen::ObjectStroke::SetCurveEnabled(bool)",
    "handler_constructor": "SPen::ObjectStrokeBinaryHandler::ObjectStrokeBinaryHandler(SPen::ObjectStrokeImpl*)",
}


def load_model():
    raw = LIBRARY.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    if digest != MODEL_SHA256:
        raise RuntimeError(f"unverified libSPenModel.so: {digest}")
    phoff = struct.unpack_from("<Q", raw, 32)[0]
    phent, phnum = struct.unpack_from("<HH", raw, 54)
    segments = []
    extent = 0
    for i in range(phnum):
        kind, flags, offset, vaddr, paddr, filesz, memsz, align = struct.unpack_from(
            "<II6Q", raw, phoff + i * phent
        )
        if kind == 1 and memsz:
            segments.append((offset, vaddr, filesz, memsz))
            extent = max(extent, vaddr + memsz)

    uc = Uc(UC_ARCH_ARM64, UC_MODE_ARM)
    uc.mem_map(BASE, (extent + 0xFFF) & ~0xFFF)
    uc.mem_map(0x20000000, 0x100000)
    for offset, vaddr, filesz, memsz in segments:
        if filesz:
            uc.mem_write(BASE + vaddr, raw[offset : offset + filesz])
        if memsz > filesz:
            uc.mem_write(BASE + vaddr + filesz, bytes(memsz - filesz))
    uc.reg_write(R.UC_ARM64_REG_TPIDR_EL0, 0x20000000)

    runtime = ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu"
    env = dict(os.environ)
    if runtime.is_dir():
        env["LD_LIBRARY_PATH"] = str(runtime.resolve())
    symbols = {}
    for line in subprocess.check_output(["nm", "-D", "-C", str(LIBRARY)], text=True, env=env).splitlines():
        parts = line.strip().split(" ", 2)
        if len(parts) == 3 and parts[1] in ("T", "W"):
            symbols[parts[2]] = BASE + int(parts[0], 16)
    disassembly = subprocess.check_output(
        [str(OBJDUMP), "-d", "-C", "-j", ".plt", str(LIBRARY)], text=True, env=env
    )
    plt = {BASE + int(address, 16): name for address, name in re.findall(r"^([0-9a-f]+) <(.+)@plt>:", disassembly, re.M)}
    return uc, symbols, plt, digest


def generate():
    uc, symbols, plt, digest = load_model()
    for name in METHODS.values():
        if name not in symbols:
            raise RuntimeError(f"missing native symbol: {name}")

    # ObjectStroke::SetCurveEnabled notifies ObjectBase after changing the
    # recovered Impl byte. The notification methods are explicit host no-ops;
    # all field access and curve methods below execute in the pinned DSO.
    host_callbacks = {
        "SPen::ObjectBase::SetChanged()": "void notification suppressed",
        "SPen::ObjectBase::SetRedrawingCacheDirtyFlag(bool)": "void notification suppressed",
    }
    host_calls = []

    def hook(u, address, _size, _data):
        name = plt.get(address)
        if name is None:
            return
        if name in host_callbacks:
            host_calls.append(name)
            u.reg_write(R.UC_ARM64_REG_X0, 0)
            u.reg_write(R.UC_ARM64_REG_PC, u.reg_read(R.UC_ARM64_REG_LR))
            return
        raise RuntimeError(f"unexpected native import: {name}")

    uc.hook_add(UC_HOOK_CODE, hook)

    # Recovered ABI/layout: ObjectStroke+32 -> Impl; handler+0 is vptr and
    # handler+8 -> Impl. Impl's saved curve byte is at +40.
    obj = 0x10020000
    impl = 0x10021000
    handler = 0x10022000
    uc.mem_map(0x10020000, 0x10000)
    uc.reg_write(R.UC_ARM64_REG_SP, STACK)

    def call(name, args):
        for index, value in enumerate(args):
            uc.reg_write(R.UC_ARM64_REG_X0 + index, value)
        uc.reg_write(R.UC_ARM64_REG_SP, STACK)
        uc.reg_write(R.UC_ARM64_REG_LR, STOP)
        uc.emu_start(symbols[name], STOP, count=100_000)
        if uc.reg_read(R.UC_ARM64_REG_PC) != STOP:
            raise RuntimeError(f"{name} did not return: PC={uc.reg_read(R.UC_ARM64_REG_PC):#x}")

    uc.mem_write(obj + 32, struct.pack("<Q", impl))
    # This constructor initializes the native handler vptr and stores Impl at
    # +8. Its vptr relocation is not needed by the two helper methods.
    call(METHODS["handler_constructor"], (handler, impl))
    handler_impl = struct.unpack("<Q", uc.mem_read(handler + 8, 8))[0]
    if handler_impl != impl:
        raise RuntimeError(f"native handler Impl pointer mismatch: {handler_impl:#x} != {impl:#x}")
    getter_address = symbols[METHODS["is_curve_enabled"]]
    setter_address = symbols[METHODS["set_curve_enabled"]]
    cases = []
    for word in MASKS:
        source = 0x10023000
        result_flag = 0x10023010
        output = 0x10023020
        uc.mem_write(source, struct.pack("<I", word))
        uc.mem_write(result_flag, bytes(4))
        uc.mem_write(output, bytes(4))
        call(METHODS["apply_property"], (handler, source, result_flag))
        call(METHODS["is_curve_enabled"], (obj,))
        getter = uc.reg_read(R.UC_ARM64_REG_X0) & 0xFFFFFFFF
        impl_curve = uc.mem_read(impl + 40, 1)[0]
        call(METHODS["write_property"], (handler, output))
        encoded = struct.unpack("<I", uc.mem_read(output, 4))[0]
        expected_flag = word & 1
        if impl_curve != expected_flag or getter != expected_flag:
            raise RuntimeError(f"property word {word:#x} mapped to wrong curve state")
        if encoded != word:
            raise RuntimeError(f"property word {word:#x} wrote {encoded:#x}")
        cases.append({
            "input_property_word": word,
            "input_property_bytes_le": struct.pack("<I", word).hex(),
            "impl_curve_field_offset": 40,
            "impl_curve_byte": impl_curve,
            "getter_value": getter,
            "writer_property_word": encoded,
            "writer_property_bytes_le": struct.pack("<I", encoded).hex(),
        })

    setter_cases = []
    for value in (0, 1, 0):
        call(METHODS["set_curve_enabled"], (obj, value))
        call(METHODS["is_curve_enabled"], (obj,))
        getter = uc.reg_read(R.UC_ARM64_REG_X0) & 0xFFFFFFFF
        actual_byte = uc.mem_read(impl + 40, 1)[0]
        if actual_byte != value or getter != value:
            raise RuntimeError(f"native setter {value} wrote {actual_byte} / getter returned {getter}")
        setter_cases.append({"setter_input": value, "impl_curve_byte": actual_byte, "getter_value": getter})

    return {
        "schema": "sdocx-object-stroke-curve-native-v1",
        "runner": {"path": str(Path(__file__).resolve().relative_to(ROOT)), "sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()},
        "library": {"path": str(LIBRARY.relative_to(ROOT)), "sha256": digest},
        "native_entries": {
            name: {"symbol": symbol, "address": hex(symbols[symbol]), "rva": hex(symbols[symbol] - BASE)}
            for name, symbol in METHODS.items()
        },
        "receiver_layout": {
            "object_stroke_address": hex(obj),
            "object_stroke_impl_pointer_offset": 32,
            "handler_address": hex(handler),
            "handler_impl_pointer_offset": 8,
            "impl_address": hex(impl),
            "impl_curve_byte_offset": 40,
            "handler_impl_pointer_after_constructor": hex(handler_impl),
            "allocation": "host zeroed memory with pointers installed at the recovered offsets; native handler constructor ran",
        },
        "property_word_cases": cases,
        "setter_cases": setter_cases,
        "host_boundaries": host_callbacks,
        "host_calls": host_calls,
        "scope": "Native helper-level property mapping only. No full binary record roundtrip, SDK deserialization, or Android document loading.",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write-fixture", action="store_true")
    mode.add_argument("--check-fixture", action="store_true")
    args = parser.parse_args()
    actual = generate()
    if args.write_fixture:
        FIXTURE.write_text(json.dumps(actual, indent=2, sort_keys=True) + "\n")
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
    else:
        expected = json.loads(FIXTURE.read_text())
        if actual != expected:
            raise SystemExit("native ObjectStroke curve property fixture mismatch")
        print("verified native property words 0x00, 0x01, 0x80, and 0x81 plus setter/getter controls")


if __name__ == "__main__":
    main()
