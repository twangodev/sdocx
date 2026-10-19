"""Bounded native trace for Oblique V4's Drawing canvas matrix context.

The pinned Composer PDF exporter builds its translation-only Matrix3 in native
code. Drawing then applies the matrix through both canvas SetMatrix vslots;
the Oblique V4 inverse-scale method calls the real Graphics GetModelMatrix.
Canvas factories, bitmap/context access, pen lookup, reference ownership, and
unsupported object paint are explicit host boundaries; SetCanvas itself runs
natively. This is runtime-context evidence,
not a full document export or renderer capture.

Run with the repository's pinned research runtime:
  PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/apk-analysis-runtime/binutils-aarch64/usr/bin \
    python3 conformance/oblique_canvas_native.py --check-fixture
"""
import argparse
import hashlib
import json
import os
import re
import struct
import subprocess
from pathlib import Path
import sys

from unicorn import UC_HOOK_CODE
from unicorn import arm64_const as R

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "conformance"))
import fountain_native as native  # noqa: E402

LIBRARY_DIR = ROOT / "scratch/apk-analysis-native/arm64-v8a"
OBJDUMP = ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump"
FIXTURE = ROOT / "conformance/oblique-canvas-native.json"
COMPOSER_SHA256 = "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f"
OBLIQUE_SHA256 = "9b76bdb709db9f56c5d35a1a615fe41c57ceef73c559c91efaf5bf436f90e0ee"

native.HASHES["ObliquePen"] = OBLIQUE_SHA256
native.HASHES["Composer"] = COMPOSER_SHA256

STUB_GET_DRAWABLE = 0x200F2000
STUB_GET_CONTEXT = 0x200F2020
STUB_CANVAS_TAG = 0x200F2030
STUB_CANVAS_DATA = 0x200F2040
STUB_CANVAS_REFERENCE = 0x200F2050
STUB_REFERENCE_RELEASE = 0x200F2060
STOP = 0x200F1000

DRAW_OBJECT = "SPen::ObjectDrawing::DrawObject(SPen::ISPBitmap*, SPen::ObjectBase const*, SPen::Rect const*, SPen::Matrix3<float> const*, bool)"
DRAWING_SET_CANVAS = "SPen::ObjectDrawing::setIPenBitmap(SPen::ISPPenCanvas*, SPen::IPen*, SPen::StrokeType)"
OBLIQUE_INVERSE_SCALE = "SPen::ObliquePenStrokeDrawableGLV4::getInverseScale()"
GRAPHICS_SET_MATRIX = "SPen::SPPenCanvas::SetMatrix(SPen::Matrix3<float> const&)"
GRAPHICS_GET_MATRIX = "SPen::SPPenCanvas::GetModelMatrix()"
PEN_COMMON_SET_CANVAS = "SPen::PenStrokeDrawableGL::SetCanvas(SPen::ISPPenCanvas*)"


def f32_bits(values):
    return [struct.pack("<f", value).hex() for value in values]


