"""Bounded Unicorn runner for Oblique V4's saved redraw traversal.

This runner calls the shipped GLV4 ``redraw`` body and its actual RTV1
``AddPoint`` callback. Android object accessors and the MotionEvent/RectF host
boundary are explicit stubs; the sampling and mesh routines remain native.
"""
import argparse
import json
import struct
from pathlib import Path

from unicorn import UC_HOOK_CODE
from unicorn import arm64_const as R

import oblique_native as core

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "conformance/oblique-redraw.json"
REDRAW = 0x40928
REDRAW_LINE = 0x405A0
END_PEN = 0x3F8B4
ADD_POINT = 0x45288
PLT = 0x66AA0
BASE_LIBRARY = ROOT / "scratch/apk-analysis-native/arm64-v8a/libSPenBase.so"
BASE_LIBRARY_SHA256 = "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb"
BASE2 = 0x50000000
CUSTOM_QUEUE_CALL = core.HEAP + 0x1F000
CUSTOM_RUNTIME_CALL = core.HEAP + 0x1F100


def f32bits(v):
    return struct.pack("<f", v).hex()


def f64bits(v):
    return struct.pack("<d", v).hex()


class RedrawOracle(core.ObliqueCore):
    def __init__(self, path, case):
        super().__init__(path)
        self.case = case
        self.bump = core.HEAP + 0x20000
        self.stub_names = self._load_base_library(BASE_LIBRARY)
        self.stub_names.update(self._load_relocations())
        self.points = case["points"]
        self.times = case.get("timestamps", list(range(len(self.points))))
        self.pressures = case.get("pressures", [0.5] * len(self.points))
        self.tilts = case.get("tilts", [0.0] * len(self.points))
        self.orientations = case.get("orientations", [0.0] * len(self.points))
        self.points_ptr = self._array("f", self.points)
        self.times_ptr = self._array("i", self.times)
        self.pressures_ptr = self._array("f", self.pressures)
        self.tilts_ptr = self._array("f", self.tilts)
        self.orientations_ptr = self._array("f", self.orientations)
        self.event_xy = self.points[-1]
        self.calls = []
        self.trace = []
        self.sample_ends = []
        self.terminal_start = None
        self.terminal_end = None
        self._setup_drawable()
        self.u.hook_add(UC_HOOK_CODE, self._hook)

    def _load_relocations(self):
        # The native helper calls internal methods through JUMP_SLOT entries.
        # Rebase RELATIVE and locally-defined JUMP_SLOT targets exactly as the
        # Android loader would; undefined imports are intercepted below.
        blob = self.blob
        phoff = struct.unpack_from("<Q", blob, 32)[0]
        shoff = struct.unpack_from("<Q", blob, 40)[0]
        shentsize, shnum, shstrndx = struct.unpack_from("<HHH", blob, 58)
        sections = [struct.unpack_from("<IIQQQQIIQQ", blob, shoff + i * shentsize) for i in range(shnum)]
        shstr = sections[shstrndx]
        names = blob[shstr[4] : shstr[4] + shstr[5]]
        by_name = {}
        for section in sections:
            off = section[0]
            end = names.find(b"\0", off)
            by_name[names[off:end].decode()] = section
        dynsym = by_name[".dynsym"]
        dynstr = by_name[".dynstr"]
        strings = blob[dynstr[4] : dynstr[4] + dynstr[5]]
        symbols = []
        for off in range(dynsym[4], dynsym[4] + dynsym[5], dynsym[9]):
            name_i, info, other, shndx, value, size = struct.unpack_from("<IBBHQQ", blob, off)
            end = strings.find(b"\0", name_i)
            symbols.append((strings[name_i:end].decode() if name_i else "", value))
        jump_slots = {}
        for secname in (".rela.dyn", ".rela.plt"):
            sec = by_name.get(secname)
            if not sec:
                continue
            jump_index = 0
            for off in range(sec[4], sec[4] + sec[5], sec[9]):
                where, info, addend = struct.unpack_from("<QQq", blob, off)
                typ, sym_i = info & 0xFFFFFFFF, info >> 32
                if typ == 0x403:  # R_AARCH64_RELATIVE
                    self.u.mem_write(core.BASE + where, struct.pack("<Q", core.BASE + addend))
                elif typ == 0x402:  # R_AARCH64_JUMP_SLOT
                    sym_name, sym_value = symbols[sym_i]
                    target = core.BASE + sym_value if sym_value else self.symbols.get(sym_name, 0)
                    self.u.mem_write(core.BASE + where, struct.pack("<Q", target))
                    if secname == ".rela.plt" and not target:
                        jump_slots[core.BASE + PLT + 0x20 + jump_index * 16] = sym_name
                    if secname == ".rela.plt":
                        jump_index += 1
        return jump_slots

    def _load_base_library(self, path):
        """Map libSPenBase and resolve core SmPath imports to its real code."""
        blob = path.read_bytes()
        self.base_sha256 = __import__("hashlib").sha256(blob).hexdigest()
        if self.base_sha256 != BASE_LIBRARY_SHA256:
            raise ValueError(f"unverified libSPenBase {path}: {self.base_sha256}")
        phoff = struct.unpack_from("<Q", blob, 32)[0]
        phentsize, phnum = struct.unpack_from("<HH", blob, 54)
        self.u.mem_map(BASE2, 0x300000)
        for i in range(phnum):
            kind, _flags, off, va, _pa, filesz, memsz, _align = struct.unpack_from("<II6Q", blob, phoff + i * phentsize)
            if kind == 1 and memsz:
                self.u.mem_write(BASE2 + va, blob[off : off + filesz])
                if memsz > filesz:
                    self.u.mem_write(BASE2 + va + filesz, bytes(memsz - filesz))

        shoff = struct.unpack_from("<Q", blob, 40)[0]
        shentsize, shnum, shstrndx = struct.unpack_from("<HHH", blob, 58)
        sections = [struct.unpack_from("<IIQQQQIIQQ", blob, shoff + i * shentsize) for i in range(shnum)]
        shstr = sections[shstrndx]
        shnames = blob[shstr[4] : shstr[4] + shstr[5]]
        by_name = {}
        for section in sections:
            off = section[0]
            end = shnames.find(b"\0", off)
            by_name[shnames[off:end].decode()] = section
        dynsym, dynstr = by_name[".dynsym"], by_name[".dynstr"]
        strings = blob[dynstr[4] : dynstr[4] + dynstr[5]]
        symbols = []
        for off in range(dynsym[4], dynsym[4] + dynsym[5], dynsym[9]):
            name_i, info, other, shndx, value, size = struct.unpack_from("<IBBHQQ", blob, off)
            end = strings.find(b"\0", name_i)
            symbols.append((strings[name_i:end].decode() if name_i else "", value))
        exports = {name: value for name, value in symbols if value}
        self.symbols = {name: BASE2 + value for name, value in exports.items()}
        plt_address = next(sec[3] for name, sec in by_name.items() if name == ".plt")
        names = {}
        for secname in (".rela.dyn", ".rela.plt"):
            sec = by_name.get(secname)
            if not sec:
                continue
            plt_index = 0
            for off in range(sec[4], sec[4] + sec[5], sec[9]):
                where, info, addend = struct.unpack_from("<QQq", blob, off)
                typ, sym_i = info & 0xFFFFFFFF, info >> 32
                if typ == 0x403:
                    self.u.mem_write(BASE2 + where, struct.pack("<Q", BASE2 + addend))
                elif typ == 0x402:
                    name, value = symbols[sym_i]
                    target = BASE2 + value if value else self.symbols.get(name, 0)
                    self.u.mem_write(BASE2 + where, struct.pack("<Q", target))
                    if secname == ".rela.plt" and not target:
                        names[BASE2 + plt_address + 0x20 + plt_index * 16] = name
                    if secname == ".rela.plt":
                        plt_index += 1

        return names

    def _alloc(self, data):
        address = self.bump
        self.bump += (len(data) + 15) & ~15
        self.u.mem_write(address, data)
        return address

    def _array(self, fmt, rows):
        flat = [x for row in rows for x in row] if rows and isinstance(rows[0], (list, tuple)) else rows
        return self._alloc(struct.pack("<" + fmt * len(flat), *flat))

    def _setup_drawable(self):
        self.drawable = core.HEAP + 0x4000
        self.stroke = core.HEAP + 0x6000
        self.rect = core.HEAP + 0x6200
        self.rt = core.HEAP + 0x8000
        self.pen_data = core.HEAP + 0xA000
        self.settings = core.HEAP + 0xA100
        self.manager = core.HEAP + 0xA200
        self.stroke_data = core.HEAP + 0xA300
        self.runtime = core.HEAP + 0xA400
        self.queue = core.HEAP + 0xA500
        self.vector = core.HEAP + 0xB000
        self.vector_data = core.HEAP + 0xC000
        self.u.mem_write(self.drawable, bytes(512))
        self.u.mem_write(self.stroke, bytes(256))
        self.u.mem_write(self.rect, bytes(32))
        self.u.mem_write(self.rt, bytes(512))
        self.u.mem_write(self.pen_data, bytes(64))
        self.u.mem_write(self.settings, bytes(64))
        self.u.mem_write(self.stroke_data, bytes(128))
        self.u.mem_write(self.runtime, bytes(128))
        self.queue_vtable = core.HEAP + 0xA600
        self.runtime_vtable = core.HEAP + 0xA700
        self.u.mem_write(self.runtime, struct.pack("<Q", self.runtime_vtable))
        self.u.mem_write(self.runtime_vtable + 216, struct.pack("<Q", CUSTOM_RUNTIME_CALL))
        self.u.mem_write(self.queue, struct.pack("<Q", self.queue_vtable))
        self.u.mem_write(self.queue_vtable + 16, struct.pack("<Q", CUSTOM_QUEUE_CALL))
        self.u.mem_write(self.drawable + 16, struct.pack("<Q", self.rt))
        self.u.mem_write(self.drawable + 24, struct.pack("<Q", self.queue))
        self.u.mem_write(self.drawable + 32, struct.pack("<Q", self.runtime))
        self.u.mem_write(self.drawable + 72, struct.pack("<Q", self.pen_data))
        self.u.mem_write(self.pen_data, struct.pack("<Q", self.settings))
        self.u.mem_write(self.pen_data + 8, bytes([int(self.case.get("curve_enabled", True))]))
        self.u.mem_write(self.pen_data + 9, bytes([int(self.case.get("fixed_enabled", False))]))
        self.write_f32(self.pen_data + 12, self.case.get("fixed_value", 0.0))
        self.write_f32(self.settings, self.case.get("size", 4.0))
        self.write_f32(self.drawable + 44, self.case.get("initial_tolerance", 0.0))
        self.write_f32(self.rt + 200, self.case.get("runtime_K", core.DEFAULT_K))
        self.write_f32(self.rt + 352, self.case.get("inverse_scale", [1.0, 1.0])[0])
        self.write_f32(self.rt + 356, self.case.get("inverse_scale", [1.0, 1.0])[1])
        capacity = 2048
        self.u.mem_write(self.vector, struct.pack("<3Q", self.vector_data, self.vector_data, self.vector_data + capacity * 12))
        self.u.mem_write(self.vector_data, bytes(capacity * 12))
        self.u.mem_write(self.rt + 264, struct.pack("<Q", self.vector))
        self.u.mem_write(self.stroke_data + 0, bytes(128))
        self.u.mem_write(core.HEAP, bytes(128))
        self.u.reg_write(R.UC_ARM64_REG_TPIDR_EL0, core.HEAP)
        self.u.mem_write(core.HEAP + 40, struct.pack("<Q", 0x1234))

    def _write_float(self, reg, value):
        self.u.reg_write(reg, struct.unpack("<I", struct.pack("<f", value))[0])

    def _return(self):
        self.u.reg_write(R.UC_ARM64_REG_PC, self.u.reg_read(R.UC_ARM64_REG_LR))

    def _hook(self, uc, address, size, _data):
        pc = address - core.BASE
        self.trace.append(address)
        if len(self.trace) > 40:
            self.trace.pop(0)
        if pc == 0x3FE2C:
            # Host lifetime wrapper destructor; the emulated host owns these
            # synthetic handles and there is no Android runtime to release.
            self._return()
        if pc == REDRAW_LINE:
            self.calls.append({"kind": "redrawLine", "xy_bits": [f"{self.u.reg_read(R.UC_ARM64_REG_S0):08x}", f"{self.u.reg_read(R.UC_ARM64_REG_S1):08x}"], "emit": self.u.reg_read(R.UC_ARM64_REG_W2)})
        if pc == 0x40798:
            self.calls.append({"kind": "redrawLineNoCurve", "xy_bits": [f"{self.u.reg_read(R.UC_ARM64_REG_S0):08x}", f"{self.u.reg_read(R.UC_ARM64_REG_S1):08x}"], "emit": self.u.reg_read(R.UC_ARM64_REG_W2)})
        if pc in (0x40E9C, 0x40EE4):
            self.sample_ends.append(self._vector_count())
        if pc == 0x40FB8:
            self.terminal_start = self._vector_count()
        if pc == 0x40FC8:
            self.terminal_end = self._vector_count()
        if address == CUSTOM_RUNTIME_CALL:
            result = self.u.reg_read(R.UC_ARM64_REG_X8)
            inv_x, inv_y = self.case.get("inverse_scale", [1.0, 1.0])
            self.u.mem_write(result, struct.pack("<9f", 1.0 / inv_x, 0.0, 0.0, 0.0, 1.0 / inv_y, 0.0, 0.0, 0.0, 1.0))
            self.u.reg_write(R.UC_ARM64_REG_W0, 1)
            self._return()
        elif address == CUSTOM_QUEUE_CALL:
            self.u.reg_write(R.UC_ARM64_REG_W0, 1)
            self._return()
        elif address in self.stub_names:
            self._host_stub(self.stub_names[address])
        elif pc == ADD_POINT:
            # The native callback body remains executable; record its input.
            xy = self.u.reg_read(R.UC_ARM64_REG_S0), self.u.reg_read(R.UC_ARM64_REG_S1)
            this = self.u.reg_read(R.UC_ARM64_REG_X0)
            previous = self.floats(this + 0x158, 2)
            self.calls.append({"kind": "AddPoint", "xy_bits": [f"{x:08x}" for x in xy],
                               "previous_xy_bits": [f"{struct.unpack('<I', struct.pack('<f', v))[0]:08x}" for v in previous]})

    def _host_stub(self, name):
        x0 = self.u.reg_read(R.UC_ARM64_REG_X0)
        if name.endswith("GetMsgQueueEv"):
            self.u.reg_write(R.UC_ARM64_REG_X0, self.queue)
        elif name.endswith("PenStrokeDataManager11GetInstanceEv"):
            self.u.reg_write(R.UC_ARM64_REG_X0, self.manager)
        elif name.endswith("getInverseScaleEv"):
            scale = self.case.get("inverse_scale", [1.0, 1.0])
            self._write_float(R.UC_ARM64_REG_S0, scale[0])
            self._write_float(R.UC_ARM64_REG_S1, scale[1])
        elif name.endswith("GetOriginalRuntimeHandleEv"):
            self.u.reg_write(R.UC_ARM64_REG_W0, 1)
        elif name.endswith("GetRuntimeHandleEv"):
            self.u.reg_write(R.UC_ARM64_REG_W0, 2)
        elif name.endswith("GetStrokeDataEi"):
            self.u.reg_write(R.UC_ARM64_REG_X0, self.stroke_data)
        elif name.endswith("GetPointDataEv"):
            self.u.reg_write(R.UC_ARM64_REG_X0, 0)
        elif name.endswith("hasSameInverseScaleENS_7Vector2IfEE"):
            self.u.reg_write(R.UC_ARM64_REG_W0, 0)
        elif name.endswith("IsRedrawingCacheDirtyEv"):
            self.u.reg_write(R.UC_ARM64_REG_W0, 1)
        elif name.endswith("GetPointCountEv"):
            self.u.reg_write(R.UC_ARM64_REG_W0, len(self.points))
        elif name.endswith("GetPointEv"):
            self.u.reg_write(R.UC_ARM64_REG_X0, self.points_ptr)
        elif name.endswith("GetTimeStampEv"):
            self.u.reg_write(R.UC_ARM64_REG_X0, self.times_ptr)
        elif name.endswith("GetPressureEv"):
            self.u.reg_write(R.UC_ARM64_REG_X0, self.pressures_ptr)
        elif name.endswith("GetTiltEv"):
            self.u.reg_write(R.UC_ARM64_REG_X0, self.tilts_ptr)
        elif name.endswith("GetOrientationEv"):
            self.u.reg_write(R.UC_ARM64_REG_X0, self.orientations_ptr)
        elif name.endswith("GetToolTypeEv"):
            self.u.reg_write(R.UC_ARM64_REG_W0, self.case.get("tool_type", 0))
        elif name.endswith("GetPenRectEv"):
            self.u.reg_write(R.UC_ARM64_REG_X0, self.rect)
        elif name.endswith("SetInverseScaleENS_7Vector2IfEE") or name.endswith("SetPenRectENS_5RectFE") or name.endswith("SetPointDataEPNS_18PenStrokePointDataE") or name.endswith("ClearRedrawingCacheDirtyFlagEv") or name.endswith("SetOriginalRuntimeHandleEi"):
            pass
        elif name.endswith("IsShapeEv"):
            self.u.reg_write(R.UC_ARM64_REG_W0, 0)
        elif name.endswith("GetInitialToloeranceEv"):
            self._write_float(R.UC_ARM64_REG_S0, self.case.get("initial_tolerance", 0.0))
        elif name == "_Znwm":
            size = self.u.reg_read(R.UC_ARM64_REG_X0)
            self.u.reg_write(R.UC_ARM64_REG_X0, self._alloc(bytes(max(1, size))))
        elif name.endswith("PenStrokePointDataC1Ev"):
            control = core.HEAP + 0xA800
            self.u.mem_write(control, struct.pack("<I", 2))
            self.u.mem_write(x0, bytes(64))
            self.u.mem_write(x0 + 8, struct.pack("<Q", control))
        elif name == "_ZdlPv":
            pass
        elif name == "memcmp":
            left = bytes(self.u.mem_read(self.u.reg_read(R.UC_ARM64_REG_X0), self.u.reg_read(R.UC_ARM64_REG_X2)))
            right = bytes(self.u.mem_read(self.u.reg_read(R.UC_ARM64_REG_X1), self.u.reg_read(R.UC_ARM64_REG_X2)))
            self.u.reg_write(R.UC_ARM64_REG_W0, 0 if left == right else 1)
        elif name in ("memcpy", "memmove"):
            dst = self.u.reg_read(R.UC_ARM64_REG_X0)
            src = self.u.reg_read(R.UC_ARM64_REG_X1)
            size = self.u.reg_read(R.UC_ARM64_REG_X2)
            self.u.mem_write(dst, bytes(self.u.mem_read(src, size)))
            self.u.reg_write(R.UC_ARM64_REG_X0, dst)
        elif name == "memset":
            dst = self.u.reg_read(R.UC_ARM64_REG_X0)
            byte = self.u.reg_read(R.UC_ARM64_REG_X1) & 255
            size = self.u.reg_read(R.UC_ARM64_REG_X2)
            self.u.mem_write(dst, bytes([byte]) * size)
            self.u.reg_write(R.UC_ARM64_REG_X0, dst)
        elif name.endswith("MotionEventC1ExiiPNS0_17PointerPropertiesEPNS0_13PointerCoordsEiii"):
            self.event_xy = self.points[-1]
        elif name.endswith("MotionEventD1Ev") or name.endswith("Error8SetErrorEm"):
            pass
        elif name.endswith("MotionEvent4GetXEi"):
            self.u.reg_write(R.UC_ARM64_REG_D0, struct.unpack("<Q", struct.pack("<d", self.event_xy[0]))[0])
        elif name.endswith("MotionEvent4GetYEi"):
            self.u.reg_write(R.UC_ARM64_REG_D0, struct.unpack("<Q", struct.pack("<d", self.event_xy[1]))[0])
        elif name.endswith("RectF3SetEffff"):
            self.u.mem_write(x0, struct.pack("<4f", *[self._read_float(i) for i in range(4)]))
        elif name.endswith("RectF10ExtendRectEv"):
            pass
        elif name.endswith("RectF5UnionERKS0_"):
            pass
        elif name.endswith("__stack_chk_fail"):
            raise RuntimeError("native stack canary failure")
        else:
            raise RuntimeError(f"unimplemented native host import {name} at {self.u.reg_read(R.UC_ARM64_REG_PC)-core.BASE:#x}")
        self._return()

    def _read_float(self, reg):
        return struct.unpack("<f", struct.pack("<I", self.u.reg_read(reg)))[0]

    def _current_vector(self):
        return struct.unpack("<Q", self.u.mem_read(self.rt + 264, 8))[0]

    def _vector_count(self):
        begin, end, _capacity = struct.unpack("<3Q", self.u.mem_read(self._current_vector(), 24))
        return (end - begin) // 12

    def run(self):
        self.u.reg_write(R.UC_ARM64_REG_X0, self.drawable)
        self.u.reg_write(R.UC_ARM64_REG_X1, self.stroke)
        self.u.reg_write(R.UC_ARM64_REG_X2, self.rect)
        self.u.reg_write(R.UC_ARM64_REG_W3, 1)
        self.u.reg_write(R.UC_ARM64_REG_SP, core.STACK + 0x8000)
        self.u.reg_write(R.UC_ARM64_REG_LR, core.SENTINEL)
        try:
            self.u.emu_start(core.BASE + REDRAW, core.SENTINEL, count=1_000_000)
        except Exception as exc:
            pc = self.u.reg_read(R.UC_ARM64_REG_PC)
            print("redraw fault", exc, "pc", hex(pc - core.BASE), "lr", hex(self.u.reg_read(R.UC_ARM64_REG_LR)), "trace", [hex(x-core.BASE) for x in self.trace], "calls", self.calls[-12:])
            raise
        if self.u.reg_read(R.UC_ARM64_REG_PC) != core.SENTINEL:
            raise RuntimeError(f"GLV4 redraw did not return: PC={self.u.reg_read(R.UC_ARM64_REG_PC)-core.BASE:#x}")
        vector = self._current_vector()
        begin, end, capacity = struct.unpack("<3Q", self.u.mem_read(vector, 24))
        if not begin <= end <= capacity or (end - begin) % 12:
            raise RuntimeError("native callback vector contains invalid storage")
        count = (end - begin) // 12
        triples = [self.floats(begin + i * 12, 3) for i in range(count)]
        return {
            "size_u32": struct.unpack("<I", self.u.mem_read(self.rt + 60, 4))[0],
            "size_conversion_instruction": "FCVTZU W9,S0 at 0x409e4 executed inside GLV4 RedrawPen",
            "inverse_scale": [self.floats(self.rt + 352, 1)[0], self.floats(self.rt + 356, 1)[0]],
            "rt_previous_xy": self.floats(self.rt + 0x158, 2),
            "pen_rect_raw": self.floats(self.rect, 4),
            "callback_triples": triples,
            "callback_triple_f32_bits": [[f32bits(v) for v in p] for p in triples],
            "sample_ends_vector3": self.sample_ends,
            "terminal_start_vector3": self.terminal_start,
            "terminal_end_vector3": self.terminal_end,
            "redraw_line_calls": [c for c in self.calls if c["kind"] == "redrawLine"],
            "redraw_line_no_curve_calls": [c for c in self.calls if c["kind"] == "redrawLineNoCurve"],
            "add_point_calls": [c for c in self.calls if c["kind"] == "AddPoint"],
            "call_count": len([c for c in self.calls if c["kind"] == "AddPoint"]),
            "scope": "native GLV4 RedrawPen to RTV1 AddPoint callback; host object/runtime/cache methods, MotionEvent, RectF, and allocation boundary use explicit stubs",
        }


