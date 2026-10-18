#!/usr/bin/env python3
"""Execute the native stroke property reader/getter and pin Drawing forwarding.

Run from the repository root with extracted APK libraries and the local Unicorn:
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/stroke_curve_flag_native.py --check-fixture
"""
import argparse
import hashlib
import json
import struct
from pathlib import Path

from unicorn import Uc, UC_ARCH_ARM64, UC_MODE_ARM
from unicorn import arm64_const as R

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_LIBS = ROOT / "scratch/apk-analysis-native/arm64-v8a"
FIXTURE = ROOT / "conformance/stroke-curve-flag.json"
MODEL_SHA256 = "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a"
DRAWING_SHA256 = "788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd"
MODEL_BASE = 0x40000000
DRAWING_BASE = 0x50000000
HEAP = 0x10000000
STACK = 0x30000000
STOP = STACK + 0x1000
HANDLER = HEAP + 0x1000
IMPL = HEAP + 0x2000
WRAPPER = HEAP + 0x3000
MASK = HEAP + 0x4000
STYLUS_OUTPUT = HEAP + 0x5000
APPLY_BINARY_PROPERTY = 0x2ED138
IS_CURVE_ENABLED = 0x2E067C
DRAWING_FORWARDING_OFFSET = 0x81F08
DRAWING_FORWARDING_WORDS = [
    0xAA1403E0,  # mov x0, x20 (stroke receiver)
    0x9400E185,  # bl ObjectStroke::IsCurveEnabled
    0xF94002C8,  # ldr x8, [x22] (drawable vtable)
    0xF9400908,  # ldr x8, [x8, #16] (curve-enabled draw slot)
    0x12000001,  # and w1, w0, #1 (forward bool)
    0xAA1603E0,  # mov x0, x22 (drawable receiver)
    0xD63F0100,  # blr x8
]


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def map_elf(uc: Uc, path: Path, base: int) -> None:
    data = path.read_bytes()
    phoff = struct.unpack_from("<Q", data, 32)[0]
    phentsize, phnum = struct.unpack_from("<HH", data, 54)
    for index in range(phnum):
        p_type, _, offset, vaddr, _, filesz, memsz, _ = struct.unpack_from(
            "<II6Q", data, phoff + index * phentsize
        )
        if p_type != 1 or memsz == 0:
            continue
        uc.mem_write(base + vaddr, data[offset : offset + filesz])
        if memsz > filesz:
            uc.mem_write(base + vaddr + filesz, bytes(memsz - filesz))


def call(uc: Uc, address: int, arguments: tuple[int, ...]) -> int:
    for index, value in enumerate(arguments):
        uc.reg_write(R.UC_ARM64_REG_X0 + index, value)
    uc.reg_write(R.UC_ARM64_REG_SP, STACK + 0x8000)
    uc.reg_write(R.UC_ARM64_REG_LR, STOP)
    uc.emu_start(address, STOP, count=100_000)
    if uc.reg_read(R.UC_ARM64_REG_PC) != STOP:
        raise RuntimeError(f"native function {address:#x} did not return")
    return uc.reg_read(R.UC_ARM64_REG_X0)