class CanvasProbe(native.NativeFountain):
    def __init__(self):
        super().__init__(LIBRARY_DIR, OBJDUMP, ("Drawing", "Graphics", "ObliquePen"))
        self.load_composer()
        self.draw_object_base = self.syms[DRAW_OBJECT] - 0x80100
        self.composer_base = self.syms["SPen::ObjectStrokePdfExporter::ExportObject()"] - 0x34F684
        self.set_matrix_calls = 0
        self.get_matrix_calls = 0

        self.drawing = self.alloc(128)
        self.context, self.context_vtable = self.alloc(32), self.alloc(256)
        self.bitmap, self.bitmap_vtable = self.alloc(128), self.alloc(256)
        self.draw_canvas = self.make_canvas()
        self.pen_canvas = self.make_canvas()
        self.oblique = self.alloc(512)
        self.drawable_vtable = self.alloc(256)
        self.pen, self.pen_vtable = self.alloc(256), self.alloc(256)
        self.u.mem_write(self.drawing, bytes(128))
        self.u.mem_write(self.oblique, bytes(512))
        self.u.mem_write(self.oblique, struct.pack("<Q", self.drawable_vtable))
        self.u.mem_write(
            self.drawable_vtable + 16,
            struct.pack("<Q", self.syms[PEN_COMMON_SET_CANVAS]),
        )
        self.u.mem_write(self.drawable_vtable + 200, struct.pack("<Q", STUB_CANVAS_TAG))
        self.u.mem_write(self.pen, struct.pack("<Q", self.pen_vtable))
        self.u.mem_write(self.pen_vtable + 232, struct.pack("<Q", STUB_GET_DRAWABLE))
        self.u.mem_write(self.context, struct.pack("<Q", self.context_vtable))
        self.u.mem_write(self.bitmap, struct.pack("<Q", self.bitmap_vtable))
        self.u.mem_write(self.context_vtable + 24, struct.pack("<Q", STUB_GET_CONTEXT))
        self.u.mem_write(self.bitmap_vtable + 24, struct.pack("<Q", STUB_GET_CONTEXT))
        self.canvas_reference, reference_vtable = self.alloc(32), self.alloc(128)
        self.u.mem_write(self.canvas_reference, struct.pack("<Q", reference_vtable))
        self.u.mem_write(reference_vtable + 72, struct.pack("<Q", STUB_REFERENCE_RELEASE))
        self.u.mem_write(reference_vtable + 80, struct.pack("<Q", STUB_REFERENCE_RELEASE))
        for canvas in (self.draw_canvas, self.pen_canvas):
            vtable = struct.unpack("<Q", self.u.mem_read(canvas, 8))[0]
            self.u.mem_write(vtable + 184, struct.pack("<Q", STUB_CANVAS_DATA))
            self.u.mem_write(vtable + 192, struct.pack("<Q", STUB_CANVAS_REFERENCE))
            self.u.mem_write(vtable + 200, struct.pack("<Q", STUB_CANVAS_TAG))
        self.u.mem_write(self.u.reg_read(R.UC_ARM64_REG_TPIDR_EL0) + 40, struct.pack("<Q", 0x123456789ABCDEF0))

        self.callbacks.update({
            "SPen::SPGraphicsFactory::CreateCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*, int)": lambda: self.x(0, self.draw_canvas),
            "SPen::SPGraphicsFactory::CreatePenCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*, int)": lambda: self.x(0, self.pen_canvas),
            "SPen::SPGraphicsFactory::ReleaseCanvas(SPen::ISPCanvasBase*)": lambda: self.x(0, 0),
            "SPen::ObjectBase::GetType() const": lambda: self.x(0, 0xFFFF),
            "SPen::ObjectBase::IsVisible() const": lambda: self.x(0, 0),
            "__android_log_print": lambda: self.x(0, 0),
            "memcmp": self.memcmp,
        })
        self.u.hook_add(UC_HOOK_CODE, self.hook_canvas)

    def make_canvas(self):
        obj, vtable = self.alloc(320), self.alloc(256)
        self.u.mem_write(obj, struct.pack("<Q", vtable))
        self.u.mem_write(vtable + 72, struct.pack("<Q", self.syms[GRAPHICS_SET_MATRIX]))
        self.u.mem_write(vtable + 168, struct.pack("<Q", STUB_CANVAS_TAG))
        self.u.mem_write(vtable + 216, struct.pack("<Q", self.syms[GRAPHICS_GET_MATRIX]))
        self.u.mem_write(obj + 216, struct.pack("<9f", 1, 0, 0, 0, 1, 0, 0, 0, 1))
        return obj

    @staticmethod
    def composer_image_size():
        blob = (LIBRARY_DIR / "libSPenComposer.so").read_bytes()
        phoff = struct.unpack_from("<Q", blob, 32)[0]
        phentsize, phnum = struct.unpack_from("<HH", blob, 54)
        extent = 0
        for index in range(phnum):
            kind, _flags, _offset, vaddr, _paddr, _filesz, memsz, _align = struct.unpack_from(
                "<II6Q", blob, phoff + index * phentsize
            )
            if kind == 1:
                extent = max(extent, vaddr + memsz)
        return (extent + 0xFFF) & ~0xFFF

    def load_composer(self):
        path = LIBRARY_DIR / "libSPenComposer.so"
        blob = path.read_bytes()
        if hashlib.sha256(blob).hexdigest() != COMPOSER_SHA256:
            raise RuntimeError("unverified Composer DSO")
        base = 0x6000000
        self.u.mem_map(base, self.composer_image_size())
        phoff = struct.unpack_from("<Q", blob, 32)[0]
        phentsize, phnum = struct.unpack_from("<HH", blob, 54)
        for index in range(phnum):
            kind, _flags, offset, vaddr, _paddr, filesz, memsz, _align = struct.unpack_from(
                "<II6Q", blob, phoff + index * phentsize
            )
            if kind == 1 and memsz:
                self.u.mem_write(base + vaddr, blob[offset : offset + filesz])
                if memsz > filesz:
                    self.u.mem_write(base + vaddr + filesz, bytes(memsz - filesz))

        env = dict(os.environ)
        runtime = ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu"
        if runtime.is_dir():
            env["LD_LIBRARY_PATH"] = str(runtime.resolve())
        for line in subprocess.check_output(["nm", "-D", "-C", path], text=True, env=env).splitlines():
            parts = line.strip().split(" ", 2)
            if len(parts) == 3 and parts[1] in ("T", "W"):
                self.syms[parts[2]] = base + int(parts[0], 16)
        disassembly = subprocess.check_output(
            [str(OBJDUMP), "-d", "-C", "-j", ".plt", path], text=True, env=env
        )
        for address, name in re.findall(r"^([0-9a-f]+) <(.+)@plt>:", disassembly, re.M):
            self.plt[base + int(address, 16)] = name

    def hook_canvas(self, _uc, address, _size, _data):
        if address == STUB_GET_DRAWABLE:
            self.x(0, self.oblique)
            self.u.reg_write(R.UC_ARM64_REG_PC, self.u.reg_read(R.UC_ARM64_REG_LR))
        elif address == STUB_GET_CONTEXT:
            self.x(0, 0xDEAD0000)
            self.u.reg_write(R.UC_ARM64_REG_PC, self.u.reg_read(R.UC_ARM64_REG_LR))
        elif address == STUB_CANVAS_DATA:
            self.x(0, self.context)
            self.u.reg_write(R.UC_ARM64_REG_PC, self.u.reg_read(R.UC_ARM64_REG_LR))
        elif address == STUB_CANVAS_REFERENCE:
            self.x(0, self.canvas_reference)
            self.u.reg_write(R.UC_ARM64_REG_PC, self.u.reg_read(R.UC_ARM64_REG_LR))
        elif address == STUB_REFERENCE_RELEASE:
            self.x(0, 0)
            self.u.reg_write(R.UC_ARM64_REG_PC, self.u.reg_read(R.UC_ARM64_REG_LR))
        elif address == STUB_CANVAS_TAG:
            self.x(0, 0)
            self.u.reg_write(R.UC_ARM64_REG_PC, self.u.reg_read(R.UC_ARM64_REG_LR))
        elif address == self.syms[GRAPHICS_SET_MATRIX]:
            self.set_matrix_calls += 1
        elif address == self.syms[GRAPHICS_GET_MATRIX]:
            self.get_matrix_calls += 1

    def memcmp(self):
        left, right, length = self.x(0), self.x(1), self.x(2)
        a = bytes(self.u.mem_read(left, length))
        b = bytes(self.u.mem_read(right, length))
        self.x(0, 0 if a == b else next((x - y for x, y in zip(a, b) if x != y), 0))

    def call(self, name, x=(), *, x8=None, limit=1_000_000):
        for index, value in enumerate(x):
            self.x(index, value)
        if x8 is not None:
            self.u.reg_write(R.UC_ARM64_REG_X8, x8)
        self.u.reg_write(R.UC_ARM64_REG_SP, 0x200EF000)
        self.u.reg_write(R.UC_ARM64_REG_LR, STOP)
        self.u.emu_start(self.syms[name], STOP, count=limit)
        if self.u.reg_read(R.UC_ARM64_REG_PC) != STOP:
            raise RuntimeError(f"{name} did not return: PC={self.u.reg_read(R.UC_ARM64_REG_PC):#x}")

    def pdf_export_matrix(self):
        """Run the native PDF exporter's Matrix3 ctor and translation stores."""
        exporter = self.alloc(160)
        self.u.mem_write(exporter + 104, struct.pack("<2f", 120.0, 75.0))
        drawing, vtable = self.alloc(256), self.alloc(256)
        self.u.mem_write(drawing, struct.pack("<Q", vtable))
        self.u.mem_write(vtable + 144, struct.pack("<Q", STUB_CANVAS_TAG))
        self.u.mem_write(vtable + 16, struct.pack("<Q", STUB_CANVAS_TAG))
        sp = 0x200EE000
        self.u.reg_write(R.UC_ARM64_REG_SP, sp)
        self.u.reg_write(R.UC_ARM64_REG_X29, sp + 0x70)
        self.x(19, exporter)
        self.x(22, drawing)
        stop_at_draw_list = self.composer_base + 0x34F7EC
        self.u.emu_start(self.composer_base + 0x34F784, stop_at_draw_list, count=10_000)
        if self.u.reg_read(R.UC_ARM64_REG_PC) != stop_at_draw_list:
            raise RuntimeError("native PDF matrix setup did not reach DrawObjectList")
        stack_matrix = self.x(4)
        matrix_values = struct.unpack("<9f", self.u.mem_read(stack_matrix, 36))
        matrix = self.alloc(36)
        self.u.mem_write(matrix, bytes(self.u.mem_read(stack_matrix, 36)))
        return matrix, matrix_values

    def run_case(self, case):
        self.set_matrix_calls = self.get_matrix_calls = 0
        self.u.mem_write(self.drawing + 8, struct.pack("<Q", self.context))
        self.u.mem_write(self.drawing + 80, b"\0")
        if case["source"] == "native_pdf_export_translation":
            matrix, supplied = self.pdf_export_matrix()
        else:
            supplied = case["matrix"]
            matrix = self.alloc(36)
            self.u.mem_write(matrix, struct.pack("<9f", *supplied))

        # The real Drawing body constructs both canvases and invokes their
        # actual Graphics SetMatrix methods. Object paint is suppressed by the
        # explicit unsupported-object host getter above.
        self.call(DRAW_OBJECT, (self.drawing, self.bitmap, 0xDEAD1000, 0, matrix, 0))
        if self.set_matrix_calls != 2:
            raise RuntimeError(f"Drawing applied matrix {self.set_matrix_calls} times, expected two canvases")

        # The real Drawing GL admission method dispatches GetStrokeDrawableGL
        # and SetCanvas through the explicitly bounded pen/drawable callbacks.
        self.call(DRAWING_SET_CANVAS, (self.drawing, self.pen_canvas, self.pen, 0))
        if struct.unpack("<Q", self.u.mem_read(self.oblique + 32, 8))[0] != self.pen_canvas:
            raise RuntimeError("Drawing did not assign the pen canvas")

        # This invokes actual Oblique V4 code, which virtually calls actual
        # Graphics GetModelMatrix. The Matrix3 copy constructor is native too.
        self.call(OBLIQUE_INVERSE_SCALE, (self.oblique,))
        if self.get_matrix_calls != 1:
            raise RuntimeError(f"Oblique queried model matrix {self.get_matrix_calls} times")
        inverse_scale = [self.f(index) for index in range(2)]
        oblique_model_queries = self.get_matrix_calls
        model = self.alloc(36)
        self.call(GRAPHICS_GET_MATRIX, (self.pen_canvas,), x8=model)
        returned = struct.unpack("<9f", self.u.mem_read(model, 36))
        if f32_bits(returned) != f32_bits(supplied):
            raise RuntimeError("native GetModelMatrix did not roundtrip the Drawing matrix")
        return {
            "source": case["source"],
            "supplied_matrix_f32_bits": f32_bits(supplied),
            "model_matrix_f32_bits": f32_bits(returned),
            "inverse_scale_f32_bits": f32_bits(inverse_scale),
            "drawing_set_matrix_calls": self.set_matrix_calls,
            "oblique_get_model_matrix_calls": oblique_model_queries,
            "explicit_get_model_matrix_calls": self.get_matrix_calls - oblique_model_queries,
        }