def generate(path):
    # Explicit host state keeps the scope bounded: caller-side cache selection
    # and scale discovery are not inferred from these direct redraw inputs.
    cases = [
        {"id": "three_points_curve", "points": [[10.0, 20.0], [18.0, 25.0], [26.0, 20.0]], "timestamps": [0, 8, 16], "size": 4.0, "curve_enabled": True},
        {"id": "three_points_line_low_size", "points": [[10.0, 20.0], [18.0, 25.0], [26.0, 20.0]], "timestamps": [0, 8, 16], "size": 0.75, "curve_enabled": False, "inverse_scale": [2.0, 0.5]},
        {"id": "coincident_curve", "points": [[10.0, 20.0], [10.0, 20.0], [10.0, 20.0]], "timestamps": [0, 8, 16], "size": 2.0, "curve_enabled": True},
        {"id": "tool_threshold_and_fractional_size", "points": [[4.0, 8.0], [16.0, 12.0], [28.0, 8.0]], "timestamps": [0, 8, 16], "size": 1.999, "curve_enabled": True, "tool_type": 1, "fixed_enabled": True, "fixed_value": 6.0, "inverse_scale": [0.25, 3.0]},
        {"id": "curve_zero_quad_after_short_alternation", "points": [[0.0, 0.0]] * 5, "size": 4.0, "curve_enabled": True, "tool_type": 1},
        {"id": "no_curve_duplicate_segments_tool3", "points": [[0.0, 0.0], [0.0, 0.0], [8.0, 0.0], [8.0, 0.0]], "size": 6.0, "curve_enabled": False, "tool_type": 3},
        {"id": "terminal_cubic_tool3", "points": [[0.0, 0.0], [10.0, 0.0], [20.0, 10.0], [30.0, 0.0], [40.0, -10.0]], "size": 6.0, "curve_enabled": True, "tool_type": 3},
        {"id": "terminal_cubic_width_band_12", "points": [[0.0, 0.0], [10.0, 0.0], [20.0, 10.0], [30.0, 0.0], [40.0, -10.0]], "size": 12.75, "curve_enabled": True, "tool_type": 2},
        {"id": "curve_zero_quad_nonorigin_tool1", "points": [[5.0, 9.0]] * 5, "size": 4.0, "curve_enabled": True, "tool_type": 1},
        {"id": "coincident_curve_tool3_nonorigin", "points": [[17.0, 23.0]] * 5, "size": 4.0, "curve_enabled": True, "tool_type": 3},
        {"id": "no_curve_duplicate_nonorigin_tool3", "points": [[5.0, 9.0], [5.0, 9.0], [13.0, 9.0], [13.0, 9.0]], "size": 6.0, "curve_enabled": False, "tool_type": 3},
        {"id": "curve_zero_quad_followed_by_move_tool1", "points": [[5.0, 9.0]] * 5 + [[100.0, 9.0]], "size": 4.0, "curve_enabled": True, "tool_type": 1},
        # Exact synthetic geometry input used by the Oblique MAX SVG transport
        # test.  This keeps its native redraw evidence separate from the paint
        # wrapper (color/opacity) and from saved-document caller state.
        {"id": "svg_transport_synthetic_tool2", "points": [[8.0, 12.0], [23.0, 15.0], [39.0, 10.0]], "timestamps": [0, 8, 16], "size": 5.0, "curve_enabled": True, "tool_type": 2, "inverse_scale": [1.0, 1.0]},
    ]
    native = []
    for case in cases:
        oracle = RedrawOracle(path, case)
        native.append({**case, "native": oracle.run()})
    return {
        "schema": "sdocx-oblique-v4-native-redraw-v1",
        "libraries": {"libSPenObliquePen.so": core.LIBRARY_SHA256, "libSPenBase.so": BASE_LIBRARY_SHA256},
        "entrypoints": {"GLV4_RedrawPen": "0x40928", "GLV4_redraw": "0x40d64", "RTV1_AddPoint": "0x45288", "lineToMesh": "0x45374", "makeTriangles": "0x455d0"},
        "native_base_support": {"SmPath_getLength": "libSPenBase.so:0xbd370", "SmPath_getPosTan": "libSPenBase.so:0xbd3a0", "SmPath_cubicTo": "libSPenBase.so:0xbdcf0", "host_hashes": "pinned by libraries map"},
        "cases": native,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--library", type=Path, default=core.LIBRARY)
    parser.add_argument("--write-fixture", action="store_true")
    parser.add_argument("--check-fixture", action="store_true")
    args = parser.parse_args()
    result = generate(args.library)
    if args.write_fixture:
        FIXTURE.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    elif args.check_fixture:
        if json.loads(FIXTURE.read_text()) != result:
            raise SystemExit("native redraw differs from conformance/oblique-redraw.json")
        print(f"verified {len(result['cases'])} native redraw cases")
    else:
        print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
