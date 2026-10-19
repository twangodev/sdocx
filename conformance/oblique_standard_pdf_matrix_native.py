"""Native Standard ObjectPDFWriter matrix and Oblique inverse-scale witness.

The pinned ObjectPDFWriter body runs natively. Host callbacks provide the
PDF-writer rectangle, queue, bitmap factory result, Drawing object, and the
DrawObjectList virtual return. The captured matrix is replayed through native
Drawing/Graphics/Oblique methods by the committed canvas harness. Rectangles
and scales are explicit probes, not claimed device export dimensions.

Run from the repository root with the project research runtime:
  PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/apk-analysis-runtime/binutils-aarch64/usr/bin \
    python3 conformance/oblique_standard_pdf_matrix_native.py --check-fixture
"""
import argparse
import json
import struct
from pathlib import Path

from unicorn import UC_HOOK_CODE
from unicorn import arm64_const as R

from oblique_canvas_native import CanvasProbe, STOP
import fountain_native as native

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "conformance/oblique-standard-pdf-matrix-native.json"

CASES = [
    {"id": "unit_dimensions", "rect": [0.0, 0.0, 1812.0, 15372.0], "scale": 1.0},
    {"id": "scaled_dimensions", "rect": [0.0, 0.0, 906.0, 7686.0], "scale": 0.5},
    {"id": "translated_same_extent", "rect": [100.0, 200.0, 1006.0, 7886.0], "scale": 0.5},
]
QUEUE = 0x200F3010
WRITER_RECT = 0x200F3020
DRAW_SYSTEM_FONT = 0x200F3030
DRAW_LIST = 0x200F3040


def f32_bits(values):
    return [struct.pack("<f", value).hex() for value in values]


def run_case(case):
    # A fresh emulator per case avoids Composer/static or heap state leaking
    # from one writer invocation into another.
    probe = CanvasProbe()
    ctx, ctxvt = probe.alloc(64), probe.alloc(128)
    writer, writer_vtable = probe.alloc(64), probe.alloc(128)
    drawing, drawing_vtable = probe.alloc(128), probe.alloc(256)
    bitmap, queue, listing, note = (probe.alloc(64) for _ in range(4))
    probe.u.mem_write(ctx, struct.pack("<Q", ctxvt))
    probe.u.mem_write(ctxvt + 24, struct.pack("<Q", QUEUE))
    probe.u.mem_write(writer, struct.pack("<Q", writer_vtable))
    probe.u.mem_write(writer_vtable + 48, struct.pack("<Q", WRITER_RECT))
    probe.u.mem_write(drawing, struct.pack("<Q", drawing_vtable))
    probe.u.mem_write(drawing_vtable + 144, struct.pack("<Q", DRAW_SYSTEM_FONT))
    probe.u.mem_write(drawing_vtable + 16, struct.pack("<Q", DRAW_LIST))

    obj = probe.alloc(80)
    probe.u.mem_write(obj, struct.pack("<Q", ctx))
    probe.u.mem_write(obj + 8, struct.pack("<f", case["scale"]))
    probe.u.mem_write(obj + 16, struct.pack("<Q", note))
    probe.u.mem_write(obj + 24, struct.pack("<Q", writer))
    probe.u.mem_write(obj + 40, struct.pack("<Q", 0))  # Standard mode
    probe.u.mem_write(obj + 48, struct.pack("<Q", drawing))

    captured = {"bitmap_create_args": None, "draw_list": None}
    create_bitmap = next(
        name for name in probe.plt.values()
        if name.startswith("SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int,")
    )

    def bitmap_callback():
        captured["bitmap_create_args"] = [probe.x(index) for index in range(3)]
        probe.x(0, bitmap)

    probe.callbacks[create_bitmap] = bitmap_callback

    def native_boundaries(_uc, address, _size, _data):
        if address == QUEUE:
            probe.x(0, queue)
            probe.u.reg_write(R.UC_ARM64_REG_PC, probe.u.reg_read(R.UC_ARM64_REG_LR))
        elif address == WRITER_RECT:
            for index, value in enumerate(case["rect"]):
                probe.f(index, value)
            probe.u.reg_write(R.UC_ARM64_REG_PC, probe.u.reg_read(R.UC_ARM64_REG_LR))
        elif address == DRAW_SYSTEM_FONT:
            probe.u.reg_write(R.UC_ARM64_REG_PC, probe.u.reg_read(R.UC_ARM64_REG_LR))
        elif address == DRAW_LIST:
            matrix_ptr = probe.x(4)
            captured["draw_list"] = {
                "matrix": list(struct.unpack("<9f", probe.u.mem_read(matrix_ptr, 36))),
                "register_args": [probe.x(index) for index in range(7)],
            }
            probe.u.reg_write(R.UC_ARM64_REG_PC, STOP)

    probe.u.hook_add(UC_HOOK_CODE, native_boundaries)
    method = next(
        name for name in probe.syms
        if name.startswith("SPen::ObjectPDFWriter::WriteObjectListAsBitmap(")
    )
    probe.call(method, (obj, listing, 0, 0))
    if captured["draw_list"] is None or captured["bitmap_create_args"] is None:
        raise RuntimeError("native writer did not reach the DrawObjectList boundary")

    matrix = captured["draw_list"]["matrix"]
    # This second native pass verifies the actual Drawing -> Graphics model
    # matrix path and actual Oblique V4 inverse-scale calculation.
    inverse = probe.run_case({"source": "explicit_matrix", "matrix": matrix})
    return {
        "id": case["id"],
        "host_rect_f32_bits": f32_bits(case["rect"]),
        "host_export_scale_f32_bits": f32_bits([case["scale"]]),
        "native_bitmap_create_args": captured["bitmap_create_args"],
        "native_draw_list_matrix_f32_bits": f32_bits(matrix),
        "native_draw_list_register_args": captured["draw_list"]["register_args"],
        "native_drawing_set_matrix_calls": inverse["drawing_set_matrix_calls"],
        "native_graphics_model_matrix_f32_bits": inverse["model_matrix_f32_bits"],
        "native_oblique_inverse_scale_f32_bits": inverse["inverse_scale_f32_bits"],
    }