def cases():
    return [
        {"id": "identity", "source": "explicit_matrix", "matrix": [1, 0, 0, 0, 1, 0, 0, 0, 1]},
        {"id": "pdf_translation", "source": "native_pdf_export_translation"},
        {"id": "nonunit_scale", "source": "explicit_matrix", "matrix": [2, 0, 0, 0, 4, 0, 0, 0, 1]},
    ]


def generate():
    probe = CanvasProbe()
    results = []
    for case in cases():
        result = {"id": case["id"], **probe.run_case(case)}
        expected_inverse = {
            "identity": ["0000803f", "0000803f"],
            "pdf_translation": ["0000803f", "0000803f"],
            "nonunit_scale": ["0000003f", "0000803e"],
        }[case["id"]]
        if result["inverse_scale_f32_bits"] != expected_inverse:
            raise RuntimeError(f"{case['id']}: native inverse-scale bits changed")
        results.append(result)
    return {
        "schema": "sdocx-oblique-drawing-canvas-native-v1",
        "evidence": {
            "libraries": {
                "libSPenBase.so": native.HASHES["Base"],
                "libSPenPenCommon.so": native.HASHES["PenCommon"],
                "libSPenFountainPen.so": native.HASHES["FountainPen"],
                "libSPenDrawing.so": native.HASHES["Drawing"],
                "libSPenGraphics.so": native.HASHES["Graphics"],
                "libSPenObliquePen.so": OBLIQUE_SHA256,
                "libSPenComposer.so": COMPOSER_SHA256,
            },
            "native_methods": {
                "Drawing::DrawObject": "0x80100; canvas matrix calls at 0x80274 and 0x80290",
                "Drawing::setIPenBitmap": "0x829fc; StrokeType zero GL route",
                "PenCommon::PenStrokeDrawableGL::SetCanvas": "resolved ELF symbol; native canvas assignment with host reference/resource callbacks",
                "Graphics::SPPenCanvas::SetMatrix": "0xaed84",
                "Graphics::SPPenCanvas::GetModelMatrix": "0xae4dc",
                "Oblique V4 getInverseScale": "0x4045c",
                "Composer PDF matrix fragment": "ObjectStrokePdfExporter::ExportObject+0x100 through pre-DrawObjectList boundary",
            },
            "host_boundaries": [
                "SPGraphicsFactory canvas creation/release and bitmap/context access",
                "canvas tag and reference-count virtual methods; reference accessors return zero, exercising native SetCanvas's logged no-resource path",
                "Pen GetStrokeDrawableGL; native PenStrokeDrawableGL::SetCanvas owns canvas assignment",
                "__android_log_print return-zero logger boundary",
                "ObjectBase GetType/IsVisible return an unsupported invisible object to suppress paint",
                "bounded memcmp implementation",
            ],
            "scope": "Native matrix propagation and inverse-scale lookup only; no saved stroke geometry, GPU submission, full PDF export, or general caller matrix policy.",
        },
        "cases": results,
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
            raise SystemExit("Oblique canvas native output differs from fixture")
        print(f"matched {len(actual['cases'])} native canvas cases")


if __name__ == "__main__":
    main()
