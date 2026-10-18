"""Execute Oblique V4's native size conversion and vector geometry helpers.

This bounded oracle runs real routines from the pinned Android ARM64 ELF under
Unicorn: RedrawPen's FCVTZU size conversion, lineToMesh, and makeTriangles. A
preallocated host vector lets makeTriangles use its native in-capacity append
path. It captures core corners, lineToMesh fringe corners, and triangle
vertices. It does not run callback-buffer submission, saved redraw, or a
renderer, so it makes no full-stroke or appearance claim.

Run from the repository root with the research Python dependencies:
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/oblique_native.py --write-fixture
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/oblique_native.py --check-fixture
"""
import argparse
import hashlib
import json
import math
import struct
from pathlib import Path

from unicorn import Uc, UC_ARCH_ARM64, UC_MODE_ARM
from unicorn import arm64_const as R

ROOT = Path(__file__).resolve().parents[1]
LIBRARY_SHA256 = "9b76bdb709db9f56c5d35a1a615fe41c57ceef73c559c91efaf5bf436f90e0ee"
LIBRARY = ROOT / "scratch/apk-analysis-native/arm64-v8a/libSPenObliquePen.so"
FIXTURE = ROOT / "conformance/oblique-v4-core.json"
BASE = 0x40000000
HEAP = 0x10000000
STACK = 0x30000000
SENTINEL = 0x30001000
LINE_TO_MESH = 0x45374
MAKE_TRIANGLES = 0x455D0
SIZE_FCVTZU = 0x409E4
DEFAULT_K = 0.7071067690849304
VECTOR_CAPACITY = 1024

LINE = (
    "SPen::ObliquePenStrokeDrawableRTV1::lineToMesh("
    "SPen::Vector2<float> const&, SPen::Vector2<float> const&, "
    "SPen::Vector3<float>&, SPen::Vector3<float>&, SPen::Vector3<float>&, "
    "SPen::Vector3<float>&, SPen::Vector3<float>&, SPen::Vector3<float>&, "
    "SPen::Vector3<float>&, SPen::Vector3<float>&)"
)


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def f32bits(value):
    return struct.pack("<f", value).hex()