def generate():
    cases = [run_case(case) for case in CASES]
    identity = f32_bits([1, 0, 0, 0, 1, 0, 0, 0, 1])
    for case in cases:
        if case["native_draw_list_matrix_f32_bits"] != identity:
            raise RuntimeError(f"{case['id']}: Standard mode did not produce identity matrix")
        if case["native_graphics_model_matrix_f32_bits"] != identity:
            raise RuntimeError(f"{case['id']}: Drawing/Graphics model matrix was not identity")
        if case["native_oblique_inverse_scale_f32_bits"] != f32_bits([1, 1]):
            raise RuntimeError(f"{case['id']}: native Oblique inverse scale changed")
    return {
        "schema": "sdocx-oblique-standard-object-pdf-writer-matrix-native-v1",
        "evidence": {
            "libraries": {
                "libSPenBase.so": native.HASHES["Base"],
                "libSPenPenCommon.so": native.HASHES["PenCommon"],
                "libSPenFountainPen.so": native.HASHES["FountainPen"],
                "libSPenDrawing.so": native.HASHES["Drawing"],
                "libSPenGraphics.so": native.HASHES["Graphics"],
                "libSPenObliquePen.so": "9b76bdb709db9f56c5d35a1a615fe41c57ceef73c559c91efaf5bf436f90e0ee",
                "libSPenComposer.so": "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f",
            },
            "native_locations": {
                "standard_caller": "Composer+0x35add8; WriteObjectListAsBitmap call +0xae passes mode 0",
                "writer_entry": "Composer+0x37c3bc",
                "writer_dimensions_and_create_bitmap": "Composer+0x37c428-0x37c4c0",
                "writer_matrix_construction": "Composer+0x37c4cc-0x37c520",
                "writer_draw_list_call": "Composer+0x37c554",
                "writer_bounds_and_output_crop_after_draw": "Composer+0x37c594-0x37c648",
                "drawing_matrix_application": "Drawing+0x80100; two canvas SetMatrix calls",
                "graphics_set_and_get_model_matrix": "Graphics+0xaed84 and +0xae4dc",
                "oblique_v4_get_inverse_scale": "ObliquePen+0x4045c",
            },
            "host_boundaries": [
                "IPDFWriter rectangle virtual returns the explicit per-case RectF",
                "context queue, CreateBitmap result, system-font virtual, and Drawing receiver are host-provided layouts/callbacks",
                "DrawObjectList is observed at its virtual entry and returns to host without executing the list draw",
                "the second pass replays the captured native matrix through actual Drawing/Graphics/Oblique methods, with the committed canvas harness host boundaries",
            ],
            "scope": "Standard ObjectPDFWriter mode-0 matrix and bitmap dimension behavior; not a full Standard export, object rendering, bitmap pixels, or PDF pixels. Probe RectF/scale inputs are synthetic explicit values.",
        },
        "cases": cases,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--write-fixture", action="store_true")
    action.add_argument("--check-fixture", action="store_true")
    args = parser.parse_args()
    actual = generate()
    encoded = json.dumps(actual, indent=2, sort_keys=True) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
    else:
        expected = json.loads(FIXTURE.read_text())
        if actual != expected:
            raise SystemExit("Standard ObjectPDFWriter native output differs from fixture")
        print(f"matched {len(actual['cases'])} Standard ObjectPDFWriter cases")


if __name__ == "__main__":
    main()