def capture_case(model: Path, mask: int) -> dict[str, object]:
    uc = Uc(UC_ARCH_ARM64, UC_MODE_ARM)
    uc.mem_map(MODEL_BASE, 0x600000)
    uc.mem_map(HEAP, 0x10000)
    uc.mem_map(STACK, 0x10000)
    map_elf(uc, model, MODEL_BASE)
    # The native reader receives ObjectStrokeBinaryHandler* whose +8 field is
    # ObjectStrokeImpl*. IsCurveEnabled receives an outer ObjectStroke shell
    # whose +32 field points to the same implementation.
    uc.mem_write(HANDLER + 8, struct.pack("<Q", IMPL))
    uc.mem_write(WRAPPER + 32, struct.pack("<Q", IMPL))
    uc.mem_write(MASK, struct.pack("<I", mask))
    uc.mem_write(IMPL + 40, b"\xff")
    uc.mem_write(STYLUS_OUTPUT, b"\xff")
    call(uc, MODEL_BASE + APPLY_BINARY_PROPERTY, (HANDLER, MASK, STYLUS_OUTPUT))
    member_byte = uc.mem_read(IMPL + 40, 1)[0]
    getter = call(uc, MODEL_BASE + IS_CURVE_ENABLED, (WRAPPER,))
    stylus_output = uc.mem_read(STYLUS_OUTPUT, 1)[0]
    expected_curve = mask & 1
    expected_stylus = (mask >> 2) & 1
    if member_byte != expected_curve or getter != expected_curve:
        raise AssertionError(f"mask {mask:#x}: curve byte/getter {member_byte}/{getter}, expected {expected_curve}")
    if stylus_output != expected_stylus:
        raise AssertionError(f"mask {mask:#x}: stylus byte {stylus_output}, expected {expected_stylus}")
    return {
        "property_mask_u32": mask,
        "impl_curve_byte_at_40": member_byte,
        "is_curve_enabled": getter,
        "stylus_output_byte": stylus_output,
    }


def capture(model: Path, drawing: Path) -> dict[str, object]:
    model_hash = sha256(model)
    drawing_hash = sha256(drawing)
    if model_hash != MODEL_SHA256:
        raise ValueError(f"unverified Model library: {model_hash}")
    if drawing_hash != DRAWING_SHA256:
        raise ValueError(f"unverified Drawing library: {drawing_hash}")

    uc = Uc(UC_ARCH_ARM64, UC_MODE_ARM)
    uc.mem_map(DRAWING_BASE, 0x600000)
    map_elf(uc, drawing, DRAWING_BASE)
    observed_words = [
        struct.unpack("<I", uc.mem_read(DRAWING_BASE + DRAWING_FORWARDING_OFFSET + i * 4, 4))[0]
        for i in range(len(DRAWING_FORWARDING_WORDS))
    ]
    if observed_words != DRAWING_FORWARDING_WORDS:
        raise ValueError(f"Drawing forwarding instruction sequence changed: {observed_words!r}")

    return {
        "schema": "samsung-notes-stroke-curve-flag-native-v1",
        "libraries": {"Model": model_hash, "Drawing": drawing_hash},
        "entrypoints": {
            "apply_binary_property": "Model+0x2ed138",
            "is_curve_enabled": "Model+0x2e067c",
            "draw_object_stroke_forwarding": "Drawing+0x81f08",
        },
        "cases": [capture_case(model, mask) for mask in (0, 1, 2, 3, 0xFFFFFFFF)],
        "drawing_forwarding_words_le": [f"{word:08x}" for word in observed_words],
        "boundary": [
            "Model property reader/getter execute in Unicorn with bounded handler/implementation/wrapper shells; handler+8 and wrapper+32 are supplied ABI pointers.",
            "Drawing forwarding is checked as exact instructions from the pinned binary, not executed through a host drawable callback.",
            "The parser-to-StrokeRendering assignment is source checked separately: decode_stroke uses StrokeProperties::read(frame.properties), where compressed reads property bit 0.",
        ],
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--write-fixture", action="store_true")
    group.add_argument("--check-fixture", action="store_true")
    parser.add_argument("--model", type=Path, default=DEFAULT_LIBS / "libSPenModel.so")
    parser.add_argument("--drawing", type=Path, default=DEFAULT_LIBS / "libSPenDrawing.so")
    args = parser.parse_args()
    encoded = json.dumps(capture(args.model, args.drawing), indent=2, separators=(",", ": ")) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
    elif FIXTURE.read_text() != encoded:
        raise SystemExit("native stroke curve flag differs from conformance/stroke-curve-flag.json")
    else:
        print(f"verified {FIXTURE.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