class ObliqueCore:
    def __init__(self, path):
        blob = path.read_bytes()
        digest = hashlib.sha256(blob).hexdigest()
        if digest != LIBRARY_SHA256:
            raise ValueError(f"unverified Oblique library {path}: {digest}")
        if blob[:4] != b"\x7fELF" or blob[4:6] != b"\x02\x01":
            raise ValueError("expected little-endian ELF64")
        self.u = Uc(UC_ARCH_ARM64, UC_MODE_ARM)
        self.u.mem_map(BASE, 0x200000)
        self.u.mem_map(HEAP, 0x100000)
        self.u.mem_map(STACK, 0x20000)
        phoff = struct.unpack_from("<Q", blob, 32)[0]
        phentsize, phnum = struct.unpack_from("<HH", blob, 54)
        for i in range(phnum):
            kind, _flags, offset, vaddr, _paddr, filesz, memsz, _align = struct.unpack_from(
                "<II6Q", blob, phoff + i * phentsize
            )
            if kind != 1 or not memsz:
                continue
            if vaddr + memsz > 0x200000:
                raise ValueError("ELF segment exceeds bounded mapping")
            self.u.mem_write(BASE + vaddr, bytes(blob[offset : offset + filesz]))
            if memsz > filesz:
                self.u.mem_write(BASE + vaddr + filesz, bytes(memsz - filesz))
        self.u.reg_write(R.UC_ARM64_REG_TPIDR_EL0, HEAP)
        self.u.reg_write(R.UC_ARM64_REG_SP, STACK + 0x10000)
        self.blob = blob

    def write_f32(self, address, value):
        self.u.mem_write(address, struct.pack("<f", value))

    def floats(self, address, count):
        return list(struct.unpack("<" + "f" * count, self.u.mem_read(address, 4 * count)))

    def size_conversion(self, saved_size):
        """Run exactly the native FCVTZU W9,S0 instruction at 0x409e4."""
        if not math.isfinite(saved_size) or saved_size < 0 or saved_size >= 2**32:
            raise ValueError("fixture saved size must be finite and in [0, 2^32)")
        self.u.reg_write(R.UC_ARM64_REG_S0, struct.unpack("<I", struct.pack("<f", saved_size))[0])
        start = BASE + SIZE_FCVTZU
        self.u.emu_start(start, start + 4, count=1)
        if self.u.reg_read(R.UC_ARM64_REG_PC) != start + 4:
            raise RuntimeError("native FCVTZU instruction did not complete")
        return self.u.reg_read(R.UC_ARM64_REG_W9)

    def line_to_mesh(self, stored_size, k, inverse_scale, previous, current):
        obj = HEAP + 0x1000
        p0, p1 = HEAP + 0x2000, HEAP + 0x2010
        outputs = [HEAP + 0x3000 + 0x10 * i for i in range(8)]
        self.u.mem_write(obj, bytes(512))
        self.u.mem_write(obj + 60, struct.pack("<I", stored_size))
        self.write_f32(obj + 200, k)
        self.write_f32(obj + 352, inverse_scale[0])
        self.write_f32(obj + 356, inverse_scale[1])
        self.u.mem_write(p0, struct.pack("<2f", *previous))
        self.u.mem_write(p1, struct.pack("<2f", *current))
        for out in outputs:
            self.u.mem_write(out, bytes([0xA5]) * 12)

        # AArch64 receives x0-x7, then the final three references on the stack.
        self.u.reg_write(R.UC_ARM64_REG_X0, obj)
        self.u.reg_write(R.UC_ARM64_REG_X1, p0)
        self.u.reg_write(R.UC_ARM64_REG_X2, p1)
        for index, out in enumerate(outputs[:5]):
            self.u.reg_write(R.UC_ARM64_REG_X3 + index, out)
        call_sp = STACK + 0x8000
        self.u.reg_write(R.UC_ARM64_REG_SP, call_sp)
        self.u.mem_write(call_sp, struct.pack("<3Q", *outputs[5:]))
        self.u.reg_write(R.UC_ARM64_REG_LR, SENTINEL)
        start = BASE + LINE_TO_MESH
        self.u.emu_start(start, SENTINEL, count=2000)
        if self.u.reg_read(R.UC_ARM64_REG_PC) != SENTINEL:
            raise RuntimeError("lineToMesh did not return within 2000 instructions")
        return outputs, [self.floats(out, 3) for out in outputs]

    def make_triangles(self, outputs):
        """Run makeTriangles against a preallocated libc++ vector-of-Vector3."""
        vector = HEAP + 0x7000
        vector_data = HEAP + 0x8000
        vector_end = vector_data
        vector_cap = vector_data + VECTOR_CAPACITY * 12
        self.u.mem_write(vector, struct.pack("<3Q", vector_data, vector_end, vector_cap))
        self.u.mem_write(vector_data, bytes(VECTOR_CAPACITY * 12))
        # makeTriangles is an instance method: x0 is the drawable, x1 the
        # output vector, x2-x7 six Vector3 references, then two stack refs.
        self.u.reg_write(R.UC_ARM64_REG_X0, HEAP + 0x1000)
        self.u.reg_write(R.UC_ARM64_REG_X1, vector)
        for index, out in enumerate(outputs[:6]):
            self.u.reg_write(R.UC_ARM64_REG_X2 + index, out)
        call_sp = STACK + 0x8000
        self.u.reg_write(R.UC_ARM64_REG_SP, call_sp)
        self.u.mem_write(call_sp, struct.pack("<2Q", *outputs[6:]))
        self.u.reg_write(R.UC_ARM64_REG_LR, SENTINEL)
        start = BASE + MAKE_TRIANGLES
        self.u.emu_start(start, SENTINEL, count=200000)
        if self.u.reg_read(R.UC_ARM64_REG_PC) != SENTINEL:
            raise RuntimeError("makeTriangles did not return within 200000 instructions")
        begin, end, capacity = struct.unpack("<3Q", self.u.mem_read(vector, 24))
        if begin != vector_data or capacity != vector_cap or not begin <= end <= capacity:
            raise RuntimeError("makeTriangles changed the preallocated vector storage")
        if (end - begin) % 12:
            raise RuntimeError("native vector contains a partial Vector3")
        return [self.floats(begin + 12 * i, 3) for i in range((end - begin) // 12)]


def cases():
    return [
        {"id": "zero_width_diagonal", "saved_size": 0.0, "previous": [10.0, 20.0], "current": [18.0, 25.0]},
        {"id": "fraction_below_one_horizontal", "saved_size": 0.75, "previous": [10.0, 20.0], "current": [18.0, 20.0]},
        {"id": "integer_boundary_low_vertical", "saved_size": 1.999, "previous": [10.0, 20.0], "current": [10.0, 27.0]},
        {"id": "integer_boundary_high_reverse", "saved_size": 2.0, "previous": [18.0, 25.0], "current": [10.0, 20.0]},
        {"id": "size_eight_diagonal", "saved_size": 8.75, "previous": [10.0, 20.0], "current": [18.0, 25.0]},
        {"id": "size_nine_coincident", "saved_size": 9.0, "previous": [10.0, 20.0], "current": [10.0, 20.0]},
        {"id": "sub_epsilon_diagonal", "saved_size": 4.0, "previous": [10.0, 20.0], "current": [10.0000005, 20.0000005]},
        {"id": "above_epsilon_diagonal", "saved_size": 4.0, "previous": [10.0, 20.0], "current": [10.000002, 20.000002]},
        {"id": "nonunit_inverse_scale_x2_yhalf", "saved_size": 5.0, "runtime_K": 0.5, "inverse_scale": [2.0, 0.5], "previous": [4.0, 8.0], "current": [16.0, 12.0]},
        {"id": "nonunit_inverse_scale_xquarter_y3", "saved_size": 5.0, "runtime_K": 0.9, "inverse_scale": [0.25, 3.0], "previous": [4.0, 8.0], "current": [16.0, 12.0]},
        {"id": "asymmetric_negative_reverse_nonunit", "saved_size": 13.75, "runtime_K": 0.37, "inverse_scale": [0.75, 1.3], "previous": [3.25, -8.5], "current": [-7.75, -1.125]},
        {"id": "negative_horizontal_reverse", "saved_size": 17.0, "previous": [-24.25, -7.5], "current": [-2.0, -7.5]},
        {"id": "negative_vertical_forward", "saved_size": 6.0, "previous": [-13.0, -22.0], "current": [-13.0, -0.25]},
        {"id": "large_finite_stored_width", "saved_size": 65535.0, "runtime_K": 0.125, "inverse_scale": [1.5, 0.625], "previous": [-100.0, 250.0], "current": [1350.0, -925.0]},
        {"id": "large_coordinates_asymmetric", "saved_size": 31.0, "runtime_K": 0.625, "inverse_scale": [0.5, 2.0], "previous": [-1048576.0, 524288.0], "current": [1048576.0, -262144.0]},
        {"id": "epsilon_exact_x_sub_epsilon_y", "saved_size": 4.0, "previous": [0.0, 0.0], "current": [float.fromhex("0x1.0c6f7ap-20"), 0.0000005]},
        {"id": "epsilon_ulp_above_x_sub_epsilon_y", "saved_size": 4.0, "previous": [0.0, 0.0], "current": [float.fromhex("0x1.0c6f7cp-20"), 0.0000005]},
        {"id": "epsilon_ulp_below_x_sub_epsilon_y", "saved_size": 4.0, "previous": [0.0, 0.0], "current": [float.fromhex("0x1.0c6f78p-20"), 0.0000005]},
        {"id": "epsilon_split_axes_x_sub_y_above", "saved_size": 4.0, "previous": [0.0, 0.0], "current": [0.0000005, float.fromhex("0x1.0c6f7cp-20")]},
    ]


def generate(path):
    native = ObliqueCore(path)
    results = []
    for case in cases():
        stored = native.size_conversion(case["saved_size"])
        k = case.get("runtime_K", DEFAULT_K)
        inverse_scale = case.get("inverse_scale", [1.0, 1.0])
        outputs, vectors = native.line_to_mesh(
            stored, k, inverse_scale, case["previous"], case["current"]
        )
        triangles = native.make_triangles(outputs)
        results.append(
            {
                **case,
                "native_stored_size_u32": stored,
                "runtime_K": k,
                "runtime_K_f32_bits": f32bits(k),
                "inverse_scale": inverse_scale,
                "core_corners": vectors[:4],
                "core_corner_f32_bits": [[f32bits(v) for v in corner] for corner in vectors[:4]],
                "line_to_mesh_fringe_corners": vectors[4:],
                "line_to_mesh_all_f32_bits": [[f32bits(v) for v in vector] for vector in vectors],
                "native_triangle_vertices": triangles,
                "native_triangle_vertex_f32_bits": [[f32bits(v) for v in point] for point in triangles],
                "scope": "lineToMesh and makeTriangles outputs; callback-buffer submission and appearance excluded",
            }
        )
    return {
        "schema": "sdocx-oblique-v4-native-mesh-v2",
        "evidence": {
            "library": "libSPenObliquePen.so",
            "sha256": LIBRARY_SHA256,
            "lineToMesh_symbol": LINE,
            "lineToMesh_elf_vaddr": "0x45374",
            "makeTriangles_symbol": "SPen::ObliquePenStrokeDrawableRTV1::makeTriangles(std::__ndk1::vector<SPen::Vector3<float>, std::__ndk1::allocator<SPen::Vector3<float> > >*, SPen::Vector3<float> const&, SPen::Vector3<float> const&, SPen::Vector3<float> const&, SPen::Vector3<float> const&, SPen::Vector3<float> const&, SPen::Vector3<float> const&, SPen::Vector3<float> const&, SPen::Vector3<float> const&)",
            "makeTriangles_elf_vaddr": "0x455d0",
            "vector_stub": "host-initialized libc++ vector triple and 1024 preallocated Vector3 slots; reallocation is excluded",
            "native_size_instruction": "ObliquePenStrokeDrawableGLV4::RedrawPen+0xbc (FCVTZU W9,S0), ELF vaddr 0x409e4",
            "native_size_instruction_elf_vaddr": "0x409e4",
            "default_runtime_K": DEFAULT_K,
            "default_runtime_K_f32_bits": f32bits(DEFAULT_K),
            "limitations": [
                "The actual FCVTZU instruction and complete lineToMesh/makeTriangles routines execute under Unicorn.",
                "The triangle vector uses host-initialized preallocated storage; growth and destruction are excluded.",
                "Near-coincident cases characterize the selected coordinates only; native epsilon comparisons may depend on f32 rounding and inverse scale.",
                "Callback-buffer submission, saved redraw, cache, SVG/PDF, shader and pixel behavior are not tested.",
            ],
        },
        "cases": results,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--library", type=Path, default=LIBRARY)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--write-fixture", action="store_true")
    action.add_argument("--check-fixture", action="store_true")
    args = parser.parse_args()
    result = generate(args.library)
    encoded = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f"wrote {FIXTURE.relative_to(ROOT)} with {len(result['cases'])} native cases")
    else:
        if not FIXTURE.exists() or FIXTURE.read_text() != encoded:
            raise SystemExit("fixture mismatch; regenerate only after reviewing native output")
        print(f"verified {len(result['cases'])} native cases against {FIXTURE.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
